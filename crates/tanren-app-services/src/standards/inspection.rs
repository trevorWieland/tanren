//! Standards filesystem inspection.
//!
//! Reads the project configuration, resolves the standards root, scans
//! for standards files, and returns a [`StandardsInspectionResponse`].

use std::path::Path;

use serde::Deserialize;
use tanren_contract::{
    StandardCategory, StandardImportance, StandardKind, StandardSchema, StandardStatus,
    StandardView, StandardsFailureReason, StandardsInspectionRequest, StandardsInspectionResponse,
};

use crate::AppServiceError;

pub fn inspect(
    request: &StandardsInspectionRequest,
) -> Result<StandardsInspectionResponse, AppServiceError> {
    let project_dir = Path::new(&request.project_dir);
    let config = read_project_config(project_dir)?;
    let standards_root = project_dir.join(config.standards.root);

    if !standards_root.is_dir() {
        return Err(AppServiceError::Standards(
            StandardsFailureReason::StandardsRootNotFound,
        ));
    }

    let mut standards = Vec::new();
    collect_standards(&standards_root, &mut standards)?;

    if standards.is_empty() {
        return Err(AppServiceError::Standards(
            StandardsFailureReason::StandardsEmpty,
        ));
    }

    Ok(StandardsInspectionResponse {
        standards_root: standards_root.display().to_string(),
        standards,
    })
}

fn read_project_config(project_dir: &Path) -> Result<ProjectConfig, AppServiceError> {
    let config_path = project_dir.join("tanren.yml");
    let content = std::fs::read_to_string(&config_path)
        .map_err(|_| AppServiceError::Standards(StandardsFailureReason::StandardsRootNotFound))?;
    serde_yaml::from_str(&content)
        .map_err(|_| AppServiceError::Standards(StandardsFailureReason::StandardsFileMalformed))
}

fn collect_standards(root: &Path, out: &mut Vec<StandardView>) -> Result<(), AppServiceError> {
    let entries = std::fs::read_dir(root)
        .map_err(|_| AppServiceError::Standards(StandardsFailureReason::StandardsRootNotFound))?;
    for entry in entries {
        let entry = entry.map_err(|_| {
            AppServiceError::Standards(StandardsFailureReason::StandardsRootNotFound)
        })?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "md") {
            let content = std::fs::read_to_string(&path).map_err(|_| {
                AppServiceError::Standards(StandardsFailureReason::StandardsFileMalformed)
            })?;
            out.push(parse_standard(&content, &path, root)?);
        }
    }
    Ok(())
}

fn parse_standard(
    content: &str,
    absolute_path: &Path,
    root: &Path,
) -> Result<StandardView, AppServiceError> {
    let fm = extract_frontmatter(content)?;
    let relative = absolute_path
        .strip_prefix(root)
        .map_err(|_| AppServiceError::Standards(StandardsFailureReason::StandardsFileMalformed))?;
    let name = fm.name.unwrap_or_else(|| {
        absolute_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_default()
    });
    Ok(StandardView {
        schema: StandardSchema::current(),
        kind: map_kind(fm.kind.as_deref()),
        category: map_category(fm.category.as_deref()),
        importance: map_importance(fm.importance.as_deref()),
        status: map_status(fm.status.as_deref()),
        name,
        path: relative.display().to_string(),
    })
}

fn extract_frontmatter(content: &str) -> Result<Frontmatter, AppServiceError> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Ok(Frontmatter::default());
    }
    let rest = trimmed.get(3..).ok_or_else(|| {
        AppServiceError::Standards(StandardsFailureReason::StandardsFileMalformed)
    })?;
    let end = rest.find("---").ok_or_else(|| {
        AppServiceError::Standards(StandardsFailureReason::StandardsFileMalformed)
    })?;
    let yaml = rest.get(..end).ok_or_else(|| {
        AppServiceError::Standards(StandardsFailureReason::StandardsFileMalformed)
    })?;
    serde_yaml::from_str(yaml)
        .map_err(|_| AppServiceError::Standards(StandardsFailureReason::StandardsFileMalformed))
}

fn map_kind(raw: Option<&str>) -> StandardKind {
    match raw {
        Some("policy") => StandardKind::Policy,
        Some("guideline") => StandardKind::Guideline,
        Some("convention") => StandardKind::Convention,
        _ => StandardKind::Standard,
    }
}

fn map_category(raw: Option<&str>) -> StandardCategory {
    match raw {
        Some("testing") => StandardCategory::Testing,
        Some("documentation") => StandardCategory::Documentation,
        Some("security") => StandardCategory::Security,
        Some("architecture") => StandardCategory::Architecture,
        Some("process") => StandardCategory::Process,
        _ => StandardCategory::CodeQuality,
    }
}

fn map_importance(raw: Option<&str>) -> StandardImportance {
    match raw {
        Some("recommended" | "medium") => StandardImportance::Recommended,
        Some("informational" | "low") => StandardImportance::Informational,
        _ => StandardImportance::Required,
    }
}

fn map_status(raw: Option<&str>) -> StandardStatus {
    match raw {
        Some("deprecated") => StandardStatus::Deprecated,
        Some("pending") => StandardStatus::Pending,
        _ => StandardStatus::Active,
    }
}

#[derive(Default, Deserialize)]
struct Frontmatter {
    kind: Option<String>,
    name: Option<String>,
    category: Option<String>,
    importance: Option<String>,
    status: Option<String>,
}

#[derive(Deserialize)]
struct ProjectConfig {
    standards: ProjectStandards,
}

#[derive(Deserialize)]
struct ProjectStandards {
    root: String,
}
