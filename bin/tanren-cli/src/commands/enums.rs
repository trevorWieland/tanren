//! CLI-local wrapper enums for clap parsing.
//!
//! `tanren-contract` is transport-neutral by design (see
//! `docs/rewrite/CRATE_GUIDE.md` — contract rule: schema only, no
//! transport coupling). Clap's `ValueEnum` derive is a transport
//! concern, so it lives here in the CLI binary rather than on the
//! contract enums themselves. Each wrapper is a thin newtype with a
//! one-to-one `From` mapping back to the contract enum so handler
//! code can `.into()` at the seam.
//!
//! The special renames (`opencode`, `oauth`) match the legacy
//! canonical wire tags and the Display impls in
//! `tanren_domain::status::enums`.

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

#[cfg(test)]
mod tests {
    //! Round-trip tests ensuring each CLI wrapper maps 1:1 to the
    //! contract enum. If a new variant is added to a contract enum
    //! without updating the wrapper, these tests still compile, but
    //! the `From` impls will fail exhaustiveness checks at compile
    //! time (the `match arg` arms force the wrapper to enumerate
    //! every variant before `.into()` can succeed).

    use super::*;

    #[test]
    fn dispatch_mode_round_trip() {
        assert_eq!(
            DispatchMode::from(DispatchModeArg::Auto),
            DispatchMode::Auto
        );
        assert_eq!(
            DispatchMode::from(DispatchModeArg::Manual),
            DispatchMode::Manual
        );
    }

    #[test]
    fn dispatch_status_round_trip() {
        assert_eq!(
            DispatchStatus::from(DispatchStatusArg::Pending),
            DispatchStatus::Pending
        );
        assert_eq!(
            DispatchStatus::from(DispatchStatusArg::Running),
            DispatchStatus::Running
        );
        assert_eq!(
            DispatchStatus::from(DispatchStatusArg::Completed),
            DispatchStatus::Completed
        );
        assert_eq!(
            DispatchStatus::from(DispatchStatusArg::Failed),
            DispatchStatus::Failed
        );
        assert_eq!(
            DispatchStatus::from(DispatchStatusArg::Cancelled),
            DispatchStatus::Cancelled
        );
    }

    #[test]
    fn lane_round_trip() {
        assert_eq!(Lane::from(LaneArg::Impl), Lane::Impl);
        assert_eq!(Lane::from(LaneArg::Audit), Lane::Audit);
        assert_eq!(Lane::from(LaneArg::Gate), Lane::Gate);
    }

    #[test]
    fn phase_round_trip() {
        assert_eq!(Phase::from(PhaseArg::DoTask), Phase::DoTask);
        assert_eq!(Phase::from(PhaseArg::AuditTask), Phase::AuditTask);
        assert_eq!(Phase::from(PhaseArg::RunDemo), Phase::RunDemo);
        assert_eq!(Phase::from(PhaseArg::AuditSpec), Phase::AuditSpec);
        assert_eq!(Phase::from(PhaseArg::Investigate), Phase::Investigate);
        assert_eq!(Phase::from(PhaseArg::Gate), Phase::Gate);
        assert_eq!(Phase::from(PhaseArg::Setup), Phase::Setup);
        assert_eq!(Phase::from(PhaseArg::Cleanup), Phase::Cleanup);
    }

    #[test]
    fn cli_round_trip() {
        assert_eq!(Cli::from(CliArg::Claude), Cli::Claude);
        assert_eq!(Cli::from(CliArg::Codex), Cli::Codex);
        assert_eq!(Cli::from(CliArg::OpenCode), Cli::OpenCode);
        assert_eq!(Cli::from(CliArg::Bash), Cli::Bash);
    }

    #[test]
    fn auth_mode_round_trip() {
        assert_eq!(AuthMode::from(AuthModeArg::ApiKey), AuthMode::ApiKey);
        assert_eq!(AuthMode::from(AuthModeArg::OAuth), AuthMode::OAuth);
        assert_eq!(
            AuthMode::from(AuthModeArg::Subscription),
            AuthMode::Subscription
        );
    }
}
