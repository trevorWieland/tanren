//! Dispatch creation, query, and cancellation.
//!
//! Implements the orchestrator's dispatch CRUD operations against the
//! store traits. This module enforces the terminal-event emission rule:
//!
//! - `DispatchCompleted` only for `Outcome::Success`
//! - `DispatchFailed` for all non-success outcomes (`Fail | Blocked | Error | Timeout`)
//! - `DispatchCancelled` for user-initiated cancellation (separate path)

use chrono::Utc;
use tanren_domain::{
    ActorContext, CancelDispatch, CreateDispatch, DispatchId, DispatchSnapshot, DispatchStatus,
    DispatchView, DomainError, DomainEvent, EntityRef, EventEnvelope, EventId, FiniteF64,
    GraphRevision, Outcome, PolicyOutcome, ProvisionPayload, StepId, StepPayload, StepReadyState,
    StepType, cli_to_lane,
};
use tanren_store::{
    CancelDispatchParams, CreateDispatchParams, CreateDispatchWithInitialStepParams,
    DispatchFilter, DispatchQueryPage, EnqueueStepParams, EventStore, JobQueue, StateStore,
    UpdateDispatchStatusParams,
};
use uuid::Uuid;

use crate::Orchestrator;
use crate::error::OrchestratorError;

impl<S> Orchestrator<S>
where
    S: EventStore + JobQueue + StateStore,
{
    /// Create a new dispatch.
    ///
    /// 1. Mint IDs
    /// 2. Check policy
    /// 3. Build snapshot and event envelope
    /// 4. Persist dispatch projection
    /// 5. Enqueue the initial provision step
    /// 6. Return the created dispatch view (no read-after-write)
    pub async fn create_dispatch(
        &self,
        cmd: CreateDispatch,
    ) -> Result<DispatchView, OrchestratorError> {
        // Mint IDs first so the policy decision can reference the dispatch.
        let dispatch_id = DispatchId::new();

        // Policy check
        let decision = self.policy.check_dispatch_allowed(&cmd, dispatch_id)?;
        if decision.outcome == PolicyOutcome::Denied {
            return Err(OrchestratorError::PolicyDenied {
                decision: Box::new(decision),
            });
        }

        let now = Utc::now();
        let (params, view) = build_create_dispatch_artifacts(cmd, dispatch_id, now);

        self.store.create_dispatch_with_initial_step(params).await?;

        Ok(view)
    }

    /// Retrieve a dispatch by ID.
    pub async fn get_dispatch(
        &self,
        id: &DispatchId,
    ) -> Result<Option<DispatchView>, OrchestratorError> {
        Ok(self.store.get_dispatch(id).await?)
    }

    /// Retrieve a dispatch by ID, enforcing actor-scope read policy.
    pub async fn get_dispatch_for_actor(
        &self,
        id: &DispatchId,
        actor: &ActorContext,
    ) -> Result<Option<DispatchView>, OrchestratorError> {
        let view = self.store.get_dispatch(id).await?;
        if let Some(ref dispatch) = view {
            let decision = self.policy.check_dispatch_read_allowed(
                actor,
                &dispatch.actor,
                dispatch.dispatch_id,
            )?;
            if decision.outcome == PolicyOutcome::Denied {
                return Ok(None);
            }
        }
        Ok(view)
    }

    /// List dispatches matching the given filter.
    pub async fn list_dispatches(
        &self,
        filter: DispatchFilter,
    ) -> Result<DispatchQueryPage, OrchestratorError> {
        Ok(self.store.query_dispatches(&filter).await?)
    }

    /// List dispatches within the actor's policy-derived read scope.
    pub async fn list_dispatches_for_actor(
        &self,
        mut filter: DispatchFilter,
        actor: &ActorContext,
    ) -> Result<DispatchQueryPage, OrchestratorError> {
        filter.read_scope = Some(self.policy.dispatch_read_scope(actor));
        Ok(self.store.query_dispatches(&filter).await?)
    }

    /// Transition dispatch status to `Running`.
    pub async fn start_dispatch(&self, dispatch_id: DispatchId) -> Result<(), OrchestratorError> {
        let event = EventEnvelope::new(
            EventId::from_uuid(Uuid::now_v7()),
            Utc::now(),
            DomainEvent::DispatchStarted { dispatch_id },
        );
        self.store
            .update_dispatch_status(UpdateDispatchStatusParams {
                dispatch_id,
                status: DispatchStatus::Running,
                outcome: None,
                status_event: event,
            })
            .await?;
        Ok(())
    }

    /// Finalize a running dispatch using the single terminal-event rule.
    ///
    /// - `Outcome::Success` emits `DispatchCompleted`
    /// - all other outcomes emit `DispatchFailed`
    pub async fn finalize_dispatch(
        &self,
        dispatch_id: DispatchId,
        outcome: Outcome,
        total_duration_secs: FiniteF64,
        failed_step_id: Option<StepId>,
        failed_step_type: Option<StepType>,
        error: Option<String>,
    ) -> Result<(), OrchestratorError> {
        let (status, event) = terminal_status_event(
            dispatch_id,
            outcome,
            total_duration_secs,
            failed_step_id,
            failed_step_type,
            error,
        );
        self.store
            .update_dispatch_status(UpdateDispatchStatusParams {
                dispatch_id,
                status,
                outcome: Some(outcome),
                status_event: event,
            })
            .await?;
        Ok(())
    }

    /// Cancel a dispatch.
    ///
    /// 1. Verify the dispatch exists
    /// 2. Enforce cancel policy authorization
    /// 3. Atomically cancel pending steps + dispatch status/event
    pub async fn cancel_dispatch(&self, cmd: CancelDispatch) -> Result<(), OrchestratorError> {
        let view = self
            .store
            .get_dispatch(&cmd.dispatch_id)
            .await?
            .ok_or_else(|| {
                OrchestratorError::Domain(DomainError::NotFound {
                    entity: EntityRef::Dispatch(cmd.dispatch_id),
                })
            })?;

        let decision = self.policy.check_cancel_allowed(&cmd, &view.actor)?;
        if decision.outcome == PolicyOutcome::Denied {
            return Err(OrchestratorError::PolicyDenied {
                decision: Box::new(decision),
            });
        }

        let event = EventEnvelope::new(
            EventId::from_uuid(Uuid::now_v7()),
            Utc::now(),
            DomainEvent::DispatchCancelled {
                dispatch_id: cmd.dispatch_id,
                actor: cmd.actor.clone(),
                reason: cmd.reason.clone(),
            },
        );

        self.store
            .cancel_dispatch(CancelDispatchParams {
                dispatch_id: cmd.dispatch_id,
                actor: cmd.actor,
                reason: cmd.reason,
                status_event: event,
            })
            .await?;

        Ok(())
    }
}

fn build_create_dispatch_artifacts(
    cmd: CreateDispatch,
    dispatch_id: DispatchId,
    now: chrono::DateTime<Utc>,
) -> (CreateDispatchWithInitialStepParams, DispatchView) {
    let lane = cli_to_lane(&cmd.cli);
    let mode = cmd.mode;
    let actor = cmd.actor.clone();

    let snapshot = DispatchSnapshot {
        project: cmd.project,
        phase: cmd.phase,
        cli: cmd.cli,
        auth_mode: cmd.auth_mode,
        branch: cmd.branch,
        spec_folder: cmd.spec_folder,
        workflow_id: cmd.workflow_id,
        timeout: cmd.timeout,
        environment_profile: cmd.environment_profile,
        gate_cmd: cmd.gate_cmd,
        context: cmd.context,
        model: cmd.model,
        project_env: cmd.project_env.to_keys(),
        required_secrets: cmd.required_secrets,
        preserve_on_failure: cmd.preserve_on_failure,
        created_at: now,
    };

    let snapshot_for_view = snapshot.clone();
    let creation_event = EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        now,
        DomainEvent::DispatchCreated {
            dispatch_id,
            dispatch: Box::new(snapshot.clone()),
            mode,
            lane,
            actor: actor.clone(),
            graph_revision: GraphRevision::INITIAL,
        },
    );
    let dispatch_params = CreateDispatchParams {
        dispatch_id,
        mode,
        lane,
        dispatch: snapshot,
        actor: actor.clone(),
        graph_revision: GraphRevision::INITIAL,
        created_at: now,
        creation_event,
    };

    let step_id = StepId::new();
    let initial_step = EnqueueStepParams {
        dispatch_id,
        step_id,
        step_type: StepType::Provision,
        step_sequence: 0,
        lane: Some(lane),
        depends_on: vec![],
        graph_revision: GraphRevision::INITIAL,
        payload: StepPayload::Provision(Box::new(ProvisionPayload {
            dispatch: snapshot_for_view.clone(),
        })),
        ready_state: StepReadyState::Ready,
        enqueue_event: EventEnvelope::new(
            EventId::from_uuid(Uuid::now_v7()),
            now,
            DomainEvent::StepEnqueued {
                dispatch_id,
                step_id,
                step_type: StepType::Provision,
                step_sequence: 0,
                lane: Some(lane),
                depends_on: vec![],
                graph_revision: GraphRevision::INITIAL,
            },
        ),
    };

    let view = DispatchView {
        dispatch_id,
        mode,
        status: DispatchStatus::Pending,
        outcome: None,
        lane,
        dispatch: Box::new(snapshot_for_view),
        actor,
        graph_revision: GraphRevision::INITIAL,
        created_at: now,
        updated_at: now,
    };

    (
        CreateDispatchWithInitialStepParams {
            dispatch: dispatch_params,
            initial_step,
        },
        view,
    )
}

fn terminal_status_event(
    dispatch_id: DispatchId,
    outcome: Outcome,
    total_duration_secs: FiniteF64,
    failed_step_id: Option<StepId>,
    failed_step_type: Option<StepType>,
    error: Option<String>,
) -> (DispatchStatus, EventEnvelope) {
    let event = match outcome {
        Outcome::Success => DomainEvent::DispatchCompleted {
            dispatch_id,
            outcome,
            total_duration_secs,
        },
        Outcome::Fail | Outcome::Blocked | Outcome::Error | Outcome::Timeout => {
            DomainEvent::DispatchFailed {
                dispatch_id,
                outcome,
                failed_step_id,
                failed_step_type,
                error: error.unwrap_or_else(|| format!("dispatch terminated with {outcome}")),
            }
        }
    };

    let status = match outcome {
        Outcome::Success => DispatchStatus::Completed,
        Outcome::Fail | Outcome::Blocked | Outcome::Error | Outcome::Timeout => {
            DispatchStatus::Failed
        }
    };

    (
        status,
        EventEnvelope::new(EventId::from_uuid(Uuid::now_v7()), Utc::now(), event),
    )
}
