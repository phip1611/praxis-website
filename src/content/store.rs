// SPDX-License-Identifier: MIT OR Apache-2.0

//! Persistence of the news in a JSON file in the state directory.

use crate::content::news::NewsDocument;
use crate::persist::{self, StoreError};
use std::fmt::Debug;
use std::path::{Path, PathBuf};

/// The name of the state file.
const FILE_NAME: &str = "aktuelle-informationen.json";

/// Loads and saves the news.
///
/// A trait, so that tests can use an in-memory store and simulate failures.
pub(crate) trait NewsStore: Debug + Send + Sync {
    /// Loads the saved news, or `None` if nothing was saved yet.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the data exists but can't be read.
    fn load(&self) -> Result<Option<NewsDocument>, StoreError>;

    /// Saves the news, replacing the previous version.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the data can't be written. The previous
    /// version stays intact then.
    fn save(&self, document: &NewsDocument) -> Result<(), StoreError>;
}

/// Stores the news as JSON file in the state directory.
#[derive(Debug)]
pub(crate) struct JsonFileStore {
    /// The path of the state file.
    path: PathBuf,
}

impl JsonFileStore {
    /// Creates a store in `dir`. The directory must exist.
    pub(crate) fn new(dir: &Path) -> Self {
        Self {
            path: dir.join(FILE_NAME),
        }
    }
}

impl NewsStore for JsonFileStore {
    /// Reads and parses the state file.
    fn load(&self) -> Result<Option<NewsDocument>, StoreError> {
        persist::read_json(&self.path)
    }

    /// Replaces the state file atomically.
    fn save(&self, document: &NewsDocument) -> Result<(), StoreError> {
        persist::write_json(&self.path, document)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::sanitize::SanitizedHtml;
    use std::assert_matches;
    use tempfile::TempDir;

    /// A fresh state directory has no news yet.
    #[test]
    fn load_returns_none_without_file() {
        let dir = TempDir::new().expect("temp dir should be created");

        assert_matches!(JsonFileStore::new(dir.path()).load(), Ok(None));
    }

    /// A saved revision is loaded again, also by a new instance after a
    /// restart.
    #[test]
    fn saved_document_survives_restart() {
        let dir = TempDir::new().expect("temp dir should be created");
        let seed = NewsDocument::seed();
        let next = seed.revise(
            SanitizedHtml::sanitize("<p>neu</p>"),
            "dirk",
            seed.modified_at,
        );

        JsonFileStore::new(dir.path())
            .save(&next)
            .expect("save should succeed");

        let loaded = JsonFileStore::new(dir.path())
            .load()
            .expect("the news should be readable")
            .expect("the news should exist");
        assert_eq!(loaded, next);
        assert!(dir.path().join(FILE_NAME).exists());
    }
}
