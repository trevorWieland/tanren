//! Lifecycle state machines and value enums.

mod dispatch;
mod enums;
mod lease;
mod step;

pub use self::dispatch::DispatchStatus;
pub use self::enums::{AuthMode, Cli, DispatchMode, Lane, Outcome, Phase, StepType};
pub use self::lease::LeaseStatus;
pub use self::step::{StepReadyState, StepStatus};

/// Map a CLI harness to its concurrency lane.
///
/// - Claude, `OpenCode` → `Impl`
/// - Codex → `Audit`
/// - Bash → `Gate`
#[must_use]
pub const fn cli_to_lane(cli: &Cli) -> Lane {
    match cli {
        Cli::Claude | Cli::OpenCode => Lane::Impl,
        Cli::Codex => Lane::Audit,
        Cli::Bash => Lane::Gate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_lane_mapping() {
        assert_eq!(cli_to_lane(&Cli::Claude), Lane::Impl);
        assert_eq!(cli_to_lane(&Cli::OpenCode), Lane::Impl);
        assert_eq!(cli_to_lane(&Cli::Codex), Lane::Audit);
        assert_eq!(cli_to_lane(&Cli::Bash), Lane::Gate);
    }
}
