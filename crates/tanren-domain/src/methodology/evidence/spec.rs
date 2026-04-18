//! Typed spec.md frontmatter.
//!
//! Shape mirrors `docs/architecture/evidence-schemas.md` §2 verbatim.
//! Construction is mediated exclusively by `spec.frontmatter` tool
//! calls; this module owns the wire contract.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::SpecId;
use crate::methodology::spec::{DemoEnvironment, SpecDependencies, TouchedSymbol};
use crate::methodology::task::AcceptanceCriterion;
use crate::validated::NonEmptyString;

use super::frontmatter::{FrontmatterError, join, parse_typed};

/// Typed `spec.md` frontmatter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpecFrontmatter {
    pub kind: SpecKind,
    pub spec_id: SpecId,
    pub title: NonEmptyString,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub non_negotiables: Vec<NonEmptyString>,
    #[serde(default)]
    pub acceptance_criteria: Vec<AcceptanceCriterion>,
    #[serde(default)]
    pub demo_environment: DemoEnvironment,
    #[serde(default)]
    pub dependencies: SpecDependencies,
    pub base_branch: NonEmptyString,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub touched_symbols: Vec<TouchedSymbol>,
    pub created_at: DateTime<Utc>,
}

/// Fixed discriminant tag for the spec frontmatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SpecKind {
    Spec,
}

impl SpecFrontmatter {
    /// Parse a `---\n<yaml>\n---\n<body>` document.
    ///
    /// # Errors
    /// See [`FrontmatterError`].
    pub fn parse_from_markdown(input: &str) -> Result<(Self, String), FrontmatterError> {
        parse_typed(input)
    }

    /// Render to a canonical `---\n<yaml>\n---\n<body>` document.
    ///
    /// # Errors
    /// See [`FrontmatterError`].
    pub fn render_to_markdown(&self, body: &str) -> Result<String, FrontmatterError> {
        join(self, body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SpecFrontmatter {
        SpecFrontmatter {
            kind: SpecKind::Spec,
            spec_id: SpecId::new(),
            title: NonEmptyString::try_new("Example spec").expect("non-empty"),
            non_negotiables: vec![],
            acceptance_criteria: vec![],
            demo_environment: DemoEnvironment::default(),
            dependencies: SpecDependencies::default(),
            base_branch: NonEmptyString::try_new("main").expect("non-empty"),
            touched_symbols: vec![],
            created_at: Utc::now(),
        }
    }

    #[test]
    fn roundtrip_stable() {
        let s = sample();
        let doc = s
            .render_to_markdown("# Title\n\nBody prose.\n")
            .expect("render");
        let (parsed, body) = SpecFrontmatter::parse_from_markdown(&doc).expect("parse");
        assert_eq!(parsed, s);
        assert_eq!(body, "# Title\n\nBody prose.\n");
        // Second render is byte-for-byte identical.
        let doc2 = parsed.render_to_markdown(&body).expect("render2");
        assert_eq!(doc, doc2);
    }

    #[test]
    fn parse_rejects_unknown_frontmatter_keys() {
        let doc = format!(
            "---\nkind: spec\nspec_id: {}\ntitle: Example spec\nunknown_key: bad\nacceptance_criteria: []\ndemo_environment: {{}}\ndependencies: {{}}\nbase_branch: main\ncreated_at: 2026-01-01T00:00:00Z\n---\nbody\n",
            SpecId::new()
        );
        let err = SpecFrontmatter::parse_from_markdown(&doc).expect_err("unknown key must fail");
        let msg = err.to_string();
        assert!(msg.contains("unknown field"), "unexpected: {msg}");
    }
}
