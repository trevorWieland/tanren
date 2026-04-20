use std::path::Path;

use tanren_domain::NonEmptyString;
use tanren_domain::methodology::pillar::{Pillar, PillarId, PillarScore, builtin_pillars};

use super::config::{MethodologyRubricConfig, MethodologyRubricPillar, TanrenConfig};

/// Resolve effective pillars for runtime/install use.
///
/// Priority:
/// 1. `tanren/rubric.yml` (relative to `config_path`).
/// 2. `tanren.yml` `methodology.rubric` (canonical).
/// 3. `tanren.yml` top-level `rubric` (deprecated compatibility alias).
/// 4. Built-in pillars.
pub fn effective_pillars_for_runtime(
    config_path: &Path,
    cfg: &TanrenConfig,
) -> Result<Vec<Pillar>, String> {
    let config_dir = config_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let rubric_path = config_dir.join("tanren/rubric.yml");
    if rubric_path.exists() {
        let raw = std::fs::read_to_string(&rubric_path)
            .map_err(|e| format!("reading {}: {e}", rubric_path.display()))?;
        let parsed: MethodologyRubricConfig = serde_yaml::from_str(&raw)
            .map_err(|e| format!("parsing {}: {e}", rubric_path.display()))?;
        return build_effective(&parsed);
    }
    if !cfg.methodology.rubric.is_empty() {
        return build_effective(&cfg.methodology.rubric);
    }
    if let Some(legacy) = cfg.other.get("rubric") {
        let parsed: MethodologyRubricConfig = serde_yaml::from_value(legacy.clone())
            .map_err(|e| format!("parsing deprecated top-level tanren.yml rubric section: {e}"))?;
        return build_effective(&parsed);
    }
    Ok(builtin_pillars())
}

#[must_use]
pub fn pillar_ids_csv(pillars: &[Pillar]) -> String {
    pillars
        .iter()
        .map(|p| p.id.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn build_effective(raw: &MethodologyRubricConfig) -> Result<Vec<Pillar>, String> {
    if raw.is_empty() {
        return Ok(builtin_pillars());
    }
    let mut effective = builtin_pillars();
    if !raw.disable_builtin.is_empty() {
        effective.retain(|p| !raw.disable_builtin.iter().any(|id| id == p.id.as_str()));
    }
    for row in &raw.pillars {
        apply_row(&mut effective, row)?;
    }
    if effective.is_empty() {
        return Err(
            "effective rubric is empty; define at least one pillar or remove disable_builtin"
                .into(),
        );
    }
    Ok(effective)
}

fn apply_row(effective: &mut Vec<Pillar>, row: &MethodologyRubricPillar) -> Result<(), String> {
    let id = row.id.trim();
    if id.is_empty() {
        return Err("rubric pillar id must be non-empty".into());
    }
    let existing_index = effective.iter().position(|p| p.id.as_str() == id);
    let existing = existing_index.and_then(|idx| effective.get(idx));
    let name = row
        .name
        .as_deref()
        .or_else(|| existing.map(|p| p.name.as_str()))
        .ok_or_else(|| format!("rubric pillar `{id}` missing required field `name`"))?;
    let task_description = row
        .task_description
        .as_deref()
        .or_else(|| existing.map(|p| p.task_description.as_str()))
        .ok_or_else(|| format!("rubric pillar `{id}` missing required field `task_description`"))?;
    let spec_description = row
        .spec_description
        .as_deref()
        .or_else(|| existing.map(|p| p.spec_description.as_str()))
        .ok_or_else(|| format!("rubric pillar `{id}` missing required field `spec_description`"))?;
    let target_score = row
        .target_score
        .or_else(|| existing.map(|p| p.target_score.get()))
        .ok_or_else(|| format!("rubric pillar `{id}` missing required field `target_score`"))?;
    let passing_score = row
        .passing_score
        .or_else(|| existing.map(|p| p.passing_score.get()))
        .ok_or_else(|| format!("rubric pillar `{id}` missing required field `passing_score`"))?;
    let applicable_at = row
        .applicable_at
        .or_else(|| existing.map(|p| p.applicable_at))
        .ok_or_else(|| format!("rubric pillar `{id}` missing required field `applicable_at`"))?;

    let pillar = Pillar {
        id: PillarId::try_new(id).map_err(|e| e.to_string())?,
        name: NonEmptyString::try_new(name).map_err(|e| e.to_string())?,
        task_description: NonEmptyString::try_new(task_description).map_err(|e| e.to_string())?,
        spec_description: NonEmptyString::try_new(spec_description).map_err(|e| e.to_string())?,
        target_score: PillarScore::try_new(target_score).map_err(|e| e.to_string())?,
        passing_score: PillarScore::try_new(passing_score).map_err(|e| e.to_string())?,
        applicable_at,
    };
    if let Some(idx) = existing_index {
        effective[idx] = pillar;
    } else {
        effective.push(pillar);
    }
    Ok(())
}
