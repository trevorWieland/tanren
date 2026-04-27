use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use tanren_app_services::methodology::RequiredGuard;
use tanren_app_services::methodology::config::{
    EnvironmentConfig, InstallBinding, InstallFormat, InstallTarget, McpConfig, McpSecurityConfig,
    McpTransport, MergePolicy, MethodologyConfig, MethodologyRubricConfig, StandardsConfig,
    TanrenConfig,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum AgentTarget {
    Claude,
    Codex,
    Opencode,
}

impl AgentTarget {
    const ALL: [Self; 3] = [Self::Claude, Self::Codex, Self::Opencode];

    const fn label(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Opencode => "opencode",
        }
    }
}

pub(super) struct LoadedInstallConfig {
    pub(super) cfg: TanrenConfig,
    pub(super) generated_config_bytes: Option<Vec<u8>>,
}

pub(super) enum InstallLoadError {
    Config(String),
    Validation(String),
}

pub(super) fn parse_agent_targets(raw: &[String]) -> Result<Vec<AgentTarget>, String> {
    let mut out = Vec::new();
    let selected: Vec<String> = if raw.is_empty() {
        AgentTarget::ALL
            .iter()
            .map(|agent| agent.label().to_owned())
            .collect()
    } else {
        raw.to_vec()
    };
    for value in selected {
        let agent = match value.trim() {
            "claude" | "claude-code" => AgentTarget::Claude,
            "codex" | "codex-skills" => AgentTarget::Codex,
            "opencode" => AgentTarget::Opencode,
            other => return Err(format!("unknown --agents entry `{other}`")),
        };
        if !out.contains(&agent) {
            out.push(agent);
        }
    }
    out.sort();
    Ok(out)
}

pub(super) fn resolve_repo_root(raw: &Path) -> Result<PathBuf, String> {
    let root = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| format!("resolving current directory: {e}"))?
            .join(raw)
    };
    if root.exists() {
        std::fs::canonicalize(&root)
            .map_err(|e| format!("canonicalizing repo root {}: {e}", root.display()))
    } else {
        Err(format!("repo root {} does not exist", root.display()))
    }
}

pub(super) fn resolve_config_path(repo_root: &Path, raw: &Path) -> PathBuf {
    if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        repo_root.join(raw)
    }
}

pub(super) fn load_or_bootstrap_config(
    config_path: &Path,
    profile_arg: Option<&str>,
    requested_agents: &[AgentTarget],
    agents_explicit: bool,
) -> Result<LoadedInstallConfig, InstallLoadError> {
    if config_path.exists() {
        let yaml = std::fs::read_to_string(config_path).map_err(|e| {
            InstallLoadError::Config(format!("reading {}: {e}", config_path.display()))
        })?;
        let cfg = TanrenConfig::from_yaml(&yaml).map_err(|e| {
            InstallLoadError::Config(format!("parsing {}: {e}", config_path.display()))
        })?;
        validate_existing_bootstrap_args(&cfg, profile_arg, requested_agents, agents_explicit)?;
        return Ok(LoadedInstallConfig {
            cfg,
            generated_config_bytes: None,
        });
    }

    let Some(profile) = profile_arg else {
        return Err(InstallLoadError::Validation(
            "--profile is required when tanren.yml does not exist".into(),
        ));
    };
    let cfg = bootstrap_config(profile, requested_agents);
    let bytes = serde_yaml::to_string(&cfg).unwrap_or_default().into_bytes();
    Ok(LoadedInstallConfig {
        cfg,
        generated_config_bytes: Some(bytes),
    })
}

pub(super) fn validate_methodology_config(cfg: &MethodologyConfig) -> Result<(), String> {
    if !cfg.profiles.is_empty() {
        return Err("methodology.profiles is no longer supported; use methodology.standards.profile for standards selection".into());
    }
    if cfg.source.is_some() {
        return Err("methodology.source is no longer supported; commands are embedded Tanren distribution assets".into());
    }
    if cfg
        .install_targets
        .iter()
        .any(|target| target.format == InstallFormat::StandardsProfile)
        && cfg.standards.profile.as_deref().is_none()
    {
        return Err("standards-profile target requires methodology.standards.profile".into());
    }
    if cfg
        .install_targets
        .iter()
        .any(|target| target.format == InstallFormat::StandardsBaseline)
    {
        return Err("`standards-baseline` is no longer supported; use `standards-profile`".into());
    }
    Ok(())
}

pub(super) fn resolve_install_paths(cfg: &mut MethodologyConfig, repo_root: &Path) {
    for target in &mut cfg.install_targets {
        target.path = resolve_repo_relative_path(repo_root, &target.path);
    }
    for target in &mut cfg.mcp.also_write_configs {
        target.path = resolve_repo_relative_path(repo_root, &target.path);
    }
}

fn validate_existing_bootstrap_args(
    cfg: &TanrenConfig,
    profile_arg: Option<&str>,
    requested_agents: &[AgentTarget],
    agents_explicit: bool,
) -> Result<(), InstallLoadError> {
    if let Some(profile) = profile_arg {
        let configured = cfg.methodology.standards.profile.as_deref().ok_or_else(|| {
            InstallLoadError::Validation(
                "--profile cannot be used with an existing config that does not record methodology.standards.profile".into(),
            )
        })?;
        if configured != profile {
            return Err(InstallLoadError::Validation(format!(
                "--profile `{profile}` conflicts with existing methodology.standards.profile `{configured}`"
            )));
        }
    }
    if agents_explicit {
        let configured = configured_agents(&cfg.methodology);
        if configured != requested_agents {
            return Err(InstallLoadError::Validation(format!(
                "--agents {:?} conflicts with existing configured agents {:?}",
                agent_labels(requested_agents),
                agent_labels(&configured)
            )));
        }
    }
    Ok(())
}

fn configured_agents(cfg: &MethodologyConfig) -> Vec<AgentTarget> {
    let mut out = Vec::new();
    for target in &cfg.install_targets {
        match target.format {
            InstallFormat::ClaudeCode => out.push(AgentTarget::Claude),
            InstallFormat::CodexSkills => out.push(AgentTarget::Codex),
            InstallFormat::Opencode => out.push(AgentTarget::Opencode),
            _ => {}
        }
    }
    out.sort();
    out.dedup();
    out
}

fn agent_labels(agents: &[AgentTarget]) -> Vec<&'static str> {
    agents.iter().map(|agent| agent.label()).collect()
}

fn bootstrap_config(profile: &str, agents: &[AgentTarget]) -> TanrenConfig {
    TanrenConfig {
        methodology: MethodologyConfig {
            task_complete_requires: vec![
                RequiredGuard::GateChecked,
                RequiredGuard::Audited,
                RequiredGuard::Adherent,
            ],
            source: None,
            standards: StandardsConfig {
                profile: Some(profile.to_owned()),
            },
            install_targets: bootstrap_install_targets(agents),
            mcp: McpConfig {
                enabled: true,
                transport: McpTransport::Stdio,
                also_write_configs: bootstrap_mcp_targets(agents),
                security: McpSecurityConfig {
                    capability_issuer: Some("tanren-phase0".to_owned()),
                    capability_audience: Some("tanren-mcp".to_owned()),
                    capability_public_key_file: Some(
                        ".tanren/mcp-capability-public-key.pem".into(),
                    ),
                    capability_private_key_file: Some(
                        ".tanren/mcp-capability-private-key.pem".into(),
                    ),
                    capability_max_ttl_secs: Some(900),
                },
            },
            rubric: MethodologyRubricConfig::default(),
            variables: bootstrap_variables(profile),
            profiles: BTreeMap::new(),
        },
        environment: EnvironmentConfig::default(),
        other: BTreeMap::new(),
    }
}

fn bootstrap_variables(profile: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("task_verification_hook".to_owned(), "just check".to_owned()),
        ("spec_verification_hook".to_owned(), "just ci".to_owned()),
        ("issue_provider".to_owned(), "GitHub".to_owned()),
        (
            "project_language".to_owned(),
            project_language_for_profile(profile).to_owned(),
        ),
        ("spec_root".to_owned(), "tanren/specs".to_owned()),
        ("product_root".to_owned(), "tanren/product".to_owned()),
        ("standards_root".to_owned(), "tanren/standards".to_owned()),
        ("agent_cli_noun".to_owned(), "the agent CLI".to_owned()),
        ("task_tool_binding".to_owned(), "mcp".to_owned()),
        (
            "phase_events_file".to_owned(),
            "phase-events.jsonl".to_owned(),
        ),
    ])
}

fn bootstrap_install_targets(agents: &[AgentTarget]) -> Vec<InstallTarget> {
    let mut targets = Vec::new();
    for agent in agents {
        match agent {
            AgentTarget::Claude => targets.push(command_target(
                ".claude/commands",
                InstallFormat::ClaudeCode,
            )),
            AgentTarget::Codex => {
                targets.push(command_target(".codex/skills", InstallFormat::CodexSkills));
            }
            AgentTarget::Opencode => {
                targets.push(command_target(
                    ".opencode/commands",
                    InstallFormat::Opencode,
                ));
            }
        }
    }
    targets.push(InstallTarget {
        path: "tanren/standards".into(),
        format: InstallFormat::StandardsProfile,
        binding: InstallBinding::None,
        merge_policy: MergePolicy::PreserveExisting,
    });
    targets
}

fn bootstrap_mcp_targets(agents: &[AgentTarget]) -> Vec<InstallTarget> {
    let mut targets = Vec::new();
    for agent in agents {
        match agent {
            AgentTarget::Claude => {
                targets.push(config_target(".mcp.json", InstallFormat::ClaudeMcpJson));
            }
            AgentTarget::Codex => {
                targets.push(config_target(
                    ".codex/config.toml",
                    InstallFormat::CodexConfigToml,
                ));
            }
            AgentTarget::Opencode => {
                targets.push(config_target("opencode.json", InstallFormat::OpencodeJson));
            }
        }
    }
    targets
}

fn command_target(path: &str, format: InstallFormat) -> InstallTarget {
    InstallTarget {
        path: path.into(),
        format,
        binding: InstallBinding::Mcp,
        merge_policy: MergePolicy::Destructive,
    }
}

fn config_target(path: &str, format: InstallFormat) -> InstallTarget {
    InstallTarget {
        path: path.into(),
        format,
        binding: InstallBinding::None,
        merge_policy: MergePolicy::PreserveOtherKeys,
    }
}

fn project_language_for_profile(profile: &str) -> &'static str {
    match profile {
        "react-ts-pnpm" => "typescript",
        _ => "rust",
    }
}

fn resolve_repo_relative_path(repo_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}
