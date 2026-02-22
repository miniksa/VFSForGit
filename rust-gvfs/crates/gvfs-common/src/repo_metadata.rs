//! Repo metadata — key-value store at `.gvfs/databases/RepoMetadata.dat`.
//!
//! Backed by a file-based dictionary with `A key|value` and `D key` entries.

use std::collections::HashMap;
use std::path::PathBuf;

use tracing::debug;

use crate::config::{parse_file_based_dict, serialize_file_based_dict};

/// Keys used in RepoMetadata.
pub mod keys {
    pub const DISK_LAYOUT_VERSION: &str = "DiskLayoutVersion";
    pub const DISK_LAYOUT_MINOR_VERSION: &str = "DiskLayoutMinorVersion";
    pub const GIT_OBJECTS_ROOT: &str = "GitObjectsRoot";
    pub const LOCAL_CACHE_ROOT: &str = "LocalCacheRoot";
    pub const BLOB_SIZES_ROOT: &str = "BlobSizesRoot";
    pub const ENLISTMENT_ID: &str = "EnlistmentId";
    pub const PROJECTION_INVALID: &str = "ProjectionInvalid";
    pub const PLACEHOLDERS_NEED_UPDATE: &str = "PlaceholdersNeedUpdate";
}

/// Current disk layout version.
pub const CURRENT_DISK_LAYOUT_VERSION: &str = "20";
pub const CURRENT_DISK_LAYOUT_MINOR_VERSION: &str = "0";

/// Repository metadata stored as a file-based dictionary.
#[derive(Debug)]
pub struct RepoMetadata {
    path: PathBuf,
    entries: HashMap<String, String>,
}

impl RepoMetadata {
    /// Load an existing RepoMetadata file.
    pub fn load(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = path.into();
        let entries = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            parse_file_based_dict(&content)
        } else {
            HashMap::new()
        };
        debug!("Loaded RepoMetadata with {} entries from {:?}", entries.len(), path);
        Ok(Self { path, entries })
    }

    /// Create a new RepoMetadata file with default entries.
    pub fn create(
        path: impl Into<PathBuf>,
        enlistment_id: &str,
        git_objects_root: &str,
        local_cache_root: &str,
    ) -> anyhow::Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut entries = HashMap::new();
        entries.insert(
            keys::DISK_LAYOUT_VERSION.to_string(),
            CURRENT_DISK_LAYOUT_VERSION.to_string(),
        );
        entries.insert(
            keys::DISK_LAYOUT_MINOR_VERSION.to_string(),
            CURRENT_DISK_LAYOUT_MINOR_VERSION.to_string(),
        );
        entries.insert(keys::ENLISTMENT_ID.to_string(), enlistment_id.to_string());
        entries.insert(
            keys::GIT_OBJECTS_ROOT.to_string(),
            git_objects_root.to_string(),
        );
        entries.insert(
            keys::LOCAL_CACHE_ROOT.to_string(),
            local_cache_root.to_string(),
        );

        let meta = Self {
            path,
            entries,
        };
        meta.save()?;
        Ok(meta)
    }

    /// Get a metadata value.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(|s| s.as_str())
    }

    /// Set a metadata value.
    pub fn set(&mut self, key: &str, value: &str) {
        self.entries.insert(key.to_string(), value.to_string());
    }

    /// Save to disk.
    pub fn save(&self) -> anyhow::Result<()> {
        let content = serialize_file_based_dict(&self.entries);
        std::fs::write(&self.path, content)?;
        Ok(())
    }

    /// Get the git objects root path.
    pub fn git_objects_root(&self) -> Option<&str> {
        self.get(keys::GIT_OBJECTS_ROOT)
    }

    /// Get the local cache root path.
    pub fn local_cache_root(&self) -> Option<&str> {
        self.get(keys::LOCAL_CACHE_ROOT)
    }

    /// Get the enlistment ID.
    pub fn enlistment_id(&self) -> Option<&str> {
        self.get(keys::ENLISTMENT_ID)
    }
}
