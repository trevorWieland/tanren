//! Typed evidence schemas per
//! `docs/architecture/evidence-schemas.md` §2.
//!
//! Six shapes:
//!
//! - [`spec::SpecFrontmatter`] — `spec.md` frontmatter
//! - [`plan::PlanFrontmatter`] — `plan.md` (orchestrator-owned)
//! - [`demo::DemoFrontmatter`] — `demo.md` frontmatter
//! - [`audit::AuditFrontmatter`] — `audit.md` frontmatter
//! - [`signposts::SignpostsFrontmatter`] — `signposts.md` frontmatter
//! - [`investigation::InvestigationReport`] — `investigation-report.json`
//!
//! All parsers are deterministic, reject malformed input with typed
//! errors, and round-trip byte-for-byte when re-rendered without
//! intermediate mutation. See [`frontmatter`] for the shared
//! `---\n<yaml>\n---\n<body>` primitive.

pub mod audit;
pub mod demo;
pub mod frontmatter;
pub mod investigation;
pub mod plan;
pub mod signposts;
pub mod spec;

pub use audit::{AuditFrontmatter, AuditKind, AuditStatus};
pub use demo::{
    DemoEnvironmentProbe, DemoFrontmatter, DemoKind, DemoResult, DemoStatus, DemoStep, DemoStepMode,
};
pub use frontmatter::{
    EVIDENCE_SCHEMA_VERSION, EvidenceSchemaVersion, FrontmatterError, default_schema_version, join,
    parse_typed, split,
};
pub use investigation::{
    Confidence, InvestigationKind, InvestigationReport, InvestigationTrigger, RootCause,
    RootCauseCategory, SuggestedAction,
};
pub use plan::{PlanFrontmatter, PlanKind};
pub use signposts::{SignpostEntry, SignpostsFrontmatter, SignpostsKind};
pub use spec::{SpecFrontmatter, SpecKind};
