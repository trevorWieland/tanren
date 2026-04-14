//! [`StateStore`] trait and its implementation on [`Store`].
//!
//! All queries in this module go through `SeaORM`'s entity API and
//! use indexed columns. Projection writes that follow an event append
//! run co-transactionally with the event insert — the caller hands
//! the store an [`EventEnvelope`] via the param struct and the store
//! appends it inside the same transaction closure.

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait, sea_query::Expr,
};
use tanren_domain::{DispatchId, DispatchView, Lane, StepId, StepView};

use crate::converters::{
    dispatch as dispatch_converters, events as event_converters, step as step_converters, validate,
};
use crate::entity::{dispatch_projection, events, step_projection};
use crate::errors::{StoreError, StoreResult};
use crate::params::{CreateDispatchParams, DispatchFilter, UpdateDispatchStatusParams};
use crate::store::Store;

/// Projection read / write interface for dispatches and steps.
#[async_trait]
pub trait StateStore: Send + Sync {
    /// Look up a dispatch by id.
    async fn get_dispatch(&self, id: &DispatchId) -> StoreResult<Option<DispatchView>>;

    /// Query dispatches by filter dimensions (status, lane, user, etc.).
    async fn query_dispatches(&self, filter: &DispatchFilter) -> StoreResult<Vec<DispatchView>>;

    /// Look up a single step by id.
    async fn get_step(&self, id: &StepId) -> StoreResult<Option<StepView>>;

    /// Return all steps for a dispatch, ordered by `step_sequence`.
    async fn get_steps_for_dispatch(&self, dispatch_id: &DispatchId) -> StoreResult<Vec<StepView>>;

    /// Count steps currently in `status = 'running'`, optionally
    /// restricted to a lane. Used by the scheduler to enforce lane
    /// concurrency caps outside the dequeue fast path.
    async fn count_running_steps(&self, lane: Option<&Lane>) -> StoreResult<u64>;

    /// Insert the initial projection row for a newly-created dispatch
    /// and append its `DispatchCreated` event in one transaction.
    async fn create_dispatch_projection(&self, params: CreateDispatchParams) -> StoreResult<()>;

    /// Update a dispatch's status (and, for terminal transitions,
    /// its outcome) and append the companion lifecycle event
    /// co-transactionally. `updated_at` is set to the current wall
    /// clock.
    async fn update_dispatch_status(&self, params: UpdateDispatchStatusParams) -> StoreResult<()>;
}

#[async_trait]
impl StateStore for Store {
    async fn get_dispatch(&self, id: &DispatchId) -> StoreResult<Option<DispatchView>> {
        let row = dispatch_projection::Entity::find_by_id(id.into_uuid())
            .one(self.conn())
            .await?;
        row.map(dispatch_converters::model_to_view).transpose()
    }

    async fn query_dispatches(&self, filter: &DispatchFilter) -> StoreResult<Vec<DispatchView>> {
        let mut query = dispatch_projection::Entity::find();
        if let Some(status) = filter.status {
            query = query.filter(dispatch_projection::Column::Status.eq(status.to_string()));
        }
        if let Some(lane) = filter.lane {
            query = query.filter(dispatch_projection::Column::Lane.eq(lane.to_string()));
        }
        if let Some(ref project) = filter.project {
            query = query.filter(dispatch_projection::Column::Project.eq(project.as_str()));
        }
        if let Some(user_id) = filter.user_id {
            query = query.filter(dispatch_projection::Column::UserId.eq(user_id.into_uuid()));
        }
        if let Some(since) = filter.since {
            query = query.filter(dispatch_projection::Column::CreatedAt.gte(since));
        }
        if let Some(until) = filter.until {
            query = query.filter(dispatch_projection::Column::CreatedAt.lt(until));
        }

        let rows = query
            .order_by_desc(dispatch_projection::Column::CreatedAt)
            .limit(filter.limit)
            .offset(filter.offset)
            .all(self.conn())
            .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(dispatch_converters::model_to_view(row)?);
        }
        Ok(out)
    }

    async fn get_step(&self, id: &StepId) -> StoreResult<Option<StepView>> {
        let row = step_projection::Entity::find_by_id(id.into_uuid())
            .one(self.conn())
            .await?;
        row.map(step_converters::model_to_view).transpose()
    }

    async fn get_steps_for_dispatch(&self, dispatch_id: &DispatchId) -> StoreResult<Vec<StepView>> {
        let rows = step_projection::Entity::find()
            .filter(step_projection::Column::DispatchId.eq(dispatch_id.into_uuid()))
            .order_by_asc(step_projection::Column::StepSequence)
            .all(self.conn())
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(step_converters::model_to_view(row)?);
        }
        Ok(out)
    }

    async fn count_running_steps(&self, lane: Option<&Lane>) -> StoreResult<u64> {
        let mut query = step_projection::Entity::find().filter(
            step_projection::Column::Status.eq(tanren_domain::StepStatus::Running.to_string()),
        );
        if let Some(lane) = lane {
            query = query.filter(step_projection::Column::Lane.eq(lane.to_string()));
        }
        Ok(query.count(self.conn()).await?)
    }

    async fn create_dispatch_projection(&self, params: CreateDispatchParams) -> StoreResult<()> {
        validate::validate_create_dispatch(&params)?;
        let projection = dispatch_converters::params_to_active_model(&params)?;
        let event_model = event_converters::envelope_to_active_model(&params.creation_event)?;

        self.conn()
            .transaction::<_, (), StoreError>(move |txn| {
                Box::pin(async move {
                    dispatch_projection::Entity::insert(projection)
                        .exec(txn)
                        .await?;
                    events::Entity::insert(event_model).exec(txn).await?;
                    Ok(())
                })
            })
            .await?;
        Ok(())
    }

    async fn update_dispatch_status(&self, params: UpdateDispatchStatusParams) -> StoreResult<()> {
        validate::validate_update_dispatch_status(&params)?;

        let now = Utc::now();
        let event_model = event_converters::envelope_to_active_model(&params.status_event)?;
        let id = params.dispatch_id;
        let id_uuid = id.into_uuid();
        let status = params.status;
        let outcome = params.outcome;

        self.conn()
            .transaction::<_, (), StoreError>(move |txn| {
                Box::pin(async move {
                    // Fetch current row to enforce lifecycle transitions.
                    let row = dispatch_projection::Entity::find_by_id(id_uuid)
                        .one(txn)
                        .await?;
                    let row = row.ok_or_else(|| StoreError::NotFound {
                        entity: format!("dispatch {id}"),
                    })?;
                    let current = dispatch_converters::parse_status(&row.status)?;
                    if !current.can_transition_to(status) {
                        return Err(StoreError::InvalidTransition {
                            entity: format!("dispatch {id}"),
                            from: current.to_string(),
                            to: status.to_string(),
                        });
                    }

                    // CAS: only update if the status still matches
                    // what we read, preventing concurrent callers
                    // from both succeeding.
                    let result = dispatch_projection::Entity::update_many()
                        .col_expr(
                            dispatch_projection::Column::Status,
                            Expr::value(status.to_string()),
                        )
                        .col_expr(
                            dispatch_projection::Column::Outcome,
                            Expr::value(outcome.map(|o| o.to_string())),
                        )
                        .col_expr(dispatch_projection::Column::UpdatedAt, Expr::value(now))
                        .filter(dispatch_projection::Column::DispatchId.eq(id_uuid))
                        .filter(dispatch_projection::Column::Status.eq(current.to_string()))
                        .exec(txn)
                        .await?;
                    if result.rows_affected == 0 {
                        return Err(StoreError::Conflict(format!(
                            "dispatch {id} status changed concurrently \
                             from {current}"
                        )));
                    }
                    events::Entity::insert(event_model).exec(txn).await?;
                    Ok(())
                })
            })
            .await?;
        Ok(())
    }
}
