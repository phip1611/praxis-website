// SPDX-License-Identifier: MIT OR Apache-2.0

//! JSON files in the state directory.
//!
//! The data of this service is small enough for plain files, so there is no
//! database. Writes are atomic: a crash or power loss leaves either the old or
//! the new file, never a truncated one.

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// A failure while reading or writing a state file.
#[derive(Debug, Error)]
pub(crate) enum StoreError {
    /// An I/O operation failed.
    #[error("{operation} {path}: {source}")]
    Io {
        /// What was attempted, e.g., "can't read".
        operation: &'static str,
        /// The affected path.
        path: PathBuf,
        /// The underlying error.
        source: io::Error,
    },
    /// The file doesn't contain valid data.
    #[error("{path} is corrupt: {source}")]
    Corrupt {
        /// The affected path.
        path: PathBuf,
        /// The underlying error.
        source: serde_json::Error,
    },
}

/// Reads and parses the JSON file at `path`, or returns `None` if it doesn't
/// exist.
///
/// # Errors
///
/// Returns [`StoreError`] if the file exists but can't be read or parsed.
pub(crate) fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, StoreError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(StoreError::Io {
                operation: "can't read",
                path: path.to_path_buf(),
                source,
            });
        }
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|source| StoreError::Corrupt {
            path: path.to_path_buf(),
            source,
        })
}

/// Writes `value` as JSON to `path`, atomically and durably.
///
/// Writes a hidden temporary file next to `path`, flushes it, renames it over
/// `path`, and flushes the directory, so that the rename survives a power
/// loss. Callers must not write the same file concurrently.
///
/// # Errors
///
/// Returns [`StoreError`] if a step fails. The previous file stays intact.
pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), StoreError> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("state data should serialize");
    bytes.push(b'\n');
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .expect("state file paths should have a file name")
        .to_string_lossy();
    let temp = dir.join(format!(".{file_name}.tmp"));
    let io_error = |operation, path: &Path| {
        let path = path.to_path_buf();
        move |source| StoreError::Io {
            operation,
            path,
            source,
        }
    };

    let mut file = File::create(&temp).map_err(io_error("can't create", &temp))?;
    file.write_all(&bytes)
        .map_err(io_error("can't write", &temp))?;
    file.sync_all().map_err(io_error("can't flush", &temp))?;
    fs::rename(&temp, path).map_err(io_error("can't replace", path))?;
    File::open(dir)
        .and_then(|dir| dir.sync_all())
        .map_err(io_error("can't flush", dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;
    use tempfile::TempDir;

    /// Creates a temporary directory and the path of a file in it.
    fn temp_file() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("temp dir should be created");
        let path = dir.path().join("data.json");
        (dir, path)
    }

    /// A missing file is no error, as on first start.
    #[test]
    fn read_returns_none_without_file() {
        let (_dir, path) = temp_file();

        assert_matches!(read_json::<Vec<u32>>(&path), Ok(None));
    }

    /// What is written is read again, and no temporary file stays behind.
    #[test]
    fn write_then_read_round_trips() {
        let (dir, path) = temp_file();

        write_json(&path, &vec![1, 2, 3]).expect("write should succeed");
        write_json(&path, &vec![4]).expect("write should succeed");

        let value = read_json::<Vec<u32>>(&path)
            .expect("the file should be readable")
            .expect("the file should exist");
        assert_eq!(value, [4]);
        assert!(!dir.path().join(".data.json.tmp").exists());
    }

    /// A damaged file is reported instead of being silently replaced.
    #[test]
    fn read_reports_corrupt_file() {
        let (_dir, path) = temp_file();
        fs::write(&path, "{ not json").expect("write should succeed");

        assert_matches!(
            read_json::<Vec<u32>>(&path),
            Err(StoreError::Corrupt { .. })
        );
    }

    /// A missing directory is reported with the path, for the operator.
    #[test]
    fn write_reports_missing_directory() {
        let path = Path::new("/does/not/exist/data.json");

        let Err(StoreError::Io { path: reported, .. }) = write_json(path, &1) else {
            panic!("the value should match `Err(StoreError::Io {{ path: reported, .. }})`");
        };
        assert!(reported.starts_with("/does/not/exist"));
    }
}
