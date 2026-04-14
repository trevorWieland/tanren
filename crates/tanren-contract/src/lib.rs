//! External contract representation and versioning for tanren interfaces.
//!
//! Depends on: `tanren-domain`
//!
//! # Responsibilities
//!
//! - Interface-safe request/response/error types with serde round-trip guarantees
//! - Validated conversion from contract requests to domain commands
//! - Error mapping from domain and store errors to wire-safe error responses
//!
//! # Design Rules
//!
//! - Serialization and schema only — no orchestration logic
//! - Every interface (CLI/API/MCP/TUI) derives from this contract
//! - Contract changes must be backwards-compatible or explicitly versioned

mod convert;
pub mod enums;
pub mod error;
pub mod request;
pub mod response;

pub use convert::{cancel_dispatch_from_request, create_dispatch_from_request};
pub use enums::{
    AuthMode, Cli, DispatchMode, DispatchStatus, Lane, Outcome, Phase, StepReadyState, StepStatus,
    StepType,
};
pub use error::{ContractError, ErrorCode, ErrorResponse};
pub use request::{
    CancelDispatchRequest, CreateDispatchRequest, DispatchCursorToken, DispatchListFilter,
    parse_project_env_entries,
};
pub use response::{DispatchListResponse, DispatchResponse, StepResponse};
