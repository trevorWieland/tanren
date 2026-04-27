use std::collections::BTreeMap;
use std::path::Path;

use tanren_app_services::methodology::config::{InstallFormat, MergePolicy, MethodologyConfig};
use tanren_app_services::methodology::formats::{
    claude_mcp_json, codex_config_toml, opencode_json,
};
use tanren_app_services::methodology::installer::PlannedWrite;

const MCP_SERVER_COMMAND: &str = "tanren-mcp";
const MCP_SERVER_ARGS: &[&str] = &["serve"];

pub(crate) fn synth_mcp_write(
    path: &Path,
    format: InstallFormat,
    server_env: &BTreeMap<String, String>,
) -> Result<Option<PlannedWrite>, String> {
    let existing = std::fs::read_to_string(path).ok();
    let server_args: Vec<String> = MCP_SERVER_ARGS.iter().map(|v| (*v).to_owned()).collect();
    let bytes = match format {
        InstallFormat::ClaudeMcpJson => claude_mcp_json(
            existing.as_deref(),
            MCP_SERVER_COMMAND,
            &server_args,
            server_env,
        ),
        InstallFormat::CodexConfigToml => codex_config_toml(
            existing.as_deref(),
            MCP_SERVER_COMMAND,
            &server_args,
            server_env,
        ),
        InstallFormat::OpencodeJson => opencode_json(
            existing.as_deref(),
            MCP_SERVER_COMMAND,
            &server_args,
            server_env,
        ),
        _ => return Ok(None),
    };
    match bytes {
        Ok(b) => Ok(Some(PlannedWrite {
            dest: path.to_path_buf(),
            bytes: b,
            merge_policy: MergePolicy::PreserveOtherKeys,
            format,
        })),
        Err(err) => Err(err.to_string()),
    }
}

pub(crate) fn mcp_server_env(cfg: &MethodologyConfig) -> BTreeMap<String, String> {
    let mut out = BTreeMap::from([("TANREN_CONFIG".to_owned(), "./tanren.yml".to_owned())]);
    if let Some(issuer) = cfg.mcp.security.capability_issuer.as_deref()
        && !issuer.trim().is_empty()
    {
        out.insert(
            "TANREN_MCP_CAPABILITY_ISSUER".to_owned(),
            issuer.trim().to_owned(),
        );
    }
    if let Some(audience) = cfg.mcp.security.capability_audience.as_deref()
        && !audience.trim().is_empty()
    {
        out.insert(
            "TANREN_MCP_CAPABILITY_AUDIENCE".to_owned(),
            audience.trim().to_owned(),
        );
    }
    if let Some(public_key_file) = cfg.mcp.security.capability_public_key_file.as_ref() {
        let value = public_key_file.to_string_lossy().trim().to_owned();
        if !value.is_empty() {
            out.insert("TANREN_MCP_CAPABILITY_PUBLIC_KEY_FILE".to_owned(), value);
        }
    }
    if let Some(max_ttl_secs) = cfg.mcp.security.capability_max_ttl_secs {
        out.insert(
            "TANREN_MCP_CAPABILITY_MAX_TTL_SECS".to_owned(),
            max_ttl_secs.to_string(),
        );
    }
    out
}
