use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::ConfigSecretsError;

#[derive(Debug, Clone, Deserialize)]
struct StandardFrontmatter {
    kind: String,
    name: String,
    category: String,
    importance: String,
}

#[derive(Debug, Clone)]
pub struct Standard {
    pub name: String,
    pub category: String,
    pub importance: String,
}

#[derive(Debug, Clone)]
pub struct StandardsBundle {
    pub root: PathBuf,
    pub standards: Vec<Standard>,
}

pub fn load_standards(standards_root: &Path) -> Result<StandardsBundle, ConfigSecretsError> {
    if !standards_root.is_dir() {
        return Err(ConfigSecretsError::StandardsNotFound {
            path: standards_root.display().to_string(),
        });
    }

    let mut standards = Vec::new();
    collect_standards(standards_root, &mut standards)?;
    standards.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(StandardsBundle {
        root: standards_root.to_path_buf(),
        standards,
    })
}

fn collect_standards(dir: &Path, acc: &mut Vec<Standard>) -> Result<(), ConfigSecretsError> {
    let entries = std::fs::read_dir(dir).map_err(|e| ConfigSecretsError::StandardsParseError {
        path: dir.display().to_string(),
        detail: e.to_string(),
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| ConfigSecretsError::StandardsParseError {
            path: dir.display().to_string(),
            detail: e.to_string(),
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_standards(&path, acc)?;
        } else if path.extension().is_some_and(|ext| ext == "md") {
            let standard = parse_standard_file(&path)?;
            acc.push(standard);
        }
    }

    Ok(())
}

fn parse_standard_file(path: &Path) -> Result<Standard, ConfigSecretsError> {
    let contents =
        std::fs::read_to_string(path).map_err(|e| ConfigSecretsError::StandardsParseError {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;

    let yaml =
        extract_frontmatter(&contents).ok_or_else(|| ConfigSecretsError::StandardsParseError {
            path: path.display().to_string(),
            detail: "no YAML frontmatter found".to_string(),
        })?;

    let fm: StandardFrontmatter =
        serde_yaml::from_str(&yaml).map_err(|e| ConfigSecretsError::StandardsParseError {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;

    if fm.kind != "standard" {
        return Err(ConfigSecretsError::StandardsParseError {
            path: path.display().to_string(),
            detail: format!("expected kind=standard, found kind={}", fm.kind),
        });
    }

    Ok(Standard {
        name: fm.name,
        category: fm.category,
        importance: fm.importance,
    })
}

fn extract_frontmatter(contents: &str) -> Option<String> {
    let mut lines = contents.lines();
    let first = lines.next()?;
    if first.trim() != "---" {
        return None;
    }
    let mut yaml_lines = Vec::new();
    for line in lines {
        if line.trim() == "---" {
            return Some(yaml_lines.join("\n"));
        }
        yaml_lines.push(line);
    }
    None
}
