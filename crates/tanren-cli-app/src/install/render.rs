use crate::install::assets;
use crate::install::model::IntegrationName;

pub(crate) struct RenderedIntegration {
    pub(crate) relative_path: String,
    pub(crate) content: String,
}

pub(crate) fn render_all_integrations(
    integrations: &[IntegrationName],
) -> Vec<RenderedIntegration> {
    let commands = assets::command_files();
    let mut rendered = Vec::new();

    for integration in integrations {
        for cmd in &commands {
            let dest_dir = integration_dest_dir(*integration);
            let file_name = cmd_file_name(&cmd.relative_path);
            let content = String::from_utf8_lossy(cmd.content).into_owned();
            rendered.push(RenderedIntegration {
                relative_path: format!("{dest_dir}/{file_name}"),
                content,
            });
        }
    }

    rendered
}

fn integration_dest_dir(integration: IntegrationName) -> &'static str {
    match integration {
        IntegrationName::Claude => ".claude/commands",
        IntegrationName::Codex => ".codex/skills",
        IntegrationName::Opencode => ".opencode/commands",
    }
}

fn cmd_file_name(relative_path: &str) -> String {
    relative_path
        .strip_prefix("commands/")
        .unwrap_or(relative_path)
        .to_owned()
}
