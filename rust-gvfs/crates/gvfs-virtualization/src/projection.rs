//! Git index projection — builds a tree from the git index for ProjFS enumeration.

use std::collections::BTreeMap;
use std::path::Path;

use gvfs_common::git::{self, IndexEntry};
use parking_lot::RwLock;
use tracing::info;

/// A node in the projected file system tree.
#[derive(Debug, Clone)]
pub enum ProjectedEntry {
    File {
        name: String,
        sha: String,
        size: u32,
        mode: u32,
    },
    Directory {
        name: String,
        children: BTreeMap<String, ProjectedEntry>,
    },
}

impl ProjectedEntry {
    pub fn name(&self) -> &str {
        match self {
            ProjectedEntry::File { name, .. } => name,
            ProjectedEntry::Directory { name, .. } => name,
        }
    }

    pub fn is_directory(&self) -> bool {
        matches!(self, ProjectedEntry::Directory { .. })
    }
}

/// The projection of the git index as a file system tree.
pub struct GitIndexProjection {
    root: RwLock<BTreeMap<String, ProjectedEntry>>,
    entry_count: RwLock<usize>,
}

impl GitIndexProjection {
    pub fn new() -> Self {
        Self {
            root: RwLock::new(BTreeMap::new()),
            entry_count: RwLock::new(0),
        }
    }

    /// Load the projection from the git index file.
    pub fn load_from_index(&self, git_dir: &Path) -> anyhow::Result<()> {
        let entries = git::parse_git_index(git_dir)?;
        info!("Loaded {} entries from git index", entries.len());

        let mut root = BTreeMap::new();

        for entry in &entries {
            insert_entry(&mut root, &entry.path, entry);
        }

        *self.root.write() = root;
        *self.entry_count.write() = entries.len();

        info!("Projection built with {} entries", entries.len());
        Ok(())
    }

    /// Get entries at a given directory path.
    /// Returns (subdirectories, files) sorted by ProjFS name order.
    pub fn get_entries(&self, dir_path: &str) -> Vec<ProjectedEntry> {
        let root = self.root.read();
        let normalized = normalize_dir_path(dir_path);

        if normalized.is_empty() {
            // Root directory
            return root.values().cloned().collect();
        }

        // Walk to the target directory.
        let parts: Vec<&str> = normalized.split('/').collect();
        let mut current = &*root;

        for part in &parts {
            match current.get(*part) {
                Some(ProjectedEntry::Directory { children, .. }) => {
                    current = children;
                }
                _ => return Vec::new(),
            }
        }

        current.values().cloned().collect()
    }

    /// Look up a single entry by relative path.
    pub fn get_entry(&self, relative_path: &str) -> Option<ProjectedEntry> {
        let root = self.root.read();
        let normalized = relative_path.replace('\\', "/");
        let parts: Vec<&str> = normalized.split('/').collect();

        if parts.is_empty() {
            return None;
        }

        let mut current = &*root;
        for (i, part) in parts.iter().enumerate() {
            match current.get(*part) {
                Some(entry) => {
                    if i == parts.len() - 1 {
                        return Some(entry.clone());
                    }
                    match entry {
                        ProjectedEntry::Directory { children, .. } => {
                            current = children;
                        }
                        _ => return None,
                    }
                }
                None => return None,
            }
        }
        None
    }

    /// Check if a path exists in the projection.
    pub fn path_exists(&self, relative_path: &str) -> bool {
        self.get_entry(relative_path).is_some()
    }

    /// Get entry count.
    pub fn entry_count(&self) -> usize {
        *self.entry_count.read()
    }
}

impl Default for GitIndexProjection {
    fn default() -> Self {
        Self::new()
    }
}

/// Insert a git index entry into the tree.
fn insert_entry(
    root: &mut BTreeMap<String, ProjectedEntry>,
    path: &str,
    entry: &IndexEntry,
) {
    let parts: Vec<&str> = path.split('/').collect();

    if parts.len() == 1 {
        // File at this level.
        let name = parts[0].to_string();
        let key = name.to_lowercase();
        root.insert(
            key,
            ProjectedEntry::File {
                name,
                sha: entry.sha.clone(),
                size: entry.file_size,
                mode: entry.mode,
            },
        );
        return;
    }

    // Navigate or create intermediate directories.
    let dir_name = parts[0].to_string();
    let key = dir_name.to_lowercase();
    let rest = parts[1..].join("/");

    let dir_entry = root
        .entry(key)
        .or_insert_with(|| ProjectedEntry::Directory {
            name: dir_name,
            children: BTreeMap::new(),
        });

    if let ProjectedEntry::Directory { children, .. } = dir_entry {
        insert_entry(children, &rest, entry);
    }
}

fn normalize_dir_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_string()
}
