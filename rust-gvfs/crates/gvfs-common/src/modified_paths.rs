//! Modified paths database — tracked at `.gvfs/databases/ModifiedPaths.dat`.
//!
//! File format: line-oriented, `\r\n` newlines.
//! `A path/to/file.txt` — add entry
//! `D path/to/removed.txt` — remove entry
//! Folders end with `/` (git path separator).

use std::collections::HashSet;
use std::path::PathBuf;

use parking_lot::RwLock;
use tracing::debug;

/// The modified paths database.
pub struct ModifiedPaths {
    path: PathBuf,
    entries: RwLock<HashSet<String>>,
}

impl ModifiedPaths {
    /// Load or create the modified paths file.
    pub fn load_or_create(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = path.into();
        let entries = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            parse_modified_paths(&content)
        } else {
            let mut set = HashSet::new();
            // .gitattributes is always in the default set
            set.insert(".gitattributes".to_string());
            set
        };
        debug!("Loaded {} modified paths from {:?}", entries.len(), path);
        Ok(Self {
            path,
            entries: RwLock::new(entries),
        })
    }

    /// Add a file path to the modified set.
    pub fn add_file(&self, path: &str) -> bool {
        let normalized = normalize_path(path, false);
        // Check if a parent folder is already tracked
        if self.contains_parent_folder(&normalized) {
            return false;
        }
        let mut entries = self.entries.write();
        let inserted = entries.insert(normalized.clone());
        if inserted {
            // Append to file
            let _ = self.append_entry("A", &normalized);
        }
        inserted
    }

    /// Add a folder path to the modified set.
    pub fn add_folder(&self, path: &str) -> bool {
        let normalized = normalize_path(path, true);
        if self.contains_parent_folder(&normalized) {
            return false;
        }
        let mut entries = self.entries.write();
        let inserted = entries.insert(normalized.clone());
        if inserted {
            let _ = self.append_entry("A", &normalized);
        }
        inserted
    }

    /// Remove a path from the modified set.
    pub fn remove(&self, path: &str, is_folder: bool) -> bool {
        let normalized = normalize_path(path, is_folder);
        let mut entries = self.entries.write();
        let removed = entries.remove(&normalized);
        if removed {
            let _ = self.append_entry("D", &normalized);
        }
        removed
    }

    /// Check if a path is in the modified set.
    pub fn contains(&self, path: &str, is_folder: bool) -> bool {
        let normalized = normalize_path(path, is_folder);
        let entries = self.entries.read();
        entries.contains(&normalized)
    }

    /// Check if a parent folder of the given path is tracked.
    pub fn contains_parent_folder(&self, path: &str) -> bool {
        let entries = self.entries.read();
        let parts: Vec<&str> = path.split('/').collect();
        for i in 1..parts.len() {
            let parent = parts[..i].join("/") + "/";
            if entries.contains(&parent) {
                return true;
            }
        }
        false
    }

    /// Get all modified paths.
    pub fn get_all(&self) -> Vec<String> {
        let entries = self.entries.read();
        let mut paths: Vec<String> = entries.iter().cloned().collect();
        paths.sort();
        paths
    }

    /// Rewrite the entire file (compaction).
    pub fn flush(&self) -> anyhow::Result<()> {
        let entries = self.entries.read();
        let mut lines = Vec::new();
        let mut sorted: Vec<&String> = entries.iter().collect();
        sorted.sort();
        for entry in sorted {
            lines.push(format!("A {}", entry));
        }
        let content = lines.join("\r\n") + "\r\n";
        std::fs::write(&self.path, content)?;
        Ok(())
    }

    fn append_entry(&self, op: &str, path: &str) -> anyhow::Result<()> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{} {}\r", op, path)?;
        Ok(())
    }
}

/// Parse the ModifiedPaths.dat file content.
fn parse_modified_paths(content: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    for line in content.lines() {
        let line = line.trim_end_matches('\r').trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("A ") {
            set.insert(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("D ") {
            set.remove(rest);
        }
    }
    set
}

/// Normalize a path for the modified paths database.
fn normalize_path(path: &str, is_folder: bool) -> String {
    let mut normalized = path.replace('\\', "/");
    normalized = normalized.trim_start_matches('/').to_string();
    normalized = normalized.trim_end_matches('/').to_string();
    if is_folder && !normalized.ends_with('/') {
        normalized.push('/');
    }
    normalized
}
