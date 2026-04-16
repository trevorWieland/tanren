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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::Utc;
    use tanren_contract::{
        Cli, ContractError, CreateDispatchRequest, DispatchCursorToken, DispatchListFilter,
        DispatchMode, ErrorCode, Phase,
    };
    use tanren_domain::{ActorContext, OrgId, UserId};
    use tanren_orchestrator::Orchestrator;
    use tanren_policy::PolicyEngine;
    use tanren_store::Store;
    use uuid::Uuid;

    use super::{DispatchService, convert_list_filter, format_cursor, parse_cursor};
    use crate::ReplayGuard;
    use crate::RequestContext;

    async fn setup_service() -> DispatchService<Store> {
        let store = Store::open_and_migrate("sqlite::memory:")
            .await
            .expect("store");
        let orchestrator = Orchestrator::new(store, PolicyEngine::new());
        DispatchService::new(orchestrator)
    }

    fn sample_context() -> RequestContext {
        RequestContext::new(ActorContext::new(OrgId::new(), UserId::new()))
    }

    fn sample_request() -> CreateDispatchRequest {
        CreateDispatchRequest {
            project: "proj".to_owned(),
            phase: Phase::DoTask,
            cli: Cli::Claude,
            branch: "main".to_owned(),
            spec_folder: "spec".to_owned(),
            workflow_id: "wf-1".to_owned(),
            mode: DispatchMode::Manual,
            timeout_secs: 300,
            environment_profile: "default".to_owned(),
            auth_mode: tanren_contract::AuthMode::ApiKey,
            gate_cmd: None,
            context: None,
            model: None,
            project_env: HashMap::new(),
            required_secrets: vec![],
            preserve_on_failure: false,
        }
    }

    fn sample_replay_guard() -> ReplayGuard {
        ReplayGuard::new(
            "tanren-tests".to_owned(),
            "tanren-cli".to_owned(),
            Uuid::now_v7().to_string(),
            10,
            20,
        )
    }

    #[test]
    fn convert_list_filter_rejects_limit_above_max() {
        let filter = DispatchListFilter {
            limit: Some(tanren_store::MAX_DISPATCH_QUERY_LIMIT + 1),
            ..DispatchListFilter::default()
        };
        let err = convert_list_filter(filter).expect_err("should fail");
        assert_eq!(err.code, ErrorCode::InvalidInput);
        assert!(err.message.contains("limit"));
    }

    #[test]
    fn convert_list_filter_rejects_zero_limit() {
        let filter = DispatchListFilter {
            limit: Some(0),
            ..DispatchListFilter::default()
        };
        let err = convert_list_filter(filter).expect_err("should fail");
        assert_eq!(err.code, ErrorCode::InvalidInput);
        assert!(err.message.contains("limit"));
    }

    #[test]
    fn cursor_roundtrip_is_stable() {
        let cursor = tanren_store::DispatchCursor {
            created_at: Utc::now(),
            dispatch_id: tanren_domain::DispatchId::new(),
        };
        let encoded = format_cursor(cursor);
        let decoded = parse_cursor(encoded);
        assert_eq!(decoded, cursor);
    }

    #[test]
    fn decode_cursor_token_rejects_bad_format() {
        let err = DispatchCursorToken::decode("bad").expect_err("should fail");
        assert!(
            matches!(err, ContractError::InvalidField { ref field, .. } if field == "cursor"),
            "expected invalid cursor field, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn service_create_rejects_duplicate_required_secrets() {
        let service = setup_service().await;
        let mut req = sample_request();
        req.required_secrets = vec!["API_KEY".to_owned(), "API_KEY".to_owned()];

        let err = service
            .create(&sample_context(), req, &sample_replay_guard())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, ErrorCode::InvalidInput);
        assert!(err.message.contains("required_secrets"));
    }

    #[tokio::test]
    async fn service_create_accepts_empty_project_env_values() {
        let service = setup_service().await;
        let mut req = sample_request();
        req.project_env = HashMap::from([("EMPTY".to_owned(), String::new())]);

        let created = service
            .create(&sample_context(), req, &sample_replay_guard())
            .await
            .expect("should create");
        assert_eq!(created.project_env_keys, vec!["EMPTY".to_owned()]);
    }

    #[tokio::test]
    async fn service_get_hides_unauthorized_dispatch_as_not_found() {
        let service = setup_service().await;
        let created = service
            .create(&sample_context(), sample_request(), &sample_replay_guard())
            .await
            .expect("create");

        let unauthorized = RequestContext::new(ActorContext::new(OrgId::new(), UserId::new()));
        let err = service
            .get(&unauthorized, created.dispatch_id)
            .await
            .expect_err("unauthorized actor should not see dispatch");
        assert_eq!(err.code, ErrorCode::NotFound);
    }

    #[tokio::test]
    async fn service_cancel_hides_unauthorized_dispatch_as_not_found() {
        let service = setup_service().await;
        let created = service
            .create(&sample_context(), sample_request(), &sample_replay_guard())
            .await
            .expect("create");

        let unauthorized = RequestContext::new(ActorContext::new(OrgId::new(), UserId::new()));
        let err = service
            .cancel(
                &unauthorized,
                tanren_contract::CancelDispatchRequest {
                    dispatch_id: created.dispatch_id,
                    reason: Some("unauthorized cancel".to_owned()),
                },
                &sample_replay_guard(),
            )
            .await
            .expect_err("unauthorized actor should not see dispatch");
        assert_eq!(err.code, ErrorCode::NotFound);
    }

    #[tokio::test]
    async fn service_cancel_nonexistent_dispatch_returns_not_found() {
        let service = setup_service().await;
        let err = service
            .cancel(
                &sample_context(),
                tanren_contract::CancelDispatchRequest {
                    dispatch_id: Uuid::now_v7(),
                    reason: Some("missing dispatch".to_owned()),
                },
                &sample_replay_guard(),
            )
            .await
            .expect_err("missing dispatch should fail");
        assert_eq!(err.code, ErrorCode::NotFound);
    }
}
