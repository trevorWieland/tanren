//! Loaders for the behavior catalog (`docs/behaviors/B-*.md`) and the
//! roadmap DAG (`docs/roadmap/dag.json`). Both are surfaced as plain
//! `HashMap` keyed on behavior ID; `xtask check-bdd-tags` cross-references
//! these against parsed `.feature` files.

use anyhow::{Context, Result};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct BehaviorRecord {
    pub interfaces: BTreeSet<String>,
    pub product_status: String,
}

pub(super) fn load_behaviors(dir: &Path) -> Result<HashMap<String, BehaviorRecord>> {
    let mut map = HashMap::new();
    if !dir.exists() {
        return Ok(map);
    }
    for entry in fs::read_dir(dir).with_context(|| format!("read_dir {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !is_behavior_file(&path) {
            continue;
        }
        let content =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let Some(id) = scan_field(&content, "id") else {
            continue;
        };
        let product_status = scan_field(&content, "product_status").unwrap_or_default();
        let interfaces = scan_list(&content, "interfaces")
            .into_iter()
            .collect::<BTreeSet<_>>();
        map.insert(
            id,
            BehaviorRecord {
                interfaces,
                product_status,
            },
        );
    }
    Ok(map)
}

fn is_behavior_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if !name.starts_with("B-") {
        return false;
    }
    Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
}

fn scan_field(text: &str, field: &str) -> Option<String> {
    let prefix = format!("{field}:");
    let mut in_frontmatter = false;
    for line in text.lines() {
        let raw = line.trim_end();
        let trimmed = raw.trim_start();
        if raw == "---" {
            if in_frontmatter {
                break;
            }
            in_frontmatter = true;
            continue;
        }
        if !in_frontmatter {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            return Some(rest.trim().trim_matches('"').to_owned());
        }
    }
    None
}

fn scan_list(text: &str, field: &str) -> Vec<String> {
    let prefix = format!("{field}:");
    let mut in_frontmatter = false;
    for line in text.lines() {
        let raw = line.trim_end();
        let trimmed = raw.trim_start();
        if raw == "---" {
            if in_frontmatter {
                break;
            }
            in_frontmatter = true;
            continue;
        }
        if !in_frontmatter {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            let rest = rest.trim();
            if let Some(inner) = rest.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                return inner
                    .split(',')
                    .map(|p| p.trim().trim_matches('"').to_owned())
                    .filter(|p| !p.is_empty())
                    .collect();
            }
        }
    }
    Vec::new()
}

#[derive(Debug, Clone)]
pub(super) struct EvidenceRecord {
    pub interfaces: BTreeSet<String>,
    pub witnesses: BTreeSet<String>,
    pub node_id: String,
}

pub(super) fn load_dag_evidence(path: &Path) -> Result<HashMap<String, EvidenceRecord>> {
    let mut map = HashMap::new();
    if !path.exists() {
        return Ok(map);
    }
    let content = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&content).with_context(|| format!("parse {}", path.display()))?;
    let Some(nodes) = value.get("nodes").and_then(|n| n.as_array()) else {
        return Ok(map);
    };
    for node in nodes {
        let Some(node_id) = node.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(evidence) = node.get("expected_evidence").and_then(|v| v.as_array()) else {
            continue;
        };
        for ev in evidence {
            let Some(behavior_id) = ev.get("behavior_id").and_then(|v| v.as_str()) else {
                continue;
            };
            let interfaces = collect_str_array(ev.get("interfaces"));
            let witnesses = collect_str_array(ev.get("witnesses"));
            map.insert(
                behavior_id.to_owned(),
                EvidenceRecord {
                    interfaces,
                    witnesses,
                    node_id: node_id.to_owned(),
                },
            );
        }
    }
    Ok(map)
}

fn collect_str_array(value: Option<&serde_json::Value>) -> BTreeSet<String> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}
