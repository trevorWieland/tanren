use std::collections::HashMap;
use std::path::PathBuf;

use tanren_app_services::methodology::renderer::render_command;
use tanren_app_services::methodology::source::{CommandFamily, CommandFrontmatter, CommandSource};

#[test]
fn renderer_frontmatter_unresolved_var_reports_concrete_location() {
    let src = CommandSource {
        name: "demo".into(),
        family: CommandFamily::SpecLoop,
        frontmatter: CommandFrontmatter {
            name: "{{FRONTMATTER_NAME}}".into(),
            role: "impl".into(),
            orchestration_loop: false,
            autonomy: "autonomous".into(),
            declared_variables: vec!["FRONTMATTER_NAME".into()],
            declared_tools: vec![],
            required_capabilities: vec![],
            produces_evidence: vec![],
            description: None,
            agent: None,
            model: None,
            subtask: None,
            extensions: Default::default(),
        },
        body: "body only\n".into(),
        source_path: PathBuf::from("commands/frontmatter-demo.md"),
    };
    let ctx = HashMap::new();
    let err = render_command(&src, &ctx).expect_err("unresolved");
    let msg = err.to_string();
    assert!(
        msg.contains("commands/frontmatter-demo.md:"),
        "location should include concrete file:line:col, got: {msg}"
    );
    assert!(
        !msg.contains("<frontmatter or non-body reference>"),
        "placeholder location text must not appear, got: {msg}"
    );
}
