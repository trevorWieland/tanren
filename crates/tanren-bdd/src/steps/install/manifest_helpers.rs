mod fixture_manifest;
mod hash;
mod paths;
mod proof_contract;
mod workspace;

pub(crate) use fixture_manifest::{
    append_uninstall_stale_generated_manifest_entry,
    tamper_uninstall_manifest_with_raw_generated_entry,
};
pub(crate) use hash::sha256_hex_string;
pub(crate) use paths::{RepositoryRelativePath, io_error, validate_relative_path};
pub(crate) use proof_contract::{
    assert_manifest_rust_cargo_defaults, assert_rust_cargo_default_assets_installed,
    assert_rust_cargo_standards_installed, assert_selected_integration_command_assets,
    assert_uninstall_preserves_baseline_file_content,
    assert_uninstall_removes_generated_assets_and_manifest,
};
pub(crate) use workspace::read_workspace_catalog_file;
