use std::path::PathBuf;

use tanren_contract::{DriftPolicy, PreservationPolicy};
use tanren_identity_policy::ProjectId;

pub trait ProjectDriftContext: Send + Sync + std::fmt::Debug {
    fn resolve_repo_path(&self, project_id: ProjectId) -> Result<PathBuf, ProjectDriftError>;

    fn effective_drift_policy(&self, project_id: ProjectId) -> DriftPolicy;

    fn effective_preservation_policy(&self, project_id: ProjectId) -> PreservationPolicy;
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProjectDriftError {
    #[error("project not found: {0}")]
    ProjectNotFound(ProjectId),
    #[error("repository path not resolved for project: {0}")]
    RepoPathNotResolved(ProjectId),
}
