use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(crate) struct MethodologyRuntimeSettings {
    pub(crate) required_guards: Vec<tanren_app_services::methodology::RequiredGuard>,
    pub(crate) standards_root: PathBuf,
    pub(crate) pillars: Vec<tanren_app_services::methodology::Pillar>,
    pub(crate) issue_provider: String,
    pub(crate) runtime_tuning: tanren_app_services::methodology::MethodologyRuntimeTuning,
}

pub(crate) fn load_methodology_runtime_settings(
    config_path: &Path,
) -> anyhow::Result<MethodologyRuntimeSettings> {
    let default_root = resolve_relative_to_config(config_path, Path::new("tanren/standards"));
    if !config_path.exists() {
        return Ok(MethodologyRuntimeSettings {
            required_guards: vec![
                tanren_app_services::methodology::RequiredGuard::GateChecked,
                tanren_app_services::methodology::RequiredGuard::Audited,
                tanren_app_services::methodology::RequiredGuard::Adherent,
            ],
            standards_root: default_root,
            pillars: tanren_app_services::methodology::builtin_pillars(),
            issue_provider: "GitHub".to_owned(),
            runtime_tuning: tanren_app_services::methodology::MethodologyRuntimeTuning::default(),
        });
    }

    let raw = std::fs::read_to_string(config_path).map_err(|e| {
        anyhow::anyhow!("reading methodology config {}: {e}", config_path.display())
    })?;
    let cfg =
        tanren_app_services::methodology::config::TanrenConfig::from_yaml(&raw).map_err(|e| {
            anyhow::anyhow!("parsing methodology config {}: {e}", config_path.display())
        })?;
    let standards_raw = cfg
        .methodology
        .variables
        .get("standards_root")
        .or_else(|| cfg.methodology.variables.get("STANDARDS_ROOT"))
        .map_or("tanren/standards", String::as_str);
    let pillars = tanren_app_services::methodology::rubric_registry::effective_pillars_for_runtime(
        config_path,
        &cfg,
    )
    .map_err(|e| {
        anyhow::anyhow!(
            "resolving rubric pillars from {}: {e}",
            config_path.display()
        )
    })?;

    let mut runtime_tuning = tanren_app_services::methodology::MethodologyRuntimeTuning::default();
    if let Some(fsync_every) =
        parse_variable::<u32>(&cfg.methodology.variables, "phase_events_fsync_every")
        && let Some(policy) =
            tanren_app_services::methodology::PhaseEventsAppendPolicy::from_fsync_every(fsync_every)
    {
        runtime_tuning.phase_events_append_policy = policy;
    }
    if let Some(min_lines) = parse_variable::<usize>(
        &cfg.methodology.variables,
        "phase_events_compaction_min_lines",
    ) {
        runtime_tuning.phase_events_compaction_min_lines = min_lines.max(1);
    }
    if let Some(threshold) = parse_variable::<usize>(
        &cfg.methodology.variables,
        "projection_checkpoint_compaction_append_threshold",
    ) {
        runtime_tuning.projection_checkpoint_compaction_append_threshold = threshold.max(1);
    }

    Ok(MethodologyRuntimeSettings {
        required_guards: cfg.methodology.task_complete_requires,
        standards_root: resolve_relative_to_config(config_path, Path::new(standards_raw)),
        pillars,
        issue_provider: cfg
            .methodology
            .variables
            .get("issue_provider")
            .or_else(|| cfg.methodology.variables.get("ISSUE_PROVIDER"))
            .cloned()
            .unwrap_or_else(|| "GitHub".to_owned()),
        runtime_tuning,
    })
}

fn parse_variable<T>(vars: &BTreeMap<String, String>, key: &str) -> Option<T>
where
    T: std::str::FromStr,
{
    vars.get(key)
        .or_else(|| vars.get(&key.to_ascii_uppercase()))
        .and_then(|raw| raw.trim().parse::<T>().ok())
}

fn resolve_relative_to_config(config_path: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    let base = config_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    base.join(path)
}
