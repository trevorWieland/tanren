use std::path::Path;

use anyhow::{Context, Result};

pub fn write_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create parent directory {}", parent.display()))?;
    }
    std::fs::write(path, contents).with_context(|| format!("write {}", path.display()))
}

pub fn read_file(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))
}

pub fn remove_file(path: &Path) -> Result<()> {
    std::fs::remove_file(path).with_context(|| format!("remove {}", path.display()))
}

pub fn collect_file_snapshot(root: &Path) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    collect_file_snapshot_inner(root, root, &mut out)?;
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn collect_file_snapshot_inner(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, String)>,
) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("read dir {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_file_snapshot_inner(root, &path, out)?;
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .with_context(|| format!("strip prefix {}", path.display()))?
            .to_string_lossy()
            .to_string();
        out.push((rel, read_file(&path)?));
    }
    Ok(())
}
