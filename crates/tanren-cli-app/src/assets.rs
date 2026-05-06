//! CLI output formatting for the asset upgrade preview.

use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use tanren_app_services::preview_upgrade;
use tanren_contract::{AssetAction, UpgradePreviewResponse};

/// Run the upgrade preview and write the report to stdout.
///
/// Read-only: no files in `root` are modified.
pub(crate) fn run_preview(root: &Path) -> Result<()> {
    let response = preview_upgrade(root).context("upgrade preview")?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    format_response(&mut handle, &response)?;
    Ok(())
}

fn format_response(handle: &mut std::io::StdoutLock, resp: &UpgradePreviewResponse) -> Result<()> {
    writeln!(
        handle,
        "Upgrade preview: {} -> {}",
        resp.source_version, resp.target_version
    )
    .context("write upgrade preview header")?;

    writeln!(handle).context("write blank line")?;
    writeln!(handle, "Actions:").context("write actions header")?;
    for action in &resp.actions {
        match action {
            AssetAction::Create { path, hash } => {
                writeln!(handle, "  CREATE   {} ({})", path.display(), hash)
                    .context("write create action")?;
            }
            AssetAction::Update {
                path,
                old_hash,
                new_hash,
            } => {
                writeln!(
                    handle,
                    "  UPDATE   {} ({} -> {})",
                    path.display(),
                    old_hash,
                    new_hash
                )
                .context("write update action")?;
            }
            AssetAction::Remove { path, old_hash } => {
                writeln!(handle, "  REMOVE   {} ({})", path.display(), old_hash)
                    .context("write remove action")?;
            }
            AssetAction::Preserve { path, hash } => {
                writeln!(handle, "  PRESERVE {} ({})", path.display(), hash)
                    .context("write preserve action")?;
            }
        }
    }

    if !resp.concerns.is_empty() {
        writeln!(handle).context("write blank line")?;
        writeln!(handle, "Concerns:").context("write concerns header")?;
        for concern in &resp.concerns {
            writeln!(
                handle,
                "  {}: {}",
                match concern.kind {
                    tanren_contract::MigrationConcernKind::HashMismatch => "hash_mismatch",
                    tanren_contract::MigrationConcernKind::RemovedAsset => "removed_asset",
                    tanren_contract::MigrationConcernKind::LegacyManifest => "legacy_manifest",
                    tanren_contract::MigrationConcernKind::UserAssetPathConflict => {
                        "user_asset_path_conflict"
                    }
                    _ => "unknown",
                },
                concern.detail
            )
            .context("write concern")?;
        }
    }

    if !resp.preserved_user_paths.is_empty() {
        writeln!(handle).context("write blank line")?;
        writeln!(handle, "Preserved user paths:").context("write preserved header")?;
        for path in &resp.preserved_user_paths {
            writeln!(handle, "  {}", path.display()).context("write preserved path")?;
        }
    }

    Ok(())
}
