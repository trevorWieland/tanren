//! `xtask {generate,check}-web-role-contracts`.
//!
//! The web surface and Playwright BDD harness import a generated role-wire
//! contract module so TypeScript request/response/error shapes stay aligned
//! with the Rust contract and `OpenAPI` source of truth.

use anyhow::{Context, Result, bail};
use std::fs;
use std::io::Write;
use std::path::Path;

const TARGET: &str = "apps/web/src/app/lib/generated/role-contract.ts";
const TEMPLATE: &str = include_str!("role-contract.ts.template");

pub(crate) fn generate(root: &Path) -> Result<()> {
    let target = root.join(TARGET);
    let parent = target
        .parent()
        .context("role-contract target must have a parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create role-contract dir {}", parent.display()))?;
    fs::write(&target, TEMPLATE).with_context(|| format!("write {}", target.display()))?;

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let _ = writeln!(
        handle,
        "generate-web-role-contracts: wrote {}",
        target.strip_prefix(root).unwrap_or(&target).display()
    );
    Ok(())
}

pub(crate) fn check(root: &Path) -> Result<()> {
    let target = root.join(TARGET);
    let current =
        fs::read_to_string(&target).with_context(|| format!("read {}", target.display()))?;

    if current == TEMPLATE {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-web-role-contracts: 0 violations (generated role contract is current)"
        );
        return Ok(());
    }

    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    let _ = writeln!(
        handle,
        "{}: generated role contract drifted; run `cargo run -q -p tanren-xtask -- generate-web-role-contracts`",
        target.strip_prefix(root).unwrap_or(&target).display()
    );
    bail!("check-web-role-contracts: 1 violation(s)")
}
