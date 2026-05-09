use std::env;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const SESSION_FILE_ENV: &str = "TANREN_SESSION_FILE";
const NEW_FILE_MODE: u32 = 0o600;
const NEW_DIR_MODE: u32 = 0o700;
const TEMP_FILE_ATTEMPTS: u32 = 16;

#[derive(Debug, Clone)]
pub struct SessionFileStore {
    path: PathBuf,
}

impl SessionFileStore {
    #[must_use]
    pub const fn new(path: PathBuf) -> Self {
        Self { path }
    }

    #[must_use]
    pub fn from_env() -> Self {
        Self::new(default_session_path())
    }

    pub fn read_json_or_default<T, F>(&self, legacy_fallback: F) -> Result<T, SessionFileStoreError>
    where
        T: DeserializeOwned + Default,
        F: FnOnce(&str) -> Option<T>,
    {
        if !self.path.exists() {
            return Ok(T::default());
        }

        let raw = fs::read_to_string(&self.path).map_err(|source| SessionFileStoreError::Read {
            path: self.path.clone(),
            source,
        })?;

        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(T::default());
        }

        if raw.trim_start().starts_with('{') {
            return serde_json::from_str(&raw).map_err(|source| SessionFileStoreError::Parse {
                path: self.path.clone(),
                source,
            });
        }

        Ok(legacy_fallback(&raw).unwrap_or_default())
    }

    pub fn write_json_pretty<T>(&self, value: &T) -> Result<(), SessionFileStoreError>
    where
        T: Serialize + ?Sized,
    {
        let body = serde_json::to_vec_pretty(value)
            .map_err(|source| SessionFileStoreError::Serialize { source })?;

        create_private_parent_dirs(&self.path)?;

        #[cfg(unix)]
        let existing_mode = self.path.metadata().ok().map(|meta| {
            let mode = meta.permissions().mode() & 0o777;
            if mode == 0 { NEW_FILE_MODE } else { mode }
        });

        let (temp_path, mut temp_file) = create_temp_file(&self.path)?;

        #[cfg(unix)]
        if let Some(mode) = existing_mode {
            fs::set_permissions(&temp_path, PermissionsExt::from_mode(mode)).map_err(|source| {
                SessionFileStoreError::SetFilePermissions {
                    path: temp_path.clone(),
                    source,
                }
            })?;
        }

        temp_file
            .write_all(&body)
            .map_err(|source| SessionFileStoreError::WriteTemp {
                path: temp_path.clone(),
                source,
            })?;
        temp_file
            .sync_all()
            .map_err(|source| SessionFileStoreError::SyncTemp {
                path: temp_path.clone(),
                source,
            })?;

        drop(temp_file);

        fs::rename(&temp_path, &self.path).map_err(|source| SessionFileStoreError::Rename {
            from: temp_path.clone(),
            to: self.path.clone(),
            source,
        })?;

        sync_parent_dir(&self.path)?;
        Ok(())
    }
}

#[must_use]
pub fn default_session_path() -> PathBuf {
    if let Ok(explicit) = env::var(SESSION_FILE_ENV) {
        if !explicit.is_empty() {
            return PathBuf::from(explicit);
        }
    }

    let base = env::var("XDG_STATE_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map_or_else(
            || {
                env::var("HOME").ok().map_or_else(
                    || PathBuf::from("."),
                    |home| PathBuf::from(home).join(".local/state"),
                )
            },
            PathBuf::from,
        );

    base.join("tanren").join("session")
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SessionFileStoreError {
    #[error("read session file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("parse session file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("serialize session file JSON: {source}")]
    Serialize {
        #[source]
        source: serde_json::Error,
    },
    #[error("create session dir {path}: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("set session dir permissions on {path}: {source}")]
    SetDirPermissions {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("create temporary session file {path}: {source}")]
    CreateTemp {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("set session file permissions on {path}: {source}")]
    SetFilePermissions {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("write temporary session file {path}: {source}")]
    WriteTemp {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("sync temporary session file {path}: {source}")]
    SyncTemp {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("rename temporary session file from {from} to {to}: {source}")]
    Rename {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("sync session directory {path}: {source}")]
    SyncDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

fn create_private_parent_dirs(path: &Path) -> Result<(), SessionFileStoreError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };

    let mut missing = Vec::new();
    let mut cursor = parent.to_path_buf();
    while !cursor.exists() {
        missing.push(cursor.clone());
        if !cursor.pop() {
            break;
        }
    }

    missing.reverse();

    for dir in missing {
        match fs::create_dir(&dir) {
            Ok(()) => {}
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(source) => {
                return Err(SessionFileStoreError::CreateDir { path: dir, source });
            }
        }

        #[cfg(unix)]
        {
            fs::set_permissions(&dir, PermissionsExt::from_mode(NEW_DIR_MODE)).map_err(
                |source| SessionFileStoreError::SetDirPermissions {
                    path: dir.clone(),
                    source,
                },
            )?;
        }
    }

    Ok(())
}

fn create_temp_file(path: &Path) -> Result<(PathBuf, File), SessionFileStoreError> {
    for attempt in 0..TEMP_FILE_ATTEMPTS {
        let temp_path = temp_file_path(path, attempt);

        let mut options = OpenOptions::new();
        options.write(true).create_new(true);

        #[cfg(unix)]
        {
            options.mode(NEW_FILE_MODE);
        }

        match options.open(&temp_path) {
            Ok(file) => return Ok((temp_path, file)),
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(source) => {
                return Err(SessionFileStoreError::CreateTemp {
                    path: temp_path,
                    source,
                });
            }
        }
    }

    Err(SessionFileStoreError::CreateTemp {
        path: path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "failed to allocate unique temporary session file path",
        ),
    })
}

fn temp_file_path(path: &Path, attempt: u32) -> PathBuf {
    let mut name = OsString::from(".");
    name.push(path.file_name().unwrap_or_default());

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map_or(0, |duration| duration.as_nanos());

    name.push(format!(".tmp.{}.{}.{}", process::id(), nanos, attempt));
    path.with_file_name(name)
}

fn sync_parent_dir(path: &Path) -> Result<(), SessionFileStoreError> {
    #[cfg(unix)]
    {
        if let Some(parent) = path.parent() {
            let dir = File::open(parent).map_err(|source| SessionFileStoreError::SyncDir {
                path: parent.to_path_buf(),
                source,
            })?;
            dir.sync_all()
                .map_err(|source| SessionFileStoreError::SyncDir {
                    path: parent.to_path_buf(),
                    source,
                })?;
        }
    }

    Ok(())
}
