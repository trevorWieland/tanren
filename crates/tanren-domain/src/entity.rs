//! Typed entity references for event routing and policy resources.
//!
//! Replaces stringly-typed `(entity_type, entity_id)` tuples with a
//! discriminated enum so the set of valid entity kinds matches the
//! domain surface at compile time.

use serde::{Deserialize, Serialize};

use crate::ids::{
    ApiKeyId, DispatchId, FindingId, IssueId, LeaseId, OrgId, ProjectId, SignpostId, SpecId,
    StepId, TaskId, TeamId, UserId,
};

/// Typed reference to a top-level domain entity.
///
/// Used as the root routing metadata on [`crate::events::EventEnvelope`]
/// and as a reusable typed identifier elsewhere. Serialized as
/// `{"type": "...", "id": "..."}` so the discriminant and id are
/// always explicit on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "id", rename_all = "snake_case")]
pub enum EntityRef {
    Dispatch(DispatchId),
    Step(StepId),
    Lease(LeaseId),
    User(UserId),
    Org(OrgId),
    Team(TeamId),
    Project(ProjectId),
    ApiKey(ApiKeyId),
    Spec(SpecId),
    Task(TaskId),
    Finding(FindingId),
    Signpost(SignpostId),
    Issue(IssueId),
}

impl EntityRef {
    /// Return the discriminant kind without the inner id.
    #[must_use]
    pub const fn kind(&self) -> EntityKind {
        match self {
            Self::Dispatch(_) => EntityKind::Dispatch,
            Self::Step(_) => EntityKind::Step,
            Self::Lease(_) => EntityKind::Lease,
            Self::User(_) => EntityKind::User,
            Self::Org(_) => EntityKind::Org,
            Self::Team(_) => EntityKind::Team,
            Self::Project(_) => EntityKind::Project,
            Self::ApiKey(_) => EntityKind::ApiKey,
            Self::Spec(_) => EntityKind::Spec,
            Self::Task(_) => EntityKind::Task,
            Self::Finding(_) => EntityKind::Finding,
            Self::Signpost(_) => EntityKind::Signpost,
            Self::Issue(_) => EntityKind::Issue,
        }
    }
}

impl std::fmt::Display for EntityRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dispatch(id) => write!(f, "dispatch {id}"),
            Self::Step(id) => write!(f, "step {id}"),
            Self::Lease(id) => write!(f, "lease {id}"),
            Self::User(id) => write!(f, "user {id}"),
            Self::Org(id) => write!(f, "org {id}"),
            Self::Team(id) => write!(f, "team {id}"),
            Self::Project(id) => write!(f, "project {id}"),
            Self::ApiKey(id) => write!(f, "api_key {id}"),
            Self::Spec(id) => write!(f, "spec {id}"),
            Self::Task(id) => write!(f, "task {id}"),
            Self::Finding(id) => write!(f, "finding {id}"),
            Self::Signpost(id) => write!(f, "signpost {id}"),
            Self::Issue(id) => write!(f, "issue {id}"),
        }
    }
}

/// Discriminant tag identifying the kind of an [`EntityRef`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Dispatch,
    Step,
    Lease,
    User,
    Org,
    Team,
    Project,
    ApiKey,
    Spec,
    Task,
    Finding,
    Signpost,
    Issue,
}

impl std::fmt::Display for EntityKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dispatch => f.write_str("dispatch"),
            Self::Step => f.write_str("step"),
            Self::Lease => f.write_str("lease"),
            Self::User => f.write_str("user"),
            Self::Org => f.write_str("org"),
            Self::Team => f.write_str("team"),
            Self::Project => f.write_str("project"),
            Self::ApiKey => f.write_str("api_key"),
            Self::Spec => f.write_str("spec"),
            Self::Task => f.write_str("task"),
            Self::Finding => f.write_str("finding"),
            Self::Signpost => f.write_str("signpost"),
            Self::Issue => f.write_str("issue"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_ref_serde_roundtrip() {
        let id = DispatchId::new();
        let r = EntityRef::Dispatch(id);
        let json = serde_json::to_string(&r).expect("serialize");
        let back: EntityRef = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(r, back);
    }

    #[test]
    fn entity_ref_kind_matches_variant() {
        let d = EntityRef::Dispatch(DispatchId::new());
        assert_eq!(d.kind(), EntityKind::Dispatch);
        let s = EntityRef::Step(StepId::new());
        assert_eq!(s.kind(), EntityKind::Step);
        let l = EntityRef::Lease(LeaseId::new());
        assert_eq!(l.kind(), EntityKind::Lease);
    }

    #[test]
    fn entity_ref_wire_format() {
        let id = uuid::Uuid::parse_str("01966a00-0000-7000-8000-000000000001").expect("valid uuid");
        let r = EntityRef::Dispatch(DispatchId::from_uuid(id));
        let json = serde_json::to_string(&r).expect("serialize");
        assert!(json.contains("\"type\":\"dispatch\""));
        assert!(json.contains("\"id\":\"01966a00"));
    }
}
