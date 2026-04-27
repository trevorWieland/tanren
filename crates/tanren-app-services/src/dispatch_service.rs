//! Dispatch service — bridges contract types to orchestrator operations.
//!
//! This is the stable use-case API consumed by all transport interfaces
//! (CLI, API, MCP, TUI). It maps contract request/response types to and
//! from the domain, translating errors into wire-safe [`ErrorResponse`].

use tanren_contract::{
    CancelDispatchRequest, ContractError, DispatchCursorToken, DispatchListFilter,
    DispatchListResponse, DispatchResponse, DispatchSummaryResponse, ErrorResponse,
    cancel_dispatch_from_request, create_dispatch_from_request,
};
use tanren_domain::DispatchId;
use tanren_orchestrator::Orchestrator;
use tanren_store::{
    DispatchCursor, DispatchFilter, EventStore, JobQueue, MAX_DISPATCH_QUERY_LIMIT, StateStore,
};
use uuid::Uuid;

use crate::ReplayGuard;
use crate::RequestContext;
use crate::error::map_orchestrator_error;

/// Application service for dispatch operations.
///
/// Generic over `S` — the store implementation. Trait bounds are on the
/// impl block, not the struct definition.
#[derive(Debug)]
pub struct DispatchService<S> {
    orchestrator: Orchestrator<S>,
}

impl<S> DispatchService<S>
where
    S: EventStore + JobQueue + StateStore,
{
    /// Create a new dispatch service wrapping the given orchestrator.
    pub fn new(orchestrator: Orchestrator<S>) -> Self {
        Self { orchestrator }
    }

    /// Create a new dispatch from a contract request.
    pub async fn create(
        &self,
        context: &RequestContext,
        req: tanren_contract::CreateDispatchRequest,
        replay_guard: &ReplayGuard,
    ) -> Result<DispatchResponse, ErrorResponse> {
        let cmd = create_dispatch_from_request(context.actor().clone(), req)
            .map_err(ErrorResponse::from)?;
        let view = self
            .orchestrator
            .create_dispatch(cmd, replay_guard.to_store_replay_guard())
            .await
            .map_err(map_orchestrator_error)?;
        Ok(DispatchResponse::from(view))
    }

    /// Get a dispatch by its UUID.
    pub async fn get(
        &self,
        context: &RequestContext,
        dispatch_id: Uuid,
    ) -> Result<DispatchResponse, ErrorResponse> {
        let id = DispatchId::from_uuid(dispatch_id);
        let view = self
            .orchestrator
            .get_dispatch_for_actor(&id, context.actor())
            .await
            .map_err(map_orchestrator_error)?;
        match view {
            Some(v) => Ok(DispatchResponse::from(v)),
            None => Err(ErrorResponse::from(ContractError::NotFound {
                entity: "dispatch".to_owned(),
                id: dispatch_id.to_string(),
            })),
        }
    }

    /// List dispatches matching the given filter.
    ///
    /// Uses the store's lean summary query path; the wire response
    /// contains the scalar-only [`DispatchSummaryResponse`] shape so
    /// no JSON snapshot decode runs per row.
    pub async fn list(
        &self,
        context: &RequestContext,
        filter: DispatchListFilter,
    ) -> Result<DispatchListResponse, ErrorResponse> {
        let store_filter = convert_list_filter(filter)?;
        let page = self
            .orchestrator
            .list_dispatch_summaries_for_actor(store_filter, context.actor())
            .await
            .map_err(map_orchestrator_error)?;
        Ok(DispatchListResponse {
            dispatches: page
                .summaries
                .into_iter()
                .map(DispatchSummaryResponse::from)
                .collect(),
            next_cursor: page.next_cursor.map(format_cursor),
        })
    }

    /// Cancel a dispatch.
    pub async fn cancel(
        &self,
        context: &RequestContext,
        req: CancelDispatchRequest,
        replay_guard: &ReplayGuard,
    ) -> Result<(), ErrorResponse> {
        let cmd = cancel_dispatch_from_request(context.actor().clone(), req)
            .map_err(ErrorResponse::from)?;
        self.orchestrator
            .cancel_dispatch(cmd, replay_guard.to_store_replay_guard())
            .await
            .map_err(map_orchestrator_error)
    }
}

/// Convert a contract list filter to a store filter.
fn convert_list_filter(filter: DispatchListFilter) -> Result<DispatchFilter, ErrorResponse> {
    let mut f = DispatchFilter::new();
    f.status = filter.status.map(Into::into);
    f.lane = filter.lane.map(Into::into);
    f.project = filter.project;
    if let Some(limit) = filter.limit {
        if limit == 0 {
            return Err(ErrorResponse::from(ContractError::InvalidField {
                field: "limit".to_owned(),
                reason: "must be >= 1".to_owned(),
            }));
        }
        if limit > MAX_DISPATCH_QUERY_LIMIT {
            return Err(ErrorResponse::from(ContractError::InvalidField {
                field: "limit".to_owned(),
                reason: format!("must be <= {MAX_DISPATCH_QUERY_LIMIT} (received {limit})"),
            }));
        }
        f.limit = limit;
    }
    if let Some(cursor) = filter.cursor {
        f.cursor = Some(parse_cursor(cursor));
    }
    Ok(f)
}

fn format_cursor(cursor: DispatchCursor) -> DispatchCursorToken {
    DispatchCursorToken::new(cursor.created_at, cursor.dispatch_id.into_uuid())
}

fn parse_cursor(token: DispatchCursorToken) -> DispatchCursor {
    DispatchCursor {
        created_at: token.created_at,
        dispatch_id: DispatchId::from_uuid(token.dispatch_id),
    }
}
