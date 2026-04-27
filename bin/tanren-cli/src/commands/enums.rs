//! CLI-local wrapper enums for clap parsing.
//!
//! `tanren-contract` is transport-neutral by design: schema only, no transport
//! coupling. Clap's `ValueEnum` derive is a transport concern, so it lives here
//! in the CLI binary rather than on the contract enums themselves. Each wrapper
//! is a thin newtype with a one-to-one `From` mapping back to the contract enum
//! so handler code can `.into()` at the seam.
//!
//! The special renames (`opencode`, `oauth`) match the canonical wire tags and
//! the Display impls in `tanren_domain::status::enums`.

use clap::ValueEnum;
use tanren_contract::{AuthMode, Cli, DispatchMode, DispatchStatus, Lane, Phase};

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum DispatchModeArg {
    Auto,
    Manual,
}

impl From<DispatchModeArg> for DispatchMode {
    fn from(arg: DispatchModeArg) -> Self {
        match arg {
            DispatchModeArg::Auto => Self::Auto,
            DispatchModeArg::Manual => Self::Manual,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum DispatchStatusArg {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl From<DispatchStatusArg> for DispatchStatus {
    fn from(arg: DispatchStatusArg) -> Self {
        match arg {
            DispatchStatusArg::Pending => Self::Pending,
            DispatchStatusArg::Running => Self::Running,
            DispatchStatusArg::Completed => Self::Completed,
            DispatchStatusArg::Failed => Self::Failed,
            DispatchStatusArg::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum LaneArg {
    Impl,
    Audit,
    Gate,
}

impl From<LaneArg> for Lane {
    fn from(arg: LaneArg) -> Self {
        match arg {
            LaneArg::Impl => Self::Impl,
            LaneArg::Audit => Self::Audit,
            LaneArg::Gate => Self::Gate,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum PhaseArg {
    DoTask,
    AuditTask,
    RunDemo,
    AuditSpec,
    Investigate,
    Gate,
    Setup,
    Cleanup,
}

impl From<PhaseArg> for Phase {
    fn from(arg: PhaseArg) -> Self {
        match arg {
            PhaseArg::DoTask => Self::DoTask,
            PhaseArg::AuditTask => Self::AuditTask,
            PhaseArg::RunDemo => Self::RunDemo,
            PhaseArg::AuditSpec => Self::AuditSpec,
            PhaseArg::Investigate => Self::Investigate,
            PhaseArg::Gate => Self::Gate,
            PhaseArg::Setup => Self::Setup,
            PhaseArg::Cleanup => Self::Cleanup,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum CliArg {
    Claude,
    Codex,
    #[value(name = "opencode")]
    OpenCode,
    Bash,
}

impl From<CliArg> for Cli {
    fn from(arg: CliArg) -> Self {
        match arg {
            CliArg::Claude => Self::Claude,
            CliArg::Codex => Self::Codex,
            CliArg::OpenCode => Self::OpenCode,
            CliArg::Bash => Self::Bash,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum AuthModeArg {
    ApiKey,
    #[value(name = "oauth")]
    OAuth,
    Subscription,
}

impl From<AuthModeArg> for AuthMode {
    fn from(arg: AuthModeArg) -> Self {
        match arg {
            AuthModeArg::ApiKey => Self::ApiKey,
            AuthModeArg::OAuth => Self::OAuth,
            AuthModeArg::Subscription => Self::Subscription,
        }
    }
}
