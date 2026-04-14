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
    CancelDispatch, CreateDispatch, DispatchId, DispatchSnapshot, DispatchStatus, DispatchView,
    DomainError, DomainEvent, EntityRef, EventEnvelope, EventId, GraphRevision, PolicyOutcome,
    ProvisionPayload, StepId, StepPayload, StepReadyState, StepType, cli_to_lane,
};
use tanren_store::{
    CancelPendingStepsParams, CreateDispatchParams, DispatchFilter, EnqueueStepParams, EventStore,
    JobQueue, StateStore, UpdateDispatchStatusParams,
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

        // Clone snapshot for the view and step payload before moving into params.
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

        let params = CreateDispatchParams {
            dispatch_id,
            mode,
            lane,
            dispatch: snapshot,
            actor: actor.clone(),
            graph_revision: GraphRevision::INITIAL,
            created_at: now,
            creation_event,
        };

        self.store.create_dispatch_projection(params).await?;

        // Enqueue the initial provision step.
        let step_id = StepId::new();
        let step_event = EventEnvelope::new(
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
        );
        self.store
            .enqueue_step(EnqueueStepParams {
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
                enqueue_event: step_event,
            })
            .await?;

        // Build the view directly from in-hand data — no read-after-write.
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

        Ok(view)
    }

    /// Retrieve a dispatch by ID.
    pub async fn get_dispatch(
        &self,
        id: &DispatchId,
    ) -> Result<Option<DispatchView>, OrchestratorError> {
        Ok(self.store.get_dispatch(id).await?)
    }

    /// List dispatches matching the given filter.
    pub async fn list_dispatches(
        &self,
        filter: DispatchFilter,
    ) -> Result<Vec<DispatchView>, OrchestratorError> {
        Ok(self.store.query_dispatches(&filter).await?)
    }

    /// Cancel a dispatch.
    ///
    /// 1. Verify the dispatch exists
    /// 2. Verify the transition to `Cancelled` is valid
    /// 3. Cancel all pending steps
    /// 4. Update dispatch status with `DispatchCancelled` event
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

        if !view.status.can_transition_to(DispatchStatus::Cancelled) {
            return Err(OrchestratorError::Domain(DomainError::InvalidTransition {
                from: view.status.to_string(),
                to: "cancelled".to_owned(),
                entity: EntityRef::Dispatch(cmd.dispatch_id),
            }));
        }

        // Cancel all pending steps first
        self.store
            .cancel_pending_steps(CancelPendingStepsParams {
                dispatch_id: cmd.dispatch_id,
                actor: Some(cmd.actor.clone()),
                reason: cmd.reason.clone(),
            })
            .await?;

        // Build the cancellation event
        let event = EventEnvelope::new(
            EventId::from_uuid(Uuid::now_v7()),
            Utc::now(),
            DomainEvent::DispatchCancelled {
                dispatch_id: cmd.dispatch_id,
                actor: cmd.actor,
                reason: cmd.reason,
            },
        );

        self.store
            .update_dispatch_status(UpdateDispatchStatusParams {
                dispatch_id: cmd.dispatch_id,
                status: DispatchStatus::Cancelled,
                outcome: None,
                status_event: event,
            })
            .await?;

        Ok(())
    }
}
