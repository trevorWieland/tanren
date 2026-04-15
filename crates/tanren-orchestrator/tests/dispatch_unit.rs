//! Unit-level orchestrator tests with a recording store.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tanren_domain::{
    ActorContext, AuthMode, CancelDispatch, Cli, ConfigEnv, CreateDispatch, DispatchId,
    DispatchMode, DispatchStatus, DispatchView, DomainError, EntityKind, EntityRef,
    EventQueryResult, FiniteF64, Lane, NonEmptyString, OrgId, Outcome, Phase, StepReadyState,
    StepType, TimeoutSecs, UserId, cli_to_lane, read_scope_allows_dispatch_actor,
};
use tanren_orchestrator::{Orchestrator, OrchestratorError};
use tanren_policy::PolicyEngine;
use tanren_store::{
    AckAndEnqueueParams, AckParams, CancelDispatchParams, CancelPendingStepsParams,
    CreateDispatchParams, CreateDispatchWithInitialStepParams, DequeueParams, DispatchCursor,
    DispatchFilter, DispatchQueryPage, EnqueueStepParams, EventFilter, EventStore, JobQueue,
    NackParams, QueuedStep, StateStore, StoreConflictClass, StoreError, StoreOperation,
    UpdateDispatchStatusParams,
};
use tokio::sync::Mutex;
#[derive(Debug, Default)]
struct RecordingState {
    created_dispatches: Vec<CreateDispatchWithInitialStepParams>,
    cancelled_dispatches: Vec<CancelDispatchParams>,
    dispatch_status_updates: Vec<UpdateDispatchStatusParams>,
    policy_decision_events: Vec<tanren_domain::EventEnvelope>,
    dispatches: HashMap<DispatchId, DispatchView>,
}

#[derive(Debug, Clone, Default)]
struct RecordingStore {
    state: Arc<Mutex<RecordingState>>,
}

impl RecordingStore {
    async fn snapshot(&self) -> RecordingStateSnapshot {
        let state = self.state.lock().await;
        RecordingStateSnapshot {
            created_dispatches: state.created_dispatches.clone(),
            cancelled_dispatches: state.cancelled_dispatches.clone(),
            dispatch_status_updates: state.dispatch_status_updates.clone(),
            policy_decision_events: state.policy_decision_events.clone(),
            dispatches: state.dispatches.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct RecordingStateSnapshot {
    created_dispatches: Vec<CreateDispatchWithInitialStepParams>,
    cancelled_dispatches: Vec<CancelDispatchParams>,
    dispatch_status_updates: Vec<UpdateDispatchStatusParams>,
    policy_decision_events: Vec<tanren_domain::EventEnvelope>,
    dispatches: HashMap<DispatchId, DispatchView>,
}

#[async_trait]
impl EventStore for RecordingStore {
    async fn query_events(&self, _filter: &EventFilter) -> Result<EventQueryResult, StoreError> {
        Ok(EventQueryResult {
            events: vec![],
            total_count: None,
            has_more: false,
            next_cursor: None,
        })
    }

    async fn append_policy_decision_event(
        &self,
        event: &tanren_domain::EventEnvelope,
    ) -> Result<(), StoreError> {
        let mut state = self.state.lock().await;
        state.policy_decision_events.push(event.clone());
        Ok(())
    }
}

#[async_trait]
impl JobQueue for RecordingStore {
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
impl StateStore for RecordingStore {
    async fn get_dispatch(&self, id: &DispatchId) -> Result<Option<DispatchView>, StoreError> {
        let state = self.state.lock().await;
        Ok(state.dispatches.get(id).cloned())
    }

    async fn get_dispatch_scoped(
        &self,
        id: &DispatchId,
        scope: tanren_domain::DispatchReadScope,
    ) -> Result<Option<DispatchView>, StoreError> {
        let state = self.state.lock().await;
        Ok(state
            .dispatches
            .get(id)
            .cloned()
            .filter(|view| read_scope_allows_dispatch_actor(scope, &view.actor)))
    }

    async fn query_dispatches(
        &self,
        filter: &DispatchFilter,
    ) -> Result<DispatchQueryPage, StoreError> {
        let state = self.state.lock().await;
        let mut dispatches: Vec<DispatchView> = state.dispatches.values().cloned().collect();
        if let Some(status) = filter.status {
            dispatches.retain(|view| view.status == status);
        }
        if let Some(lane) = filter.lane {
            dispatches.retain(|view| view.lane == lane);
        }
        if let Some(ref project) = filter.project {
            dispatches.retain(|view| view.dispatch.project.as_str() == project.as_str());
        }
        if let Some(user_id) = filter.user_id {
            dispatches.retain(|view| view.actor.user_id == user_id);
        }
        if let Some(scope) = filter.read_scope {
            dispatches.retain(|view| read_scope_allows_dispatch_actor(scope, &view.actor));
        }
        dispatches.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.dispatch_id.into_uuid().cmp(&a.dispatch_id.into_uuid()))
        });

        if let Some(cursor) = filter.cursor {
            dispatches.retain(|view| {
                view.created_at < cursor.created_at
                    || (view.created_at == cursor.created_at
                        && view.dispatch_id.into_uuid() < cursor.dispatch_id.into_uuid())
            });
        }

        let limit = usize::try_from(filter.limit).unwrap_or(usize::MAX);
        let mut next_cursor = None;
        if dispatches.len() > limit {
            let cursor_row = dispatches
                .get(limit.saturating_sub(1))
                .expect("limit > 0 guarded by default filter");
            next_cursor = Some(DispatchCursor {
                created_at: cursor_row.created_at,
                dispatch_id: cursor_row.dispatch_id,
            });
            dispatches.truncate(limit);
        }

        Ok(DispatchQueryPage {
            dispatches,
            next_cursor,
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
        Err(StoreError::Conflict {
            class: StoreConflictClass::Contention,
            operation: StoreOperation::UpdateDispatchStatus,
            reason: "unexpected path: create_dispatch_projection".to_owned(),
        })
    }

    async fn create_dispatch_with_initial_step(
        &self,
        params: CreateDispatchWithInitialStepParams,
    ) -> Result<(), StoreError> {
        let mut state = self.state.lock().await;
        state.created_dispatches.push(params.clone());
        let dispatch = &params.dispatch;
        let view = DispatchView {
            dispatch_id: dispatch.dispatch_id,
            mode: dispatch.mode,
            status: DispatchStatus::Pending,
            outcome: None,
            lane: dispatch.lane,
            dispatch: Box::new(dispatch.dispatch.clone()),
            actor: dispatch.actor.clone(),
            graph_revision: dispatch.graph_revision,
            created_at: dispatch.created_at,
            updated_at: dispatch.created_at,
        };
        state.dispatches.insert(dispatch.dispatch_id, view);
        Ok(())
    }

    async fn cancel_dispatch(&self, params: CancelDispatchParams) -> Result<u64, StoreError> {
        let mut state = self.state.lock().await;
        let view = state
            .dispatches
            .get_mut(&params.dispatch_id)
            .ok_or(StoreError::NotFound {
                entity_kind: EntityKind::Dispatch,
                id: params.dispatch_id.to_string(),
            })?;
        view.status = DispatchStatus::Cancelled;
        view.updated_at = Utc::now();
        state.cancelled_dispatches.push(params);
        Ok(1)
    }

    async fn update_dispatch_status(
        &self,
        params: UpdateDispatchStatusParams,
    ) -> Result<(), StoreError> {
        let mut state = self.state.lock().await;
        let view = state
            .dispatches
            .get_mut(&params.dispatch_id)
            .ok_or(StoreError::NotFound {
                entity_kind: EntityKind::Dispatch,
                id: params.dispatch_id.to_string(),
            })?;
        if !view.status.can_transition_to(params.status) {
            return Err(StoreError::InvalidTransition {
                entity: format!("dispatch {}", params.dispatch_id),
                from: view.status.to_string(),
                to: params.status.to_string(),
            });
        }
        view.status = params.status;
        view.outcome = params.outcome;
        view.updated_at = Utc::now();
        state.dispatch_status_updates.push(params);
        Ok(())
    }
}
fn sample_actor() -> ActorContext {
    ActorContext::new(OrgId::new(), UserId::new())
}

fn sample_command(actor: ActorContext) -> CreateDispatch {
    CreateDispatch {
        actor,
        project: NonEmptyString::try_new("test-project".to_owned()).expect("non-empty"),
        phase: Phase::DoTask,
        cli: Cli::Claude,
        auth_mode: AuthMode::ApiKey,
        branch: NonEmptyString::try_new("main".to_owned()).expect("non-empty"),
        spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("non-empty"),
        workflow_id: NonEmptyString::try_new("wf-1".to_owned()).expect("non-empty"),
        mode: DispatchMode::Manual,
        timeout: TimeoutSecs::try_new(60).expect("positive"),
        environment_profile: NonEmptyString::try_new("default".to_owned()).expect("non-empty"),
        gate_cmd: Some("cargo test".to_owned()),
        context: Some("context".to_owned()),
        model: Some("claude-4".to_owned()),
        project_env: ConfigEnv::from(HashMap::from([(
            "API_URL".to_owned(),
            "https://example.com".to_owned(),
        )])),
        required_secrets: vec!["OPENAI_API_KEY".to_owned()],
        preserve_on_failure: true,
    }
}

#[tokio::test]
async fn create_dispatch_records_atomic_store_operation() {
    let store = RecordingStore::default();
    let orch = Orchestrator::new(store.clone(), PolicyEngine::new());

    let created = orch
        .create_dispatch(sample_command(sample_actor()))
        .await
        .expect("create");
    let snapshot = store.snapshot().await;

    assert_eq!(snapshot.created_dispatches.len(), 1);
    let params = &snapshot.created_dispatches[0];
    assert_eq!(params.dispatch.dispatch_id, created.dispatch_id);
    assert_eq!(params.dispatch.lane, cli_to_lane(&Cli::Claude));
    assert_eq!(params.initial_step.step_type, StepType::Provision);
    assert_eq!(params.initial_step.step_sequence, 0);
    assert_eq!(params.initial_step.ready_state, StepReadyState::Ready);

    let stored = snapshot
        .dispatches
        .get(&created.dispatch_id)
        .expect("dispatch should be stored");
    assert_eq!(stored.dispatch.required_secrets, vec!["OPENAI_API_KEY"]);
    assert!(stored.dispatch.preserve_on_failure);
}

#[tokio::test]
async fn cancel_dispatch_records_cancel_params_and_updates_status() {
    let store = RecordingStore::default();
    let orch = Orchestrator::new(store.clone(), PolicyEngine::new());
    let actor = sample_actor();

    let created = orch
        .create_dispatch(sample_command(actor.clone()))
        .await
        .expect("create");

    orch.cancel_dispatch(CancelDispatch {
        actor,
        dispatch_id: created.dispatch_id,
        reason: Some("user cancelled".to_owned()),
    })
    .await
    .expect("cancel");

    let snapshot = store.snapshot().await;
    assert_eq!(snapshot.cancelled_dispatches.len(), 1);
    assert_eq!(
        snapshot.cancelled_dispatches[0].reason.as_deref(),
        Some("user cancelled")
    );

    let stored = snapshot
        .dispatches
        .get(&created.dispatch_id)
        .expect("dispatch should be stored");
    assert_eq!(stored.status, DispatchStatus::Cancelled);
}

#[tokio::test]
async fn finalize_dispatch_enforces_single_terminal_failed_path() {
    let store = RecordingStore::default();
    let orch = Orchestrator::new(store.clone(), PolicyEngine::new());

    let created = orch
        .create_dispatch(sample_command(sample_actor()))
        .await
        .expect("create");
    orch.start_dispatch(created.dispatch_id)
        .await
        .expect("start");
    orch.finalize_dispatch(
        created.dispatch_id,
        Outcome::Error,
        FiniteF64::try_new(3.1).expect("finite"),
        None,
        None,
        Some("failed".to_owned()),
    )
    .await
    .expect("finalize");

    let snapshot = store.snapshot().await;
    let updates = snapshot.dispatch_status_updates;
    assert_eq!(updates.len(), 2, "running + failed updates expected");
    assert_eq!(updates[0].status, DispatchStatus::Running);
    assert_eq!(updates[1].status, DispatchStatus::Failed);
    assert_eq!(updates[1].outcome, Some(Outcome::Error));
    assert!(matches!(
        updates[1].status_event.payload,
        tanren_domain::DomainEvent::DispatchFailed {
            outcome: Outcome::Error,
            ..
        }
    ));
}

#[tokio::test]
async fn cancel_nonexistent_dispatch_returns_not_found() {
    let store = RecordingStore::default();
    let orch = Orchestrator::new(store.clone(), PolicyEngine::new());

    let err = orch
        .cancel_dispatch(CancelDispatch {
            actor: sample_actor(),
            dispatch_id: DispatchId::new(),
            reason: None,
        })
        .await
        .expect_err("cancel should fail");
    assert!(matches!(
        err,
        OrchestratorError::Domain(DomainError::NotFound {
            entity: EntityRef::Dispatch(_),
        })
    ));
    let snapshot = store.snapshot().await;
    assert_eq!(snapshot.policy_decision_events.len(), 1);
    assert!(matches!(
        snapshot.policy_decision_events[0].payload,
        tanren_domain::DomainEvent::PolicyDecision { ref decision, .. }
            if decision.reason_code == Some(tanren_domain::PolicyReasonCode::CancelDispatchNotFound)
    ));
    assert!(snapshot.cancelled_dispatches.is_empty());
}

#[tokio::test]
async fn cancel_dispatch_hides_actor_scope_mismatch_as_not_found() {
    let store = RecordingStore::default();
    let orch = Orchestrator::new(store.clone(), PolicyEngine::new());
    let created = orch
        .create_dispatch(sample_command(sample_actor()))
        .await
        .expect("create");

    let err = orch
        .cancel_dispatch(CancelDispatch {
            actor: ActorContext::new(OrgId::new(), UserId::new()),
            dispatch_id: created.dispatch_id,
            reason: Some("mismatch".to_owned()),
        })
        .await
        .expect_err("cancel should fail");
    assert!(
        matches!(
            err,
            OrchestratorError::Domain(DomainError::NotFound {
                entity: EntityRef::Dispatch(_),
            })
        ),
        "expected hidden not-found, got: {err:?}"
    );

    let snapshot = store.snapshot().await;
    assert_eq!(snapshot.policy_decision_events.len(), 1);
    assert!(matches!(
        snapshot.policy_decision_events[0].payload,
        tanren_domain::DomainEvent::PolicyDecision { ref decision, .. }
            if decision.reason_code == Some(tanren_domain::PolicyReasonCode::CancelOrgMismatch)
    ));
    assert!(snapshot.cancelled_dispatches.is_empty());
}

#[tokio::test]
async fn list_dispatches_applies_filters_without_store_sql_knowledge() {
    let store = RecordingStore::default();
    let orch = Orchestrator::new(store, PolicyEngine::new());
    let actor = sample_actor();

    let created = orch
        .create_dispatch(sample_command(actor.clone()))
        .await
        .expect("create");

    let mut filter = DispatchFilter::new();
    filter.status = Some(DispatchStatus::Pending);
    let page = orch
        .list_dispatches_for_actor(filter, &actor)
        .await
        .expect("list");

    assert_eq!(page.dispatches.len(), 1);
    assert_eq!(page.dispatches[0].dispatch_id, created.dispatch_id);
    assert_eq!(page.dispatches[0].status, DispatchStatus::Pending);
}
