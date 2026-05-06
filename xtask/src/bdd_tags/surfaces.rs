//! Surface registry loading for BDD tag validation.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const DEFAULT_SURFACE_IDS: &[&str] = &["web", "api", "mcp", "cli", "tui"];

#[derive(Debug)]
pub(super) struct SurfaceRegistry {
    ids: BTreeSet<String>,
}

impl SurfaceRegistry {
    pub(super) fn load(workspace_root: &Path) -> Result<Self> {
        let path = workspace_root
            .join("docs")
            .join("experience")
            .join("surfaces.yml");
        if !path.exists() {
            return Ok(Self::tanren_default());
        }
        let content =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let parsed: SurfaceRegistryFile =
            serde_yaml::from_str(&content).with_context(|| format!("parse {}", path.display()))?;
        let mut ids = BTreeSet::new();
        for surface in parsed.surfaces {
            let id = surface.id.trim();
            if id.is_empty() {
                bail!("{}: surface id must not be empty", path.display());
            }
            if !valid_surface_id(id) {
                bail!(
                    "{}: surface id {id:?} must use lowercase ASCII letters, digits, hyphen, or underscore and start with a letter",
                    path.display()
                );
            }
            if !ids.insert(id.to_owned()) {
                bail!("{}: duplicate surface id {id:?}", path.display());
            }
        }
        if ids.is_empty() {
            bail!("{}: at least one surface is required", path.display());
        }
        Ok(Self { ids })
    }

    pub(super) fn contains(&self, id: &str) -> bool {
        self.ids.contains(id)
    }

    pub(super) fn tag_display(&self) -> String {
        self.ids
            .iter()
            .map(|id| format!("@{id}"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn tanren_default() -> Self {
        Self {
            ids: DEFAULT_SURFACE_IDS
                .iter()
                .map(|id| (*id).to_owned())
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct SurfaceRegistryFile {
    surfaces: Vec<SurfaceRecord>,
}

#[derive(Debug, Deserialize)]
struct SurfaceRecord {
    id: String,
}

fn valid_surface_id(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}
