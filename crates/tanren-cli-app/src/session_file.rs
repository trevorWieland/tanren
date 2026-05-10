use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

pub(crate) fn read_session(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("read session from {}", path.display()))
}

pub(crate) fn persist_session(path: &Path, token: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create session dir {}", parent.display()))?;
    }
    reject_symlink_destination(path)?;

    let (scratch, mut file) = create_private_scratch(path)?;
    file.write_all(token.as_bytes())
        .with_context(|| format!("write session bytes to {}", scratch.display()))?;
    file.sync_all()
        .with_context(|| format!("sync session bytes to {}", scratch.display()))?;
    drop(file);

    if let Err(err) = fs::rename(&scratch, path) {
        let _ = fs::remove_file(&scratch);
        return Err(err).with_context(|| {
            format!(
                "replace session destination {} from {}",
                path.display(),
                scratch.display()
            )
        });
    }
    Ok(())
}

fn reject_symlink_destination(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                bail!(
                    "refuse to write session token to symlink destination {}",
                    path.display()
                );
            }
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("inspect session path {}", path.display())),
    }
}

fn session_scratch_path(path: &Path, attempt: u8) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    let pid = std::process::id();
    let mut scratch = path.as_os_str().to_owned();
    scratch.push(format!(".{pid}.{millis}.{attempt}.tmp"));
    PathBuf::from(scratch)
}

#[cfg(unix)]
fn create_private_scratch(path: &Path) -> Result<(PathBuf, fs::File)> {
    use std::os::unix::fs::OpenOptionsExt;

    for attempt in 0..16_u8 {
        let scratch = session_scratch_path(path, attempt);
        let created = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&scratch);
        match created {
            Ok(file) => return Ok((scratch, file)),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("create restricted session file {}", scratch.display())
                });
            }
        }
    }
    bail!(
        "failed to allocate unique scratch session file for {}",
        path.display()
    )
}

#[cfg(not(unix))]
fn create_private_scratch(path: &Path) -> Result<(PathBuf, fs::File)> {
    for attempt in 0..16_u8 {
        let scratch = session_scratch_path(path, attempt);
        let created = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&scratch);
        match created {
            Ok(file) => return Ok((scratch, file)),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("create session file {}", scratch.display()));
            }
        }
    }
    bail!(
        "failed to allocate unique scratch session file for {}",
        path.display()
    )
}
