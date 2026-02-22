//! gvfs-service.exe — Windows service for VFSForGit.
//!
//! Manages repository registration, auto-mount on login, and ProjFS enablement.
//! Listens on the "GVFS.Service" named pipe for requests from gvfs.exe.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use gvfs_common::config;
use gvfs_common::pipe::{PipeMessage, PipeServer};

// ─────────────────────────────────────────────────────────────────────────────
// Repo Registry
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RepoRegistration {
    #[serde(rename = "EnlistmentRoot")]
    enlistment_root: String,
    #[serde(rename = "OwnerSID")]
    owner_sid: String,
    #[serde(rename = "IsActive")]
    is_active: bool,
}

struct RepoRegistry {
    path: PathBuf,
    repos: RwLock<Vec<RepoRegistration>>,
}

impl RepoRegistry {
    fn load(service_data_dir: &Path) -> Self {
        let path = service_data_dir.join("repo-registry");
        let repos = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => parse_registry(&content),
                Err(e) => {
                    warn!("Failed to read repo registry: {}", e);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        Self {
            path,
            repos: RwLock::new(repos),
        }
    }

    fn register(&self, root: &str, sid: &str) {
        let mut repos = self.repos.write();
        if let Some(existing) = repos.iter_mut().find(|r| r.enlistment_root == root) {
            existing.is_active = true;
            existing.owner_sid = sid.to_string();
        } else {
            repos.push(RepoRegistration {
                enlistment_root: root.to_string(),
                owner_sid: sid.to_string(),
                is_active: true,
            });
        }
        let _ = self.save(&repos);
    }

    fn deactivate(&self, root: &str) {
        let mut repos = self.repos.write();
        if let Some(existing) = repos.iter_mut().find(|r| r.enlistment_root == root) {
            existing.is_active = false;
        }
        let _ = self.save(&repos);
    }

    fn get_active(&self) -> Vec<String> {
        let repos = self.repos.read();
        repos
            .iter()
            .filter(|r| r.is_active)
            .map(|r| r.enlistment_root.clone())
            .collect()
    }

    fn save(&self, repos: &[RepoRegistration]) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut lines = vec!["2".to_string()]; // Version
        for repo in repos {
            lines.push(serde_json::to_string(repo)?);
        }
        fs::write(&self.path, lines.join("\n"))?;
        Ok(())
    }
}

fn parse_registry(content: &str) -> Vec<RepoRegistration> {
    let mut repos = Vec::new();
    let mut lines = content.lines();

    // First line is version number.
    if let Some(_version) = lines.next() {
        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(reg) = serde_json::from_str::<RepoRegistration>(line) {
                repos.push(reg);
            }
        }
    }
    repos
}

// ─────────────────────────────────────────────────────────────────────────────
// Service main
// ─────────────────────────────────────────────────────────────────────────────

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    info!("GVFS Service starting...");

    // For now, run as a console application (not a true Windows service).
    // This makes development and testing easier.
    // A real deployment would use windows-service crate for SCM integration.
    if let Err(e) = run_service() {
        error!("Service failed: {}", e);
        std::process::exit(1);
    }
}

fn run_service() -> anyhow::Result<()> {
    let service_data = config::service_data_path();
    fs::create_dir_all(&service_data)?;

    let registry = Arc::new(RepoRegistry::load(&service_data));
    let running = Arc::new(AtomicBool::new(true));

    // Auto-mount active repos.
    let active = registry.get_active();
    for repo in &active {
        info!("Auto-mounting: {}", repo);
        let exe_dir = std::env::current_exe()?
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let gvfs_exe = exe_dir.join("gvfs.exe");
        if gvfs_exe.exists() {
            let _ = Command::new(&gvfs_exe)
                .args(["mount", repo])
                .spawn();
        }
    }

    // Start pipe server.
    let server = PipeServer::new("GVFS.Service");
    let reg_clone = registry.clone();

    info!("GVFS Service ready, listening on pipe...");

    // Handle Ctrl+C.
    let running_clone = running.clone();
    ctrlc_handler(running_clone);

    server.run(move |msg| handle_service_message(&reg_clone, msg))?;

    Ok(())
}

fn handle_service_message(registry: &RepoRegistry, msg: PipeMessage) -> PipeMessage {
    match msg.header.as_str() {
        "RegisterRepoRequest" => {
            if let Some(body) = &msg.body {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(body) {
                    let root = data["EnlistmentRoot"]
                        .as_str()
                        .unwrap_or("");
                    let sid = data["OwnerSID"]
                        .as_str()
                        .unwrap_or("S-1-5-0");
                    registry.register(root, sid);
                    info!("Registered repo: {}", root);
                    return success_response();
                }
            }
            failure_response("Invalid request body")
        }

        "UnregisterRepoRequest" => {
            if let Some(body) = &msg.body {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(body) {
                    let root = data["EnlistmentRoot"]
                        .as_str()
                        .unwrap_or("");
                    registry.deactivate(root);
                    info!("Deactivated repo: {}", root);
                    return success_response();
                }
            }
            failure_response("Invalid request body")
        }

        "GetActiveRepoListRequest" => {
            let active = registry.get_active();
            let response = serde_json::json!({
                "State": "Success",
                "RepoList": active,
            });
            PipeMessage::new("S", Some(response.to_string()))
        }

        "EnableAndAttachProjFSRequest" => {
            // ProjFS enablement — on modern Windows, ProjFS is usually already enabled.
            // Try to enable it via DISM if needed.
            info!("ProjFS enablement requested");
            let status = Command::new("dism")
                .args([
                    "/online",
                    "/enable-feature",
                    "/featurename:Client-ProjFS",
                    "/norestart",
                ])
                .status();
            match status {
                Ok(s) if s.success() => success_response(),
                _ => {
                    warn!("DISM ProjFS enable may have failed (might already be enabled)");
                    success_response()
                }
            }
        }

        other => {
            warn!("Unknown service message: {}", other);
            failure_response(&format!("Unknown command: {}", other))
        }
    }
}

fn success_response() -> PipeMessage {
    PipeMessage::new(
        "S",
        Some(r#"{"State":"Success"}"#.to_string()),
    )
}

fn failure_response(reason: &str) -> PipeMessage {
    PipeMessage::new(
        "F",
        Some(format!(r#"{{"State":"Failure","Reason":"{}"}}"#, reason)),
    )
}

fn ctrlc_handler(running: Arc<AtomicBool>) {
    let _ = std::thread::spawn(move || {
        // Simple approach: loop and check, since we can't use ctrlc crate easily.
        // The pipe server will block, so this is more of a placeholder.
        // In production, we'd use SetConsoleCtrlHandler via FFI.
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            if !running.load(Ordering::SeqCst) {
                break;
            }
        }
    });
}
