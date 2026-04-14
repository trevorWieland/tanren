//! Dispatch service — bridges contract types to orchestrator operations.
//!
//! This is the stable use-case API consumed by all transport interfaces
//! (CLI, API, MCP, TUI). It maps contract request/response types to and
//! from the domain, translating errors into wire-safe [`ErrorResponse`].

use tanren_contract::{
    CancelDispatchRequest, ContractError, CreateDispatchRequest, DispatchListFilter,
    DispatchListResponse, DispatchResponse, ErrorResponse,
};
use tanren_domain::{ActorContext, CancelDispatch, CreateDispatch, DispatchId};
use tanren_orchestrator::Orchestrator;
use tanren_store::{DispatchFilter, EventStore, JobQueue, StateStore};
use uuid::Uuid;

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

    /// Borrow the underlying orchestrator.
    pub fn orchestrator(&self) -> &Orchestrator<S> {
        &self.orchestrator
    }

    /// Create a new dispatch from a contract request.
    pub async fn create(
        &self,
        req: CreateDispatchRequest,
    ) -> Result<DispatchResponse, ErrorResponse> {
        let cmd = CreateDispatch::try_from(req).map_err(ErrorResponse::from)?;
        let view = self
            .orchestrator
            .create_dispatch(cmd)
            .await
            .map_err(|e| ErrorResponse::from(map_orchestrator_error(e)))?;
        Ok(DispatchResponse::from(view))
    }

    /// Get a dispatch by its UUID.
    pub async fn get(&self, dispatch_id: Uuid) -> Result<DispatchResponse, ErrorResponse> {
        let id = DispatchId::from_uuid(dispatch_id);
        let view = self
            .orchestrator
            .get_dispatch(&id)
            .await
            .map_err(|e| ErrorResponse::from(map_orchestrator_error(e)))?;
        match view {
            Some(v) => Ok(DispatchResponse::from(v)),
            None => Err(ErrorResponse::from(ContractError::NotFound {
                entity: "dispatch".to_owned(),
                id: dispatch_id.to_string(),
            })),
        }
    }

    /// List dispatches matching the given filter.
    pub async fn list(
        &self,
        filter: DispatchListFilter,
    ) -> Result<DispatchListResponse, ErrorResponse> {
        let store_filter = convert_list_filter(filter);
        let views = self
            .orchestrator
            .list_dispatches(store_filter)
            .await
            .map_err(|e| ErrorResponse::from(map_orchestrator_error(e)))?;
        Ok(DispatchListResponse {
            dispatches: views.into_iter().map(DispatchResponse::from).collect(),
        })
    }

    /// Cancel a dispatch.
    pub async fn cancel(&self, req: CancelDispatchRequest) -> Result<(), ErrorResponse> {
        let actor = ActorContext {
            org_id: tanren_domain::OrgId::from_uuid(req.org_id),
            user_id: tanren_domain::UserId::from_uuid(req.user_id),
            team_id: req.team_id.map(tanren_domain::TeamId::from_uuid),
            api_key_id: None,
            project_id: None,
        };
        let cmd = CancelDispatch {
            actor,
            dispatch_id: DispatchId::from_uuid(req.dispatch_id),
            reason: req.reason,
        };
        self.orchestrator
            .cancel_dispatch(cmd)
            .await
            .map_err(|e| ErrorResponse::from(map_orchestrator_error(e)))
    }
}

/// Convert a contract list filter to a store filter.
fn convert_list_filter(filter: DispatchListFilter) -> DispatchFilter {
    let mut f = DispatchFilter::new();
    f.status = filter.status;
    f.lane = filter.lane;
    f.project = filter.project;
    if let Some(limit) = filter.limit {
        f.limit = limit;
    }
    f.offset = filter.offset.unwrap_or(0);
    f
}
