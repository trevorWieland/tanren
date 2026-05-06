use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::ConfigSecretsError;

const CONFIG_FILENAME: &str = "tanren.yml";

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    pub schema: String,
    pub standards: StandardsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StandardsConfig {
    pub root: PathBuf,
}

impl ProjectConfig {
    pub fn load(project_dir: &Path) -> Result<Self, ConfigSecretsError> {
        let config_path = project_dir.join(CONFIG_FILENAME);
        let contents = std::fs::read_to_string(&config_path).map_err(|e| {
            ConfigSecretsError::ProjectConfigError {
                message: format!("cannot read {}: {e}", config_path.display()),
            }
        })?;
        let config: Self = serde_yaml::from_str(&contents).map_err(|e| {
            ConfigSecretsError::ProjectConfigError {
                message: format!("invalid config at {}: {e}", config_path.display()),
            }
        })?;
        if config.schema != "tanren.project.v0" {
            return Err(ConfigSecretsError::ProjectConfigError {
                message: format!(
                    "unsupported schema '{}' in {} (expected tanren.project.v0)",
                    config.schema,
                    config_path.display()
                ),
            });
        }
        Ok(config)
    }

    pub fn standards_root(&self, project_dir: &Path) -> PathBuf {
        project_dir.join(&self.standards.root)
    }
}
