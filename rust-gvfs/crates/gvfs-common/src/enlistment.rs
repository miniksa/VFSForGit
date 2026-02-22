//! Enlistment abstraction — path management for GVFS repos.

use std::path::{Path, PathBuf};

use crate::constants::*;

/// Represents a GVFS enlistment on disk.
#[derive(Debug, Clone)]
pub struct GvfsEnlistment {
    /// The root directory of the enlistment (e.g., `C:\Repos\myrepo`).
    pub root: PathBuf,
    /// The remote URL for the git repository.
    pub remote_url: Option<String>,
    /// The branch to track.
    pub branch: Option<String>,
}

impl GvfsEnlistment {
    /// Create a new enlistment object from a root path.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            remote_url: None,
            branch: None,
        }
    }

    pub fn with_remote(mut self, url: impl Into<String>) -> Self {
        self.remote_url = Some(url.into());
        self
    }

    pub fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    /// The working directory (`<root>/src`).
    pub fn working_dir(&self) -> PathBuf {
        self.root.join(WORKING_DIR)
    }

    /// The `.git` directory (`<root>/src/.git`).
    pub fn git_dir(&self) -> PathBuf {
        self.working_dir().join(".git")
    }

    /// The `.gvfs` directory (`<root>/.gvfs`).
    pub fn dot_gvfs_dir(&self) -> PathBuf {
        self.root.join(DOT_GVFS)
    }

    /// The databases directory (`<root>/.gvfs/databases`).
    pub fn databases_dir(&self) -> PathBuf {
        self.dot_gvfs_dir().join(DATABASES_DIR)
    }

    /// Path to RepoMetadata.dat.
    pub fn repo_metadata_path(&self) -> PathBuf {
        self.databases_dir().join(REPO_METADATA_FILE)
    }

    /// Path to ModifiedPaths.dat.
    pub fn modified_paths_path(&self) -> PathBuf {
        self.databases_dir().join(MODIFIED_PATHS_FILE)
    }

    /// Path to PlaceholderList.dat.
    pub fn placeholder_list_path(&self) -> PathBuf {
        self.databases_dir().join(PLACEHOLDER_LIST_FILE)
    }

    /// Path to VFSForGit.sqlite.
    pub fn sqlite_path(&self) -> PathBuf {
        self.databases_dir().join(SQLITE_DB_FILE)
    }

    /// The logs directory inside .gvfs.
    pub fn logs_dir(&self) -> PathBuf {
        self.dot_gvfs_dir().join("logs")
    }

    /// The named pipe name for this enlistment.
    pub fn pipe_name(&self) -> String {
        crate::pipe::get_pipe_name(&self.root)
    }

    /// The local cache root (shared git objects).
    pub fn local_cache_root(&self) -> PathBuf {
        self.root.join(".gvfs").join("gitObjectCache")
    }

    /// The git objects root (inside local cache or working dir).
    pub fn git_objects_root(&self) -> PathBuf {
        self.git_dir().join("objects")
    }

    /// The git pack directory.
    pub fn git_pack_dir(&self) -> PathBuf {
        self.git_objects_root().join("pack")
    }

    /// The blob sizes root (cached file sizes).
    pub fn blob_sizes_root(&self) -> PathBuf {
        self.root.join(".gvfs").join("BlobSizes")
    }

    /// Create all required directories for a new enlistment.
    pub fn create_directories(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(self.dot_gvfs_dir())?;
        std::fs::create_dir_all(self.databases_dir())?;
        std::fs::create_dir_all(self.logs_dir())?;
        std::fs::create_dir_all(self.local_cache_root())?;
        std::fs::create_dir_all(self.blob_sizes_root())?;
        Ok(())
    }

    /// Try to find an enlistment root by walking up from a given path.
    /// Looks for the `.gvfs` directory.
    pub fn find_root(start: &Path) -> Option<PathBuf> {
        let mut current = start.to_path_buf();
        loop {
            if current.join(DOT_GVFS).is_dir() {
                return Some(current);
            }
            if current.join(WORKING_DIR).join(".git").is_dir()
                && current.join(DOT_GVFS).exists()
            {
                return Some(current);
            }
            if !current.pop() {
                return None;
            }
        }
    }

    /// Check if this looks like a valid GVFS enlistment.
    pub fn is_valid(&self) -> bool {
        self.dot_gvfs_dir().is_dir() && self.working_dir().is_dir()
    }

    /// Get the remote URL from git config if not already set.
    pub fn resolve_remote_url(&mut self) -> anyhow::Result<()> {
        if self.remote_url.is_some() {
            return Ok(());
        }
        let output = std::process::Command::new("git")
            .args(["config", "--get", "remote.origin.url"])
            .current_dir(self.working_dir())
            .output()?;
        if output.status.success() {
            self.remote_url = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
        Ok(())
    }
}
