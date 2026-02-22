//! GVFS configuration — local and server config management.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Local GVFS config (stored at `%LocalAppData%\GVFS\gvfs.config`).
#[derive(Debug, Clone, Default)]
pub struct LocalConfig {
    pub path: PathBuf,
    pub entries: HashMap<String, String>,
}

impl LocalConfig {
    /// Load or create the local config file.
    pub fn load_or_create() -> anyhow::Result<Self> {
        let config_dir = dirs_path()?;
        std::fs::create_dir_all(&config_dir)?;
        let path = config_dir.join("gvfs.config");

        let entries = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            parse_file_based_dict(&content)
        } else {
            HashMap::new()
        };

        Ok(Self { path, entries })
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(|s| s.as_str())
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.entries.insert(key.to_string(), value.to_string());
    }
}

/// Server GVFS config (fetched from `/gvfs/config`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(rename = "AllowedGVFSClientVersions")]
    pub allowed_versions: Option<Vec<VersionRange>>,
    #[serde(rename = "CacheServers")]
    pub cache_servers: Option<Vec<CacheServerInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionRange {
    #[serde(rename = "Min")]
    pub min: Option<String>,
    #[serde(rename = "Max")]
    pub max: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheServerInfo {
    #[serde(rename = "Url")]
    pub url: String,
    #[serde(rename = "Name")]
    pub name: Option<String>,
    #[serde(rename = "GlobalDefault")]
    pub global_default: Option<bool>,
}

impl CacheServerInfo {
    /// Get the objects endpoint URL for this cache server.
    pub fn objects_url(&self) -> String {
        format!("{}/gvfs/objects", self.url.trim_end_matches('/'))
    }

    /// Get the prefetch endpoint URL.
    pub fn prefetch_url(&self) -> String {
        format!("{}/gvfs/prefetch", self.url.trim_end_matches('/'))
    }

    /// Get the sizes endpoint URL.
    pub fn sizes_url(&self) -> String {
        format!("{}/gvfs/sizes", self.url.trim_end_matches('/'))
    }
}

/// Parse a file-based dictionary (line-oriented `A key|value` / `D key` format).
pub fn parse_file_based_dict(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("A ") {
            if let Some(sep) = rest.find('|') {
                let key = &rest[..sep];
                let value = &rest[sep + 1..];
                map.insert(key.to_string(), value.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("D ") {
            map.remove(rest.trim());
        }
    }
    map
}

/// Serialize entries to the file-based dictionary format.
pub fn serialize_file_based_dict(entries: &HashMap<String, String>) -> String {
    let mut lines = Vec::new();
    for (key, value) in entries {
        lines.push(format!("A {}|{}", key, value));
    }
    lines.sort();
    lines.join("\r\n") + "\r\n"
}

/// Get the GVFS config directory path.
fn dirs_path() -> anyhow::Result<PathBuf> {
    let local_app_data = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into()))
                .join("AppData")
                .join("Local")
        });
    Ok(local_app_data.join("GVFS"))
}

/// Global GVFS config path.
pub fn global_config_path() -> PathBuf {
    dirs_path().unwrap_or_else(|_| PathBuf::from("C:\\ProgramData\\GVFS")).join("gvfs.config")
}

/// Service data location.
pub fn service_data_path() -> PathBuf {
    PathBuf::from(
        std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".into()),
    )
    .join("GVFS.Service")
}
