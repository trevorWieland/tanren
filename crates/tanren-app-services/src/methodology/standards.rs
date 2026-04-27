//! Standards profile loading and validation.
//!
//! `tanren-cli install` bootstraps a target repo's
//! `tanren/standards/<category>/<name>.md` tree from embedded profile
//! markdown under the `preserve_existing` merge policy.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use tanren_domain::methodology::evidence::frontmatter::split as split_frontmatter;
use tanren_domain::methodology::standard::{Standard, StandardImportance};
use tanren_domain::validated::NonEmptyString;

use super::errors::{MethodologyError, MethodologyResult};

/// One standards profile source file embedded in the Tanren
/// distribution and ready to install into a target repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileStandardAsset {
    pub relative_path: PathBuf,
    pub bytes: Vec<u8>,
}

/// Load runtime standards from the configured standards root.
///
/// Behavior:
/// - If `root` exists: load every `*.md` recursively and validate strict
///   metadata/frontmatter schema for each file.
/// - If `root` does not exist or contains no standards: hard error.
///
/// # Errors
/// Returns [`MethodologyError::Validation`] when file schema/metadata is
/// invalid, or [`MethodologyError::Io`] on filesystem errors.
pub fn load_runtime_standards(root: &Path) -> MethodologyResult<Vec<Standard>> {
    if !root.exists() {
        return Err(MethodologyError::Validation(format!(
            "standards root `{}` does not exist",
            root.display()
        )));
    }
    let files = collect_markdown_files(root)?;
    if files.is_empty() {
        return Err(MethodologyError::Validation(format!(
            "standards root `{}` exists but contains no .md standards",
            root.display()
        )));
    }
    let mut out = Vec::with_capacity(files.len());
    for file in files {
        out.push(parse_standard_file(root, &file)?);
    }
    out.sort_by(|a, b| {
        a.category
            .as_str()
            .cmp(b.category.as_str())
            .then(a.name.as_str().cmp(b.name.as_str()))
    });
    Ok(out)
}

/// Load and validate the named embedded standards profile.
///
/// # Errors
/// Returns [`MethodologyError::Validation`] if the profile name is
/// invalid, missing, empty, or contains malformed standards.
pub fn load_embedded_profile_assets(profile: &str) -> MethodologyResult<Vec<ProfileStandardAsset>> {
    validate_profile_name(profile)?;
    let prefix = format!("profiles/{profile}/");
    let mut out = Vec::new();
    for asset in super::assets::PROFILE_ASSETS {
        let Some(rel_raw) = asset.path.strip_prefix(&prefix) else {
            continue;
        };
        if !Path::new(asset.path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        {
            continue;
        }
        let rel = PathBuf::from(rel_raw);
        let _ = parse_standard_text(Path::new(&prefix), &rel, asset.contents)?;
        out.push(ProfileStandardAsset {
            relative_path: rel,
            bytes: asset.contents.as_bytes().to_vec(),
        });
    }
    if out.is_empty() {
        return Err(MethodologyError::Validation(format!(
            "standards profile `{profile}` is missing or contains no .md standards"
        )));
    }
    out.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(out)
}

fn validate_profile_name(profile: &str) -> MethodologyResult<()> {
    if profile.trim().is_empty()
        || profile.contains('/')
        || profile.contains('\\')
        || profile
            .split('.')
            .any(|part| part.is_empty() || part == "..")
    {
        return Err(MethodologyError::Validation(format!(
            "invalid standards profile `{profile}`"
        )));
    }
    Ok(())
}

fn collect_markdown_files(root: &Path) -> MethodologyResult<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|source| MethodologyError::Io {
            path: dir.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| MethodologyError::Io {
                path: dir.clone(),
                source,
            })?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path
                .extension()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn parse_standard_file(root: &Path, path: &Path) -> MethodologyResult<Standard> {
    let raw = std::fs::read_to_string(path).map_err(|source| MethodologyError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let rel = path.strip_prefix(root).map_err(|_| {
        MethodologyError::Validation(format!(
            "standard file `{}` is outside configured root `{}`",
            path.display(),
            root.display()
        ))
    })?;
    parse_standard_text(root, rel, &raw)
}

fn parse_standard_text(root: &Path, rel: &Path, raw: &str) -> MethodologyResult<Standard> {
    let (yaml, body) = split_frontmatter(raw).map_err(|e| {
        MethodologyError::Validation(format!(
            "invalid frontmatter in {}: {e}",
            root.join(rel).display()
        ))
    })?;
    let fm: StandardFrontmatterIn = serde_yaml::from_value(yaml).map_err(|e| {
        MethodologyError::Validation(format!(
            "standard frontmatter schema error in {}: {e}",
            root.join(rel).display()
        ))
    })?;
    if fm.kind.as_str() != "standard" {
        return Err(MethodologyError::Validation(format!(
            "standard `{}` has invalid kind `{}`; expected `standard`",
            root.join(rel).display(),
            fm.kind
        )));
    }
    let stem = rel
        .file_stem()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| {
            MethodologyError::Validation(format!(
                "standard file `{}` has no valid UTF-8 stem",
                root.join(rel).display()
            ))
        })?;
    let dir_category = rel
        .parent()
        .and_then(Path::file_name)
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| {
            MethodologyError::Validation(format!(
                "standard file `{}` must be under `<root>/<category>/<name>.md`",
                root.join(rel).display()
            ))
        })?;
    if dir_category != fm.category {
        return Err(MethodologyError::Validation(format!(
            "standard `{}` category mismatch: path has `{dir_category}`, frontmatter has `{}`",
            root.join(rel).display(),
            fm.category
        )));
    }
    if stem != fm.name {
        return Err(MethodologyError::Validation(format!(
            "standard `{}` name mismatch: file stem `{stem}`, frontmatter name `{}`",
            root.join(rel).display(),
            fm.name
        )));
    }
    let body = body.trim();
    if body.is_empty() {
        return Err(MethodologyError::Validation(format!(
            "standard `{}` body is empty",
            root.join(rel).display()
        )));
    }
    Ok(Standard {
        name: NonEmptyString::try_new(fm.name).map_err(MethodologyError::Domain)?,
        category: NonEmptyString::try_new(fm.category).map_err(MethodologyError::Domain)?,
        applies_to: fm.applies_to,
        applies_to_languages: fm.applies_to_languages,
        applies_to_domains: fm.applies_to_domains,
        importance: parse_importance(&fm.importance).ok_or_else(|| {
            MethodologyError::Validation(format!(
                "standard `{}` has invalid importance `{}`",
                root.join(rel).display(),
                fm.importance
            ))
        })?,
        body: body.to_owned(),
    })
}

fn parse_importance(raw: &str) -> Option<StandardImportance> {
    match raw {
        "low" => Some(StandardImportance::Low),
        "medium" => Some(StandardImportance::Medium),
        "high" => Some(StandardImportance::High),
        "critical" => Some(StandardImportance::Critical),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StandardFrontmatterIn {
    kind: String,
    name: String,
    category: String,
    importance: String,
    #[serde(default)]
    applies_to: Vec<String>,
    #[serde(default)]
    applies_to_languages: Vec<String>,
    #[serde(default)]
    applies_to_domains: Vec<String>,
}
