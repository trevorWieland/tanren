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
    ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
    Select, TransactionTrait, sea_query::Expr,
};
#[cfg(feature = "test-hooks")]
use sea_orm::{DbBackend, QueryTrait, Statement};
use tanren_domain::{
    ActorContext, DispatchId, DispatchReadScope, DispatchView, EntityKind, Lane, StepId, StepView,
};

use crate::converters::{
    dispatch as dispatch_converters, events as event_converters, step as step_converters, validate,
};
use crate::entity::enums::{DispatchStatusModel, LaneModel, OutcomeModel, StepStatusModel};
use crate::entity::{dispatch_projection, events, step_projection};
use crate::errors::{StoreConflictClass, StoreError, StoreOperation, StoreResult};
use crate::params::{
    CancelDispatchParams, CreateDispatchParams, CreateDispatchWithInitialStepParams,
    DispatchFilter, DispatchQueryPage, DispatchSummaryQueryPage, UpdateDispatchStatusParams,
};
use crate::state_store_cancel::{
    CancelDispatchTxnInput, normalize_cancel_error, run_cancel_dispatch_transaction,
};
use crate::store::Store;
use crate::token_replay_store::consume_replay_guard_once;

/// Projection read / write interface for dispatches and steps.
#[async_trait]
pub trait StateStore: Send + Sync {
    /// Look up a dispatch by id.
    async fn get_dispatch(&self, id: &DispatchId) -> StoreResult<Option<DispatchView>>;

    /// Look up minimal ownership fields for cancel authorization checks.
    async fn get_dispatch_actor_context_for_cancel_auth(
        &self,
        id: &DispatchId,
    ) -> StoreResult<Option<ActorContext>>;

    /// Look up a dispatch by id within an actor-derived read scope.
    async fn get_dispatch_scoped(
        &self,
        id: &DispatchId,
        scope: DispatchReadScope,
    ) -> StoreResult<Option<DispatchView>>;

    /// Query dispatches by filter dimensions (status, lane, user, etc.).
    /// Reads the full JSON-backed [`DispatchView`]; for high-volume
    /// list traffic prefer
    /// [`query_dispatch_summaries`](Self::query_dispatch_summaries).
    async fn query_dispatches(&self, filter: &DispatchFilter) -> StoreResult<DispatchQueryPage>;

    /// Scalar-only list path — skips the per-row JSON decode. Wire
    /// contracts exposing list APIs should consume this.
    async fn query_dispatch_summaries(
        &self,
        filter: &DispatchFilter,
    ) -> StoreResult<DispatchSummaryQueryPage>;

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

    /// Create dispatch projection + initial step + both lifecycle
    /// events in one transaction.
    async fn create_dispatch_with_initial_step(
        &self,
        params: CreateDispatchWithInitialStepParams,
    ) -> StoreResult<()>;

    /// Atomically cancel a dispatch and all pending non-teardown
    /// steps, appending all companion events in one transaction.
    async fn cancel_dispatch(&self, params: CancelDispatchParams) -> StoreResult<u64>;

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

    async fn get_dispatch_actor_context_for_cancel_auth(
        &self,
        id: &DispatchId,
    ) -> StoreResult<Option<ActorContext>> {
        let row = dispatch_projection::Entity::find_by_id(id.into_uuid())
            .select_only()
            .column(dispatch_projection::Column::UserId)
            .column(dispatch_projection::Column::OrgId)
            .column(dispatch_projection::Column::ScopeProjectId)
            .column(dispatch_projection::Column::ScopeTeamId)
            .column(dispatch_projection::Column::ScopeApiKeyId)
            .into_tuple::<(
                uuid::Uuid,
                uuid::Uuid,
                Option<uuid::Uuid>,
                Option<uuid::Uuid>,
                Option<uuid::Uuid>,
            )>()
            .one(self.conn())
            .await?;

        Ok(row.map(
            |(user_id, org_id, project_id, team_id, api_key_id)| ActorContext {
                org_id: tanren_domain::OrgId::from_uuid(org_id),
                user_id: tanren_domain::UserId::from_uuid(user_id),
                team_id: team_id.map(tanren_domain::TeamId::from_uuid),
                api_key_id: api_key_id.map(tanren_domain::ApiKeyId::from_uuid),
                project_id: project_id.map(tanren_domain::ProjectId::from_uuid),
            },
        ))
    }

    async fn get_dispatch_scoped(
        &self,
        id: &DispatchId,
        scope: DispatchReadScope,
    ) -> StoreResult<Option<DispatchView>> {
        let row = apply_scoped_dispatch_filter(
            dispatch_projection::Entity::find_by_id(id.into_uuid()),
            scope,
        )
        .one(self.conn())
        .await?;
        row.map(dispatch_converters::model_to_view).transpose()
    }

    async fn query_dispatches(&self, filter: &DispatchFilter) -> StoreResult<DispatchQueryPage> {
        let page_size = filter.limit.min(crate::params::MAX_DISPATCH_QUERY_LIMIT);
        if page_size == 0 {
            return Ok(DispatchQueryPage {
                dispatches: Vec::new(),
                next_cursor: None,
            });
        }
        let rows = build_dispatch_query(filter, page_size)
            .all(self.conn())
            .await?;
        build_dispatch_query_page(rows, page_size)
    }

    async fn query_dispatch_summaries(
        &self,
        filter: &DispatchFilter,
    ) -> StoreResult<DispatchSummaryQueryPage> {
        crate::state_store_summary::query_dispatch_summaries_impl(self.conn(), filter).await
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
        let mut query = step_projection::Entity::find()
            .filter(step_projection::Column::Status.eq(StepStatusModel::Running));
        if let Some(lane) = lane {
            query = query.filter(step_projection::Column::Lane.eq(LaneModel::from(*lane)));
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

    async fn create_dispatch_with_initial_step(
        &self,
        params: CreateDispatchWithInitialStepParams,
    ) -> StoreResult<()> {
        validate::validate_create_dispatch_with_initial_step(&params)?;
        let replay_guard = params.replay_guard;
        let dispatch_row = dispatch_converters::params_to_active_model(&params.dispatch)?;
        let step_row = step_converters::enqueue_to_active_model(
            &params.initial_step,
            params.dispatch.created_at,
        )?;
        let creation_event =
            event_converters::envelope_to_active_model(&params.dispatch.creation_event)?;
        let step_event =
            event_converters::envelope_to_active_model(&params.initial_step.enqueue_event)?;

        self.conn()
            .transaction::<_, (), StoreError>(move |txn| {
                Box::pin(async move {
                    consume_replay_guard_once(txn, replay_guard).await?;
                    dispatch_projection::Entity::insert(dispatch_row)
                        .exec(txn)
                        .await?;
                    step_projection::Entity::insert(step_row).exec(txn).await?;
                    events::Entity::insert(creation_event).exec(txn).await?;
                    events::Entity::insert(step_event).exec(txn).await?;
                    Ok(())
                })
            })
            .await?;
        Ok(())
    }

    async fn cancel_dispatch(&self, params: CancelDispatchParams) -> StoreResult<u64> {
        validate::validate_cancel_dispatch(&params)?;

        let dispatch_id = params.dispatch_id;
        let dispatch_uuid = dispatch_id.into_uuid();
        let actor = params.actor;
        let reason = params.reason;
        let replay_guard = params.replay_guard;
        let now = Utc::now();
        let dispatch_event_timestamp = params.status_event.timestamp;
        let step_event_timestamp = dispatch_event_timestamp - chrono::Duration::microseconds(1);
        let dispatch_event_model =
            event_converters::envelope_to_active_model(&params.status_event)?;

        let result = self
            .conn()
            .transaction::<_, u64, StoreError>(move |txn| {
                Box::pin(run_cancel_dispatch_transaction(
                    txn,
                    CancelDispatchTxnInput {
                        dispatch_id,
                        dispatch_uuid,
                        actor,
                        reason,
                        replay_guard,
                        now,
                        step_event_timestamp,
                        dispatch_event_model,
                    },
                ))
            })
            .await
            .map_err(StoreError::from);

        result.map_err(normalize_cancel_error)
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
                        entity_kind: EntityKind::Dispatch,
                        id: id.to_string(),
                    })?;
                    let current = tanren_domain::DispatchStatus::from(row.status);
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
                            Expr::value(DispatchStatusModel::from(status)),
                        )
                        .col_expr(
                            dispatch_projection::Column::Outcome,
                            Expr::value(outcome.map(OutcomeModel::from)),
                        )
                        .col_expr(dispatch_projection::Column::UpdatedAt, Expr::value(now))
                        .filter(dispatch_projection::Column::DispatchId.eq(id_uuid))
                        .filter(
                            dispatch_projection::Column::Status
                                .eq(DispatchStatusModel::from(current)),
                        )
                        .exec(txn)
                        .await?;
                    if result.rows_affected == 0 {
                        return Err(StoreError::Conflict {
                            class: StoreConflictClass::Contention,
                            operation: StoreOperation::UpdateDispatchStatus,
                            reason: format!(
                                "dispatch {id} status changed concurrently from {current}"
                            ),
                        });
                    }
                    events::Entity::insert(event_model).exec(txn).await?;
                    Ok(())
                })
            })
            .await?;
        Ok(())
    }
}

pub(crate) type DispatchProjectionSelect = Select<dispatch_projection::Entity>;

pub(crate) fn build_dispatch_query(
    filter: &DispatchFilter,
    page_size: u64,
) -> DispatchProjectionSelect {
    let mut query = apply_common_dispatch_filters(dispatch_projection::Entity::find(), filter);
    if let Some(scope) = filter.read_scope {
        query = apply_scoped_dispatch_filter(query, scope);
    }

    query
        .order_by_desc(dispatch_projection::Column::CreatedAt)
        .order_by_desc(dispatch_projection::Column::DispatchId)
        .limit(page_size.saturating_add(1))
}

/// Build the exact scoped dispatch query statement used at runtime.
///
/// Exposed behind `test-hooks` so integration tests can run backend-native
/// `EXPLAIN` on the same predicate and ordering plan that `query_dispatches`
/// executes.
#[cfg(feature = "test-hooks")]
pub fn dispatch_query_statement_for_backend(
    filter: &DispatchFilter,
    page_size: u64,
    backend: DbBackend,
) -> Statement {
    let built = build_dispatch_query(filter, page_size).build(backend);
    match built.values {
        Some(values) => Statement::from_sql_and_values(backend, built.sql, values),
        None => Statement::from_string(backend, built.sql),
    }
}

pub(crate) fn apply_scoped_dispatch_filter(
    mut query: DispatchProjectionSelect,
    scope: DispatchReadScope,
) -> DispatchProjectionSelect {
    query = query.filter(dispatch_projection::Column::OrgId.eq(scope.org_id.into_uuid()));
    query = apply_scope_dimension_filter(
        query,
        dispatch_projection::Column::ScopeProjectId,
        scope.project_id.map(tanren_domain::ProjectId::into_uuid),
    );
    query = apply_scope_dimension_filter(
        query,
        dispatch_projection::Column::ScopeTeamId,
        scope.team_id.map(tanren_domain::TeamId::into_uuid),
    );
    apply_scope_dimension_filter(
        query,
        dispatch_projection::Column::ScopeApiKeyId,
        scope.api_key_id.map(tanren_domain::ApiKeyId::into_uuid),
    )
}

fn apply_scope_dimension_filter(
    query: DispatchProjectionSelect,
    column: dispatch_projection::Column,
    value: Option<uuid::Uuid>,
) -> DispatchProjectionSelect {
    match value {
        Some(value) => query.filter(Condition::any().add(column.is_null()).add(column.eq(value))),
        None => query.filter(column.is_null()),
    }
}

pub(crate) fn apply_common_dispatch_filters(
    mut query: DispatchProjectionSelect,
    filter: &DispatchFilter,
) -> DispatchProjectionSelect {
    if let Some(status) = filter.status {
        query =
            query.filter(dispatch_projection::Column::Status.eq(DispatchStatusModel::from(status)));
    }
    if let Some(lane) = filter.lane {
        query = query.filter(dispatch_projection::Column::Lane.eq(LaneModel::from(lane)));
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
    if let Some(cursor) = filter.cursor {
        query = query.filter(
            Condition::any()
                .add(dispatch_projection::Column::CreatedAt.lt(cursor.created_at))
                .add(
                    Condition::all()
                        .add(dispatch_projection::Column::CreatedAt.eq(cursor.created_at))
                        .add(
                            dispatch_projection::Column::DispatchId
                                .lt(cursor.dispatch_id.into_uuid()),
                        ),
                ),
        );
    }
    query
}

fn build_dispatch_query_page(
    mut rows: Vec<dispatch_projection::Model>,
    page_size: u64,
) -> StoreResult<DispatchQueryPage> {
    let page_size_usize = usize::try_from(page_size).unwrap_or(usize::MAX);
    let has_more = rows.len() > page_size_usize;
    if has_more {
        rows.truncate(page_size_usize);
    }

    let next_cursor = rows.last().map(|row| crate::params::DispatchCursor {
        created_at: row.created_at,
        dispatch_id: DispatchId::from_uuid(row.dispatch_id),
    });

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(dispatch_converters::model_to_view(row)?);
    }
    Ok(DispatchQueryPage {
        dispatches: out,
        next_cursor: if has_more { next_cursor } else { None },
    })
}
