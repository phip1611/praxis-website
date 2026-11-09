// SPDX-License-Identifier: MIT OR Apache-2.0

//! A news store in memory, which can simulate failures.

use crate::content::news::NewsDocument;
use crate::content::store::NewsStore;
use crate::persist::StoreError;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Stores the news in memory.
#[derive(Debug, Default)]
pub(crate) struct MemoryStore {
    /// The saved document.
    document: Mutex<Option<NewsDocument>>,
    /// Whether saves fail.
    fail_saves: AtomicBool,
}

impl MemoryStore {
    /// Makes all following saves fail, like a full disk.
    pub(crate) fn fail_saves(&self) {
        self.fail_saves.store(true, Ordering::SeqCst);
    }
}

impl NewsStore for MemoryStore {
    /// Returns the saved document.
    fn load(&self) -> Result<Option<NewsDocument>, StoreError> {
        Ok(self.document.lock().unwrap().clone())
    }

    /// Saves the document, or fails if requested.
    fn save(&self, document: &NewsDocument) -> Result<(), StoreError> {
        if self.fail_saves.load(Ordering::SeqCst) {
            return Err(StoreError::Io {
                operation: "can't write",
                path: PathBuf::from("memory"),
                source: io::Error::other("simulated failure"),
            });
        }
        *self.document.lock().unwrap() = Some(document.clone());
        Ok(())
    }
}
