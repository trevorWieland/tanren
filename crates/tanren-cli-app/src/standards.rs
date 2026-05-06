use std::io::Write;

use anyhow::{Context, Result};
use tanren_app_services::{AppServiceError, Handlers};
use tanren_contract::{StandardsFailureReason, StandardsInspectionRequest};

pub(crate) fn run_inspect(project_dir: &str) -> Result<()> {
    let handlers = Handlers::new();
    let request = StandardsInspectionRequest {
        project_dir: project_dir.to_owned(),
    };
    let response = handlers
        .inspect_standards(&request)
        .map_err(standards_error)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "standards_root={}", response.standards_root)
        .context("write standards root")?;
    writeln!(handle, "count={}", response.count()).context("write standards count")?;
    Ok(())
}

fn standards_error(err: AppServiceError) -> anyhow::Error {
    match err {
        AppServiceError::Standards(reason) => {
            anyhow::anyhow!(
                "error: {} — {}: {}",
                cli_message(reason),
                reason.code(),
                reason.summary()
            )
        }
        AppServiceError::InvalidInput(message) => {
            anyhow::anyhow!("error: validation_failed — {message}")
        }
        _ => anyhow::anyhow!("error: internal_error"),
    }
}

fn cli_message(reason: StandardsFailureReason) -> &'static str {
    match reason {
        StandardsFailureReason::StandardsRootNotFound => "standards not found",
        StandardsFailureReason::StandardsFileMalformed => "parse error",
        StandardsFailureReason::StandardsEmpty => "standards empty",
        StandardsFailureReason::InvalidSchema => "invalid schema",
        StandardsFailureReason::PathViolation => "path violation",
        StandardsFailureReason::TreeBoundsExceeded => "tree bounds exceeded",
        _ => "unknown standards failure",
    }
}
