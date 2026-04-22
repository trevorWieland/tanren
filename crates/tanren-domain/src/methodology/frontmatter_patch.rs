//! Typed frontmatter mutation events (spec + demo).
//!
//! Split out of [`crate::methodology::events`] to keep that file
//! within the workspace's per-file line budget. The `MethodologyEvent`
//! enum references these types; folding a stream reconstructs the
//! current `spec.md` / `demo.md` frontmatter state.

use serde::{Deserialize, Serialize};

use crate::SpecId;
use crate::methodology::evidence::demo::{DemoStatus, DemoStepMode};
use crate::methodology::spec::{DemoEnvironment, SpecDependencies, SpecRelevanceContext};
use crate::methodology::task::AcceptanceCriterion;
use crate::validated::NonEmptyString;

/// A spec-frontmatter mutation. Replay folds these into the spec's
/// current frontmatter state; the orchestrator renders `spec.md`
/// from that state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecFrontmatterUpdated {
    pub spec_id: SpecId,
    pub patch: SpecFrontmatterPatch,
}

/// One typed patch to spec frontmatter. Mirrors the six `§3.3` tools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SpecFrontmatterPatch {
    SetTitle {
        title: NonEmptyString,
    },
    SetProblemStatement {
        problem_statement: NonEmptyString,
    },
    SetMotivations {
        motivations: Vec<NonEmptyString>,
    },
    SetExpectations {
        expectations: Vec<NonEmptyString>,
    },
    SetPlannedBehaviors {
        planned_behaviors: Vec<NonEmptyString>,
    },
    SetImplementationPlan {
        implementation_plan: Vec<NonEmptyString>,
    },
    SetNonNegotiables {
        items: Vec<NonEmptyString>,
    },
    AddAcceptanceCriterion {
        criterion: AcceptanceCriterion,
    },
    SetDemoEnvironment {
        demo_environment: DemoEnvironment,
    },
    SetDependencies {
        dependencies: SpecDependencies,
    },
    SetBaseBranch {
        branch: NonEmptyString,
    },
    SetRelevanceContext {
        relevance_context: SpecRelevanceContext,
    },
}

/// A demo-frontmatter mutation. Same pattern as spec frontmatter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DemoFrontmatterUpdated {
    pub spec_id: SpecId,
    pub patch: DemoFrontmatterPatch,
}

/// One typed patch to demo frontmatter. Mirrors the three `§3.4`
/// tools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DemoFrontmatterPatch {
    AddStep {
        id: NonEmptyString,
        mode: DemoStepMode,
        description: NonEmptyString,
        expected_observable: NonEmptyString,
    },
    MarkStepSkip {
        step_id: NonEmptyString,
        reason: NonEmptyString,
    },
    AppendResult {
        step_id: NonEmptyString,
        status: DemoStatus,
        observed: NonEmptyString,
    },
}
