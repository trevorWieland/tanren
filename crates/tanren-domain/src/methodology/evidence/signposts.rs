//! Typed signposts.md frontmatter.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{SignpostId, SpecId, TaskId};
use crate::methodology::signpost::SignpostStatus;
use crate::validated::NonEmptyString;

use super::frontmatter::{FrontmatterError, join, parse_typed};

/// Typed `signposts.md` frontmatter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignpostsFrontmatter {
    pub kind: SignpostsKind,
    pub spec_id: SpecId,
    #[serde(default)]
    pub entries: Vec<SignpostEntry>,
}

/// Fixed discriminant tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SignpostsKind {
    Signposts,
}

/// One signpost entry in the frontmatter list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignpostEntry {
    pub id: SignpostId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    pub status: SignpostStatus,
    pub problem: NonEmptyString,
    pub evidence: NonEmptyString,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tried: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solution: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_affected: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SignpostsFrontmatter {
    /// Parse.
    ///
    /// # Errors
    /// See [`FrontmatterError`].
    pub fn parse_from_markdown(input: &str) -> Result<(Self, String), FrontmatterError> {
        parse_typed(input)
    }

    /// Render.
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

    #[test]
    fn roundtrip_stable() {
        let s = SignpostsFrontmatter {
            kind: SignpostsKind::Signposts,
            spec_id: SpecId::new(),
            entries: vec![SignpostEntry {
                id: SignpostId::new(),
                task_id: None,
                status: SignpostStatus::Unresolved,
                problem: NonEmptyString::try_new("Tricky race").expect("non-empty"),
                evidence: NonEmptyString::try_new("logs/race.txt").expect("non-empty"),
                tried: vec!["approach A".into()],
                solution: None,
                resolution: None,
                files_affected: vec![],
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }],
        };
        let doc = s.render_to_markdown("signposts body\n").expect("render");
        let (back, body) = SignpostsFrontmatter::parse_from_markdown(&doc).expect("parse");
        assert_eq!(back, s);
        assert_eq!(body, "signposts body\n");
    }
}
