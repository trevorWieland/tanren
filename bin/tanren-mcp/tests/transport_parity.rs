include!("../../tanren-cli/tests/cli_mcp_parity_impl.inc");

#[derive(Clone, Copy)]
struct InvalidToolRoute {
    tool: &'static str,
    noun: &'static str,
    verb: &'static str,
}

fn invalid_tool_routes() -> Vec<InvalidToolRoute> {
    all_tool_descriptors()
        .iter()
        .map(|descriptor| InvalidToolRoute {
            tool: descriptor.name,
            noun: descriptor.cli_noun,
            verb: descriptor.cli_verb,
        })
        .collect()
}

impl McpSession {
    fn call_expect_error_json(&mut self, tool: &str, args: &Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        send_frame(
            &mut self.child,
            &json!({
                "jsonrpc":"2.0",
                "id": id,
                "method":"tools/call",
                "params":{"name":tool,"arguments":args}
            }),
        );
        let resp = read_response_for_id(&mut self.reader, id);
        assert_eq!(
            resp["result"]["isError"],
            json!(true),
            "mcp call {tool} should fail for invalid input: {resp:?}"
        );
        let text = resp["result"]["content"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v["text"].as_str())
            .expect("tool error text");
        serde_json::from_str(text).expect("tool error json")
    }
}

fn run_cli_tool_expect_error(
    url: &str,
    spec_folder: &Path,
    noun: &str,
    verb: &str,
    params: &Value,
) -> Value {
    let payload = serde_json::to_string(params).expect("serialize payload");
    let spec_id = fixed_spec_id().to_string();
    let out = Command::cargo_bin("tanren-cli")
        .expect("bin")
        .env("TANREN_PHASE_CAPABILITIES", MCP_CAPABILITIES)
        .args([
            "--database-url",
            url,
            "methodology",
            "--phase",
            "do-task",
            "--spec-id",
            spec_id.as_str(),
            "--spec-folder",
            spec_folder.to_str().expect("utf8 path"),
            "--agent-session-id",
            MCP_AGENT_SESSION_ID,
            noun,
            verb,
            "--json",
            &payload,
        ])
        .output()
        .expect("run cli tool");
    assert!(
        !out.status.success(),
        "cli {noun} {verb} should fail for invalid input"
    );
    serde_json::from_slice(&out.stderr).expect("parse cli stderr error json")
}

async fn assert_cli_invalid_tool_no_side_effects(
    url: &str,
    spec_folder: &Path,
    noun: &str,
    verb: &str,
) -> Value {
    let before_events = methodology_envelopes(url).await.len();
    let before_phase_lines = phase_event_lines(spec_folder).len();
    let error = run_cli_tool_expect_error(url, spec_folder, noun, verb, &json!({}));
    let after_events = methodology_envelopes(url).await.len();
    let after_phase_lines = phase_event_lines(spec_folder).len();
    assert_eq!(
        after_events, before_events,
        "invalid cli tool `{noun} {verb}` must not append methodology events",
    );
    assert_eq!(
        after_phase_lines, before_phase_lines,
        "invalid cli tool `{noun} {verb}` must not append phase-events lines",
    );
    error
}

async fn assert_mcp_invalid_tool_no_side_effects(
    session: &mut McpSession,
    url: &str,
    spec_folder: &Path,
    tool: &str,
) -> Value {
    let before_events = methodology_envelopes(url).await.len();
    let before_phase_lines = phase_event_lines(spec_folder).len();
    let args = json!({});
    let error = session.call_expect_error_json(tool, &args);
    let after_events = methodology_envelopes(url).await.len();
    let after_phase_lines = phase_event_lines(spec_folder).len();
    assert_eq!(
        after_events, before_events,
        "invalid mcp tool `{tool}` must not append methodology events",
    );
    assert_eq!(
        after_phase_lines, before_phase_lines,
        "invalid mcp tool `{tool}` must not append phase-events lines",
    );
    error
}

#[tokio::test]
async fn cli_and_mcp_match_invalid_input_rejection_for_full_tool_matrix() {
    let (_d1, cli_url) = mkdb("cli-invalid");
    let (_d2, mcp_url) = mkdb("mcp-invalid");
    let spec_id = fixed_spec_id();
    let cli_root = tempfile::tempdir().expect("tempdir");
    let mcp_root = tempfile::tempdir().expect("tempdir");
    let cli_spec_folder = cli_root
        .path()
        .join(format!("2026-01-01-0101-{spec_id}-cli-invalid"));
    let mcp_spec_folder = mcp_root
        .path()
        .join(format!("2026-01-01-0101-{spec_id}-mcp-invalid"));
    std::fs::create_dir_all(&cli_spec_folder).expect("mkdir cli spec folder");
    std::fs::create_dir_all(&mcp_spec_folder).expect("mkdir mcp spec folder");
    std::fs::write(cli_spec_folder.join("phase-events.jsonl"), "").expect("seed cli phase-events");
    std::fs::write(mcp_spec_folder.join("phase-events.jsonl"), "").expect("seed mcp phase-events");

    let mut registry_probe = McpSession::start(&mcp_url, &mcp_spec_folder, "do-task");
    let registry_tools = registry_probe.list_tools();
    assert_registry_tool_coverage(&registry_tools);
    drop(registry_probe);

    let mut mcp = McpSession::start(&mcp_url, &mcp_spec_folder, "do-task");
    let invalid_routes = invalid_tool_routes();
    let mut cli_errors = Vec::with_capacity(invalid_routes.len());
    let mut mcp_errors = Vec::with_capacity(invalid_routes.len());
    for route in &invalid_routes {
        let cli_error = assert_cli_invalid_tool_no_side_effects(
            &cli_url,
            &cli_spec_folder,
            route.noun,
            route.verb,
        )
        .await;
        let mcp_error = assert_mcp_invalid_tool_no_side_effects(
            &mut mcp,
            &mcp_url,
            &mcp_spec_folder,
            route.tool,
        )
        .await;
        assert_eq!(
            cli_error["kind"],
            json!("validation_failed"),
            "cli invalid result kind mismatch for {}",
            route.tool
        );
        assert_eq!(
            mcp_error["kind"],
            json!("validation_failed"),
            "mcp invalid result kind mismatch for {}",
            route.tool
        );
        cli_errors.push(cli_error);
        mcp_errors.push(mcp_error);
    }
    assert_eq!(
        cli_errors.len(),
        parity_covered_tools().len(),
        "cli invalid matrix must cover all tools"
    );
    assert_eq!(
        mcp_errors.len(),
        parity_covered_tools().len(),
        "mcp invalid matrix must cover all tools"
    );
}
