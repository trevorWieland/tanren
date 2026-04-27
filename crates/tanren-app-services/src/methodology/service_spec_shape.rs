//! Additional spec-shaping frontmatter methods for [`MethodologyService`].

use tanren_contract::methodology::{
    AckResponse, SetSpecExpectationsParams, SetSpecImplementationPlanParams,
    SetSpecMotivationsParams, SetSpecPlannedBehaviorsParams, SetSpecProblemStatementParams,
};
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::{
    MethodologyEvent, SpecFrontmatterPatch, SpecFrontmatterUpdated,
};
use tanren_domain::methodology::phase_id::PhaseId;

use super::capabilities::enforce;
use super::errors::{MethodologyResult, require_non_empty};
use super::service::MethodologyService;

impl MethodologyService {
    /// `set_spec_problem_statement`.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn set_spec_problem_statement(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: SetSpecProblemStatementParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::SpecFrontmatter, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "set_spec_problem_statement",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let problem_statement = require_non_empty(
                    "/problem_statement",
                    &params.problem_statement,
                    Some(4_000),
                )?;
                self.emit_event(
                    phase,
                    MethodologyEvent::SpecFrontmatterUpdated(SpecFrontmatterUpdated {
                        spec_id: params.spec_id,
                        patch: SpecFrontmatterPatch::SetProblemStatement { problem_statement },
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    /// `set_spec_motivations` — full-replace the list.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn set_spec_motivations(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: SetSpecMotivationsParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::SpecFrontmatter, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "set_spec_motivations",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                self.emit_event(
                    phase,
                    MethodologyEvent::SpecFrontmatterUpdated(SpecFrontmatterUpdated {
                        spec_id: params.spec_id,
                        patch: SpecFrontmatterPatch::SetMotivations {
                            motivations: params.motivations,
                        },
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    /// `set_spec_expectations` — full-replace the list.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn set_spec_expectations(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: SetSpecExpectationsParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::SpecFrontmatter, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "set_spec_expectations",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                self.emit_event(
                    phase,
                    MethodologyEvent::SpecFrontmatterUpdated(SpecFrontmatterUpdated {
                        spec_id: params.spec_id,
                        patch: SpecFrontmatterPatch::SetExpectations {
                            expectations: params.expectations,
                        },
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    /// `set_spec_planned_behaviors` — full-replace the list.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn set_spec_planned_behaviors(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: SetSpecPlannedBehaviorsParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::SpecFrontmatter, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "set_spec_planned_behaviors",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                self.emit_event(
                    phase,
                    MethodologyEvent::SpecFrontmatterUpdated(SpecFrontmatterUpdated {
                        spec_id: params.spec_id,
                        patch: SpecFrontmatterPatch::SetPlannedBehaviors {
                            planned_behaviors: params.planned_behaviors,
                        },
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    /// `set_spec_implementation_plan` — full-replace ordered steps.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn set_spec_implementation_plan(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: SetSpecImplementationPlanParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::SpecFrontmatter, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "set_spec_implementation_plan",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                self.emit_event(
                    phase,
                    MethodologyEvent::SpecFrontmatterUpdated(SpecFrontmatterUpdated {
                        spec_id: params.spec_id,
                        patch: SpecFrontmatterPatch::SetImplementationPlan {
                            implementation_plan: params.implementation_plan,
                        },
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }
}
