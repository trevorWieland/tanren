use std::path::{Path, PathBuf};

use tanren_testkit::process::{CommandOutput, TanrenCliBinary};
use tanren_testkit::temp_repo::TempRepo;

#[derive(Debug, Default, cucumber::World)]
pub struct BehaviorWorld {
    pub installer_repo: Option<TempRepo>,
    pub installer_output: Option<CommandOutput>,
    pub installer_snapshot: Vec<(String, String)>,
    pub lifecycle_database_url: Option<String>,
    pub lifecycle_spec_folder: Option<PathBuf>,
    pub lifecycle_config_path: Option<PathBuf>,
    pub lifecycle_task_id: Option<String>,
    pub lifecycle_finding_id: Option<String>,
    pub lifecycle_check_run_id: Option<String>,
    pub lifecycle_attempt_id: Option<String>,
    pub lifecycle_root_cause_id: Option<String>,
}

impl BehaviorWorld {
    pub fn repo_path(&self) -> &Path {
        self.installer_repo
            .as_ref()
            .map_or_else(|| Path::new(""), TempRepo::path)
    }

    pub fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_default()
    }

    pub fn run_cli(&mut self, args: Vec<String>, repo_arg: bool) {
        let binary = TanrenCliBinary::from_env(Self::workspace_root());
        let repo = repo_arg.then_some(self.repo_path());
        self.installer_output = Some(binary.run(args, repo).unwrap_or_else(|err| CommandOutput {
            status: Some(127),
            stdout: String::new(),
            stderr: err.to_string(),
        }));
    }
}
