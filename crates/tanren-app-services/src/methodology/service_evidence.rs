//! Spec + demo frontmatter tool methods (§3.3, §3.4).
//!
//! Each method validates params at the boundary, emits a typed
//! `SpecFrontmatterUpdated` / `DemoFrontmatterUpdated` event, and
//! returns `()` on success. The orchestrator folds these events to
//! render the rendered `spec.md` / `demo.md` frontmatter — agents
//! never write those files directly.

use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::{
    DemoFrontmatterPatch, DemoFrontmatterUpdated, MethodologyEvent, SpecFrontmatterPatch,
    SpecFrontmatterUpdated,
};
use tanren_domain::methodology::phase_id::PhaseId;

use tanren_contract::methodology::{
    AckResponse, AddDemoStepParams, AddSpecAcceptanceCriterionParams, AppendDemoResultParams,
    MarkDemoStepSkipParams, SetSpecBaseBranchParams, SetSpecDemoEnvironmentParams,
    SetSpecDependenciesParams, SetSpecNonNegotiablesParams, SetSpecRelevanceContextParams,
    SetSpecTitleParams,
};

use super::capabilities::enforce;
use super::errors::{MethodologyResult, require_non_empty};
use super::service::MethodologyService;

impl MethodologyService {
    // ========= §3.3 Spec frontmatter ==========================

    /// `set_spec_title` — emit a `SpecFrontmatterUpdated` / `SetTitle` patch.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn set_spec_title(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: SetSpecTitleParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::SpecFrontmatter, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "set_spec_title",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let title = require_non_empty("/title", &params.title, Some(200))?;
                self.emit_event(
                    phase,
                    MethodologyEvent::SpecFrontmatterUpdated(SpecFrontmatterUpdated {
                        spec_id: params.spec_id,
                        patch: SpecFrontmatterPatch::SetTitle { title },
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    /// `set_spec_non_negotiables` — full-replace the list.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn set_spec_non_negotiables(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: SetSpecNonNegotiablesParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::SpecFrontmatter, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "set_spec_non_negotiables",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                self.emit_event(
                    phase,
                    MethodologyEvent::SpecFrontmatterUpdated(SpecFrontmatterUpdated {
                        spec_id: params.spec_id,
                        patch: SpecFrontmatterPatch::SetNonNegotiables {
                            items: params.items,
                        },
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    /// `add_spec_acceptance_criterion`.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn add_spec_acceptance_criterion(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: AddSpecAcceptanceCriterionParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::SpecFrontmatter, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "add_spec_acceptance_criterion",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                self.emit_event(
                    phase,
                    MethodologyEvent::SpecFrontmatterUpdated(SpecFrontmatterUpdated {
                        spec_id: params.spec_id,
                        patch: SpecFrontmatterPatch::AddAcceptanceCriterion {
                            criterion: params.criterion,
                        },
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    /// `set_spec_demo_environment`.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn set_spec_demo_environment(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: SetSpecDemoEnvironmentParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::SpecFrontmatter, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "set_spec_demo_environment",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                self.emit_event(
                    phase,
                    MethodologyEvent::SpecFrontmatterUpdated(SpecFrontmatterUpdated {
                        spec_id: params.spec_id,
                        patch: SpecFrontmatterPatch::SetDemoEnvironment {
                            demo_environment: params.demo_environment,
                        },
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    /// `set_spec_dependencies`.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn set_spec_dependencies(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: SetSpecDependenciesParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::SpecFrontmatter, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "set_spec_dependencies",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                self.emit_event(
                    phase,
                    MethodologyEvent::SpecFrontmatterUpdated(SpecFrontmatterUpdated {
                        spec_id: params.spec_id,
                        patch: SpecFrontmatterPatch::SetDependencies {
                            dependencies: params.dependencies,
                        },
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    /// `set_spec_base_branch`.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn set_spec_base_branch(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: SetSpecBaseBranchParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::SpecFrontmatter, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "set_spec_base_branch",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let branch = require_non_empty("/branch", &params.branch, Some(200))?;
                self.emit_event(
                    phase,
                    MethodologyEvent::SpecFrontmatterUpdated(SpecFrontmatterUpdated {
                        spec_id: params.spec_id,
                        patch: SpecFrontmatterPatch::SetBaseBranch { branch },
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    /// `set_spec_relevance_context`.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn set_spec_relevance_context(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: SetSpecRelevanceContextParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::SpecFrontmatter, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "set_spec_relevance_context",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                self.emit_event(
                    phase,
                    MethodologyEvent::SpecFrontmatterUpdated(SpecFrontmatterUpdated {
                        spec_id: params.spec_id,
                        patch: SpecFrontmatterPatch::SetRelevanceContext {
                            relevance_context: params.relevance_context,
                        },
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    // ========= §3.4 Demo frontmatter =========================

    /// `add_demo_step`.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn add_demo_step(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: AddDemoStepParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::DemoFrontmatter, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "add_demo_step",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let id = require_non_empty("/id", &params.id, Some(80))?;
                let description =
                    require_non_empty("/description", &params.description, Some(1000))?;
                let expected_observable =
                    require_non_empty("/expected_observable", &params.expected_observable, None)?;
                self.emit_event(
                    phase,
                    MethodologyEvent::DemoFrontmatterUpdated(DemoFrontmatterUpdated {
                        spec_id: params.spec_id,
                        patch: DemoFrontmatterPatch::AddStep {
                            id,
                            mode: params.mode,
                            description,
                            expected_observable,
                        },
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    /// `mark_demo_step_skip`.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn mark_demo_step_skip(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: MarkDemoStepSkipParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::DemoFrontmatter, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "mark_demo_step_skip",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let step_id = require_non_empty("/step_id", &params.step_id, Some(80))?;
                let reason = require_non_empty("/reason", &params.reason, None)?;
                self.emit_event(
                    phase,
                    MethodologyEvent::DemoFrontmatterUpdated(DemoFrontmatterUpdated {
                        spec_id: params.spec_id,
                        patch: DemoFrontmatterPatch::MarkStepSkip { step_id, reason },
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    /// `append_demo_result`. Supersedes the previous Lane-0.5 stub
    /// that only enforced the capability gate.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn append_demo_result(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: AppendDemoResultParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::DemoResults, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "append_demo_result",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let step_id = require_non_empty("/step_id", &params.step_id, Some(80))?;
                let observed = require_non_empty("/observed", &params.observed, None)?;
                self.emit_event(
                    phase,
                    MethodologyEvent::DemoFrontmatterUpdated(DemoFrontmatterUpdated {
                        spec_id: params.spec_id,
                        patch: DemoFrontmatterPatch::AppendResult {
                            step_id,
                            status: params.status,
                            observed,
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
