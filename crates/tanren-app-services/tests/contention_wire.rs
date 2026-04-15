use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tanren_app_services::{DispatchService, ReplayGuard, RequestContext};
use tanren_contract::CancelDispatchRequest;
use tanren_domain::{
    ActorContext, AuthMode, ConfigKeys, DispatchId, DispatchMode, DispatchStatus, DispatchView,
    EventEnvelope, EventQueryResult, GraphRevision, Lane, NonEmptyString, OrgId, Phase,
    TimeoutSecs, UserId, read_scope_allows_dispatch_actor,
};
use tanren_orchestrator::Orchestrator;
use tanren_policy::PolicyEngine;
use tanren_store::{
    AckAndEnqueueParams, AckParams, CancelDispatchParams, CancelPendingStepsParams,
    CreateDispatchParams, CreateDispatchWithInitialStepParams, DequeueParams, DispatchFilter,
    DispatchQueryPage, EnqueueStepParams, EventFilter, EventStore, JobQueue, NackParams,
    QueuedStep, StateStore, StoreConflictClass, StoreError, StoreOperation,
    UpdateDispatchStatusParams,
};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone)]
struct ContentionStore {
    dispatch_id: DispatchId,
    actor: ActorContext,
    _marker: Arc<Mutex<()>>,
}

fn sample_replay_guard() -> ReplayGuard {
    ReplayGuard::new(
        "tanren-test".to_owned(),
        "tanren-cli".to_owned(),
        Uuid::now_v7().to_string(),
        1,
        2,
    )
}

#[async_trait]
impl EventStore for ContentionStore {
    async fn query_events(&self, _filter: &EventFilter) -> Result<EventQueryResult, StoreError> {
        Ok(EventQueryResult {
            events: vec![],
            total_count: None,
            has_more: false,
            next_cursor: None,
        })
    }

    async fn append_policy_decision_event(&self, _event: &EventEnvelope) -> Result<(), StoreError> {
        Ok(())
    }
}

#[async_trait]
impl JobQueue for ContentionStore {
    async fn enqueue_step(&self, _params: EnqueueStepParams) -> Result<(), StoreError> {
        Ok(())
    }

    async fn dequeue(&self, _params: DequeueParams) -> Result<Option<QueuedStep>, StoreError> {
        Ok(None)
    }

    async fn ack(&self, _params: AckParams) -> Result<(), StoreError> {
        Ok(())
    }

    async fn ack_and_enqueue(&self, _params: AckAndEnqueueParams) -> Result<(), StoreError> {
        Ok(())
    }

    async fn cancel_pending_steps(
        &self,
        _params: CancelPendingStepsParams,
    ) -> Result<u64, StoreError> {
        Ok(0)
    }

    async fn nack(&self, _params: NackParams) -> Result<(), StoreError> {
        Ok(())
    }

    async fn heartbeat_step(&self, _step_id: &tanren_domain::StepId) -> Result<(), StoreError> {
        Ok(())
    }

    async fn recover_stale_steps(&self, _timeout_secs: u64) -> Result<u64, StoreError> {
        Ok(0)
    }
}

#[async_trait]
impl StateStore for ContentionStore {
    async fn get_dispatch(&self, id: &DispatchId) -> Result<Option<DispatchView>, StoreError> {
        if *id != self.dispatch_id {
            return Ok(None);
        }

        Ok(Some(DispatchView {
            dispatch_id: self.dispatch_id,
            mode: DispatchMode::Manual,
            status: DispatchStatus::Pending,
            outcome: None,
            lane: Lane::Impl,
            dispatch: Box::new(tanren_domain::DispatchSnapshot {
                project: NonEmptyString::try_new("proj".to_owned()).expect("project"),
                phase: Phase::DoTask,
                cli: tanren_domain::Cli::Claude,
                auth_mode: AuthMode::ApiKey,
                branch: NonEmptyString::try_new("main".to_owned()).expect("branch"),
                spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("spec"),
                workflow_id: NonEmptyString::try_new("wf-1".to_owned()).expect("workflow"),
                timeout: TimeoutSecs::try_new(60).expect("timeout"),
                environment_profile: NonEmptyString::try_new("default".to_owned())
                    .expect("profile"),
                gate_cmd: None,
                context: None,
                model: None,
                project_env: ConfigKeys::default(),
                required_secrets: vec![],
                preserve_on_failure: false,
                created_at: Utc::now(),
            }),
            actor: self.actor.clone(),
            graph_revision: GraphRevision::INITIAL,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }))
    }

    async fn get_dispatch_scoped(
        &self,
        id: &DispatchId,
        scope: tanren_domain::DispatchReadScope,
    ) -> Result<Option<DispatchView>, StoreError> {
        Ok(self
            .get_dispatch(id)
            .await?
            .filter(|view| read_scope_allows_dispatch_actor(scope, &view.actor)))
    }

    async fn query_dispatches(
        &self,
        _filter: &DispatchFilter,
    ) -> Result<DispatchQueryPage, StoreError> {
        Ok(DispatchQueryPage {
            dispatches: vec![],
            next_cursor: None,
        })
    }

    async fn get_step(
        &self,
        _id: &tanren_domain::StepId,
    ) -> Result<Option<tanren_domain::StepView>, StoreError> {
        Ok(None)
    }

    async fn get_steps_for_dispatch(
        &self,
        _dispatch_id: &DispatchId,
    ) -> Result<Vec<tanren_domain::StepView>, StoreError> {
        Ok(vec![])
    }

    async fn count_running_steps(&self, _lane: Option<&Lane>) -> Result<u64, StoreError> {
        Ok(0)
    }

    async fn create_dispatch_projection(
        &self,
        _params: CreateDispatchParams,
    ) -> Result<(), StoreError> {
        Ok(())
    }

    async fn create_dispatch_with_initial_step(
        &self,
        _params: CreateDispatchWithInitialStepParams,
    ) -> Result<(), StoreError> {
        Ok(())
    }

    async fn cancel_dispatch(&self, _params: CancelDispatchParams) -> Result<u64, StoreError> {
        Err(StoreError::Conflict {
            class: StoreConflictClass::Contention,
            operation: StoreOperation::CancelDispatch,
            reason: "forced contention".to_owned(),
        })
    }

    async fn update_dispatch_status(
        &self,
        _params: UpdateDispatchStatusParams,
    ) -> Result<(), StoreError> {
        Ok(())
    }
}

#[tokio::test]
async fn cancel_contention_returns_contention_conflict_wire_code() {
    let actor = ActorContext::new(OrgId::new(), UserId::new());
    let store = ContentionStore {
        dispatch_id: DispatchId::new(),
        actor: actor.clone(),
        _marker: Arc::new(Mutex::new(())),
    };
    let orchestrator = Orchestrator::new(store.clone(), PolicyEngine::new());
    let service = DispatchService::new(orchestrator);

    let err = service
        .cancel(
            &RequestContext::new(actor),
            CancelDispatchRequest {
                dispatch_id: store.dispatch_id.into_uuid(),
                reason: Some("stop".to_owned()),
            },
            &sample_replay_guard(),
        )
        .await
        .expect_err("cancel should fail");

    assert_eq!(err.code, tanren_contract::ErrorCode::ContentionConflict);
}
