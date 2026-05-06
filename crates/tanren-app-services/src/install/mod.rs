pub mod drift;
pub mod manifest;
pub mod project_context;

pub use drift::{DriftEvalResult, evaluate_drift};
pub use manifest::{
    AssetCategory, AssetOwnership, EntryDriftPolicy, PRESERVED_INPUTS, PROJECTION_MANIFEST,
    PreservedInputEntry, ProjectionEntry,
};
pub use project_context::{ProjectDriftContext, ProjectDriftError};
