use super::*;

pub(super) fn assert_rust_cargo_default_assets_installed_inner(
    repository_root: &Path,
) -> Result<(), InstallProofFailure> {
    let manifest = read_install_manifest(repository_root)?;
    let expected_integrations = InstallProofIntegration::all();
    assert_manifest_profile_and_integrations(
        &manifest,
        InstallProofProfile::RustCargo,
        &expected_integrations,
    )?;
    assert_command_assets_for_selected_integrations(
        repository_root,
        &manifest,
        &expected_integrations,
    )?;
    assert_standards_assets_for_rust_cargo(repository_root, &manifest)?;
    Ok(())
}

pub(super) fn assert_rust_cargo_standards_installed_inner(
    repository_root: &Path,
) -> Result<(), InstallProofFailure> {
    let manifest = read_install_manifest(repository_root)?;
    assert_standards_assets_for_rust_cargo(repository_root, &manifest)
}

pub(super) fn assert_selected_integration_command_assets_inner(
    repository_root: &Path,
    selected_integrations: &str,
) -> Result<(), InstallProofFailure> {
    let selected =
        parse_install_integration_selection(selected_integrations).map_err(|source| {
            InstallProofFailure::InvalidIntegrationSelection {
                selection: selected_integrations.to_owned(),
                source,
            }
        })?;
    let manifest = read_install_manifest(repository_root)?;
    assert_manifest_profile_and_integrations(&manifest, InstallProofProfile::RustCargo, &selected)?;
    assert_command_assets_for_selected_integrations(repository_root, &manifest, &selected)?;
    assert_unselected_integration_roots_are_empty(repository_root, &selected)
}

pub(super) fn assert_manifest_rust_cargo_defaults_inner(
    repository_root: &Path,
) -> Result<(), InstallProofFailure> {
    let manifest = read_install_manifest(repository_root)?;
    let expected_integrations = InstallProofIntegration::all();
    assert_manifest_profile_and_integrations(
        &manifest,
        InstallProofProfile::RustCargo,
        &expected_integrations,
    )?;

    let has_command_assets = manifest
        .parsed
        .entries
        .iter()
        .any(|entry| entry.asset_class == InstallProofAssetClass::MethodologyCommand);
    if !has_command_assets {
        return Err(manifest_contract_error(
            &manifest,
            "manifest must include at least one methodology-command entry",
        ));
    }

    let has_standards_assets = manifest
        .parsed
        .entries
        .iter()
        .any(|entry| entry.asset_class == InstallProofAssetClass::StandardsProfile);
    if !has_standards_assets {
        return Err(manifest_contract_error(
            &manifest,
            "manifest must include at least one standards-profile entry",
        ));
    }

    Ok(())
}

fn read_install_manifest(
    repository_root: &Path,
) -> Result<ObservedInstallManifest, InstallProofFailure> {
    let path = repository_root.join(INSTALL_MANIFEST_REPO_PATH);
    let raw = read_to_string_with_context(&path, "read install manifest from repository fixture")?;
    let parsed: InstallManifest =
        toml::from_str(&raw).map_err(|source| InstallProofFailure::InstallManifestTomlParse {
            manifest_path: path.clone(),
            source,
        })?;

    if parsed.manifest_version != INSTALL_MANIFEST_VERSION {
        let actual_version = parsed.manifest_version;
        return Err(manifest_contract_error(
            &ObservedInstallManifest { path, raw, parsed },
            &format!(
                "manifest_version must equal {INSTALL_MANIFEST_VERSION}, got {actual_version}"
            ),
        ));
    }

    Ok(ObservedInstallManifest { path, raw, parsed })
}

fn assert_manifest_profile_and_integrations(
    manifest: &ObservedInstallManifest,
    expected_profile: InstallProofProfile,
    expected_integrations: &BTreeSet<InstallProofIntegration>,
) -> Result<(), InstallProofFailure> {
    if manifest.parsed.profile != expected_profile {
        return Err(manifest_contract_error(
            manifest,
            &format!(
                "manifest profile must be '{}', got '{}'",
                expected_profile.as_str(),
                manifest.parsed.profile.as_str()
            ),
        ));
    }

    let actual_integrations: BTreeSet<InstallProofIntegration> =
        manifest.parsed.integrations.iter().copied().collect();
    if actual_integrations != *expected_integrations {
        return Err(manifest_contract_error(
            manifest,
            &format!(
                "manifest integrations must be {}, got {}",
                format_integration_set(expected_integrations),
                format_integration_set(&actual_integrations)
            ),
        ));
    }

    Ok(())
}

fn assert_command_assets_for_selected_integrations(
    repository_root: &Path,
    manifest: &ObservedInstallManifest,
    selected: &BTreeSet<InstallProofIntegration>,
) -> Result<(), InstallProofFailure> {
    let mut seen = BTreeSet::new();
    for entry in manifest
        .parsed
        .entries
        .iter()
        .filter(|entry| entry.asset_class == InstallProofAssetClass::MethodologyCommand)
    {
        let integration = entry.integration.ok_or_else(|| {
            manifest_contract_error(
                manifest,
                &format!(
                    "methodology-command entry '{}' must include integration",
                    entry.path.as_str()
                ),
            )
        })?;

        if !selected.contains(&integration) {
            return Err(manifest_contract_error(
                manifest,
                &format!(
                    "methodology-command entry '{}' unexpectedly targets integration '{}'",
                    entry.path.as_str(),
                    integration.as_str()
                ),
            ));
        }

        let destination_roots: BTreeSet<&'static str> = selected
            .iter()
            .map(|selected_integration| selected_integration.destination_root())
            .collect();
        if !is_generated_command_destination(entry.path.as_str(), &destination_roots) {
            return Err(manifest_contract_error(
                manifest,
                &format!(
                    "methodology-command entry '{}' must be under selected integration destination root",
                    entry.path.as_str()
                ),
            ));
        }

        assert_file_exists(repository_root, entry.path.as_str())?;
        seen.insert(integration);
    }

    if seen != *selected {
        return Err(manifest_contract_error(
            manifest,
            &format!(
                "methodology-command integrations must match selection {}; saw {}",
                format_integration_set(selected),
                format_integration_set(&seen)
            ),
        ));
    }

    Ok(())
}

fn assert_standards_assets_for_rust_cargo(
    repository_root: &Path,
    manifest: &ObservedInstallManifest,
) -> Result<(), InstallProofFailure> {
    let mut saw_any = false;
    for entry in manifest
        .parsed
        .entries
        .iter()
        .filter(|entry| entry.asset_class == InstallProofAssetClass::StandardsProfile)
    {
        saw_any = true;
        if !entry.path.as_str().starts_with(RUST_CARGO_PROFILE_ROOT) {
            return Err(manifest_contract_error(
                manifest,
                &format!(
                    "standards-profile entry '{}' must be under '{}'",
                    entry.path.as_str(),
                    RUST_CARGO_PROFILE_ROOT
                ),
            ));
        }
        assert_file_exists(repository_root, entry.path.as_str())?;
    }

    if !saw_any {
        return Err(manifest_contract_error(
            manifest,
            "manifest must include at least one standards-profile entry",
        ));
    }

    Ok(())
}

fn assert_unselected_integration_roots_are_empty(
    repository_root: &Path,
    selected: &BTreeSet<InstallProofIntegration>,
) -> Result<(), InstallProofFailure> {
    for integration in InstallProofIntegration::all() {
        if selected.contains(&integration) {
            continue;
        }

        let root_path = repository_root.join(integration.destination_root());
        if has_any_files(&root_path)? {
            return Err(InstallProofFailure::ExpectedFileToBeAbsent { path: root_path });
        }
    }
    Ok(())
}

fn has_any_files(path: &Path) -> Result<bool, InstallProofFailure> {
    if !path.exists() {
        return Ok(false);
    }

    let entries = fs::read_dir(path).map_err(|source| InstallProofFailure::ReadDirectory {
        path: path.to_path_buf(),
        action: "inspect unselected integration root",
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| InstallProofFailure::ReadDirectoryEntry {
            path: path.to_path_buf(),
            action: "inspect unselected integration root entries",
            source,
        })?;
        let entry_path = entry.path();
        let file_type =
            entry
                .file_type()
                .map_err(|source| InstallProofFailure::InspectFileType {
                    path: entry_path.clone(),
                    action: "inspect unselected integration root entry type",
                    source,
                })?;

        if file_type.is_file() {
            return Ok(true);
        }
        if file_type.is_dir() && has_any_files(&entry_path)? {
            return Ok(true);
        }
    }

    Ok(false)
}

fn is_generated_command_destination(
    path: &str,
    destination_roots: &BTreeSet<&'static str>,
) -> bool {
    destination_roots
        .iter()
        .any(|root| matches_generated_command_layout(path, root))
}

fn matches_generated_command_layout(path: &str, destination_root: &str) -> bool {
    let Some(filename) = path.strip_prefix(destination_root) else {
        return false;
    };
    if filename.is_empty() || filename.contains('/') {
        return false;
    }
    Path::new(filename)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        && filename != ".md"
}

fn manifest_contract_error(
    manifest: &ObservedInstallManifest,
    expected: &str,
) -> InstallProofFailure {
    InstallProofFailure::ManifestContractViolation {
        expected: expected.to_owned(),
        manifest_path: manifest.path.clone(),
        manifest: manifest.raw.clone(),
    }
}

fn format_integration_set(set: &BTreeSet<InstallProofIntegration>) -> String {
    let values = set
        .iter()
        .map(|integration| integration.as_str())
        .collect::<Vec<_>>();
    format!("[{}]", values.join(", "))
}

#[cfg(feature = "test-hooks")]
pub(super) fn tamper_manifest_with_raw_generated_entry_inner(
    repository_root: &Path,
    raw_path: &str,
) -> Result<(), InstallProofFailure> {
    const TAMPERED_ENTRY_SHA256: &str =
        "0000000000000000000000000000000000000000000000000000000000000000";
    let manifest_path = repository_root.join(INSTALL_MANIFEST_REPO_PATH);
    let mut manifest = read_to_string_with_context(&manifest_path, "read install manifest")?;
    let path_line = format!("path = \"{raw_path}\"");
    if manifest.contains(path_line.as_str()) {
        return Err(InstallProofFailure::StaleManifestPathAlreadyPresent {
            path: raw_path.to_owned(),
        });
    }
    let _ = write!(
        manifest,
        "\n[[entries]]\npath = \"{raw_path}\"\ncontent_hash = \"{TAMPERED_ENTRY_SHA256}\"\nasset_class = \"methodology-command\"\nintegration = \"{}\"\npreservation = \"replace-generated\"\n",
        InstallProofIntegration::Codex.as_str()
    );
    fs::write(&manifest_path, manifest).map_err(|source| InstallProofFailure::WriteFile {
        path: manifest_path,
        action: "write install manifest with tampered raw entry",
        source,
    })?;
    Ok(())
}

#[cfg(feature = "test-hooks")]
pub(super) fn read_workspace_catalog_file_inner(
    relative_path: &InstallProofRepoRelativePath,
) -> Result<String, InstallProofFailure> {
    let absolute = workspace_root()?.join(relative_path.as_str());
    read_to_string_with_context(&absolute, "read workspace catalog file")
}

#[cfg(feature = "test-hooks")]
fn workspace_root() -> Result<PathBuf, InstallProofFailure> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|source| InstallProofFailure::CanonicalizeWorkspaceRoot {
            action: "resolving workspace root",
            source,
        })
}

fn assert_file_exists(
    repository_root: &Path,
    relative_path: &str,
) -> Result<(), InstallProofFailure> {
    let absolute = repository_root.join(relative_path);
    if !absolute.exists() {
        return Err(InstallProofFailure::ExpectedFileToExist { path: absolute });
    }
    Ok(())
}

fn read_to_string_with_context(
    path: &Path,
    action: &'static str,
) -> Result<String, InstallProofFailure> {
    fs::read_to_string(path).map_err(|source| InstallProofFailure::ReadFile {
        path: path.to_path_buf(),
        action,
        source,
    })
}
