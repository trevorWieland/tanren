use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};

static TEMP_REPO_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
pub struct TempRepo {
    root: PathBuf,
}

impl TempRepo {
    pub fn create(prefix: &str) -> Result<Self> {
        let id = TEMP_REPO_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("{prefix}-{}-{id}", std::process::id()));
        if root.exists() {
            std::fs::remove_dir_all(&root)
                .with_context(|| format!("remove existing temp repo {}", root.display()))?;
        }
        std::fs::create_dir_all(&root)
            .with_context(|| format!("create temp repo {}", root.display()))?;
        Ok(Self { root })
    }

    pub fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
