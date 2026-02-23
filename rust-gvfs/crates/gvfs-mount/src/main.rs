//! gvfs-mount.exe — Mount daemon for VFSForGit.
//!
//! This binary is launched by `gvfs mount` as a background process. It:
//! 1. Loads the git index and builds a projection
//! 2. Starts the ProjFS virtualizer on the working directory
//! 3. Starts a named pipe server for IPC (status, unmount, DLO, lock, etc.)
//! 4. Waits until an unmount request is received

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

use gvfs_common::constants;
use gvfs_common::enlistment::GvfsEnlistment;
use gvfs_common::http::{GitAuth, GvfsClient};
use gvfs_common::lock::{GvfsLock, LockData};
use gvfs_common::pipe::{PipeMessage, PipeServer};
use gvfs_common::repo_metadata::RepoMetadata;

use gvfs_virtualization::virtualizer::GvfsVirtualizer;

/// Mount state shared between the pipe server and main thread.
struct MountState {
    enlistment: GvfsEnlistment,
    lock: GvfsLock,
    running: AtomicBool,
    status: parking_lot::RwLock<String>,
    virtualizer: parking_lot::RwLock<Option<Arc<GvfsVirtualizer>>>,
}

fn main() {
    // Initialize logging — log to file and stderr.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: gvfs-mount <enlistment-root>");
        std::process::exit(1);
    }

    let root = PathBuf::from(&args[1]);
    info!("Starting GVFS mount for {:?}", root);

    if let Err(e) = run_mount(&root) {
        error!("Mount failed: {}", e);
        std::process::exit(1);
    }
}

fn run_mount(root: &Path) -> anyhow::Result<()> {
    let enlistment = GvfsEnlistment::new(root);

    if !enlistment.is_valid() {
        anyhow::bail!("{:?} is not a valid GVFS enlistment", root);
    }

    // Load repo metadata.
    let metadata = RepoMetadata::load(enlistment.repo_metadata_path())?;
    info!(
        "Enlistment ID: {}",
        metadata.enlistment_id().unwrap_or("unknown")
    );

    // Resolve remote URL and pre-acquire credentials for the HTTP client.
    // We acquire credentials once here at mount startup so that file hydration
    // never triggers interactive credential prompts.
    let mut enl = enlistment.clone();
    enl.resolve_remote_url()?;

    let http_client = enl.remote_url.as_ref().map(|url| {
        // Try to acquire CACHED credentials (GCM_INTERACTIVE=never prevents prompts).
        // Clone already authenticated interactively, so GCM should have the token cached.
        let auth = GitAuth::from_repo_credential_manager(&enlistment.working_dir(), url)
            .ok();
        if auth.is_some() {
            info!("Credentials acquired for HTTP client");
        } else {
            warn!("No cached credentials available — file hydration will use git cat-file");
        }
        Arc::new(GvfsClient::new(url, auth))
    });

    // Create the virtualizer.
    let virtualizer = Arc::new(GvfsVirtualizer::new(enlistment.clone(), http_client)?);

    // Load the git index projection.
    virtualizer.load_projection()?;

    // Set up the ProjFS virtualization root on the working directory (src/).
    let working_dir = enlistment.working_dir();

    // Generate a stable virtualization instance GUID from the enlistment ID.
    let enlistment_id_str = metadata.enlistment_id().unwrap_or("00000000-0000-0000-0000-000000000000");
    let instance_uuid = uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_URL,
        enlistment_id_str.as_bytes(),
    );
    let instance_id = windows_core::GUID::from_u128(instance_uuid.as_u128());

    // Mark the working directory as a ProjFS virtualization root.
    // This sets a reparse point that ProjFS recognizes.
    // It may fail if already marked (e.g., re-mount) — that's OK.
    match projfs::mark_directory_as_placeholder(&working_dir, "", &instance_id) {
        Ok(()) => info!("Marked {:?} as ProjFS virtualization root", working_dir),
        Err(e) => info!("mark_directory_as_placeholder result: {:?} (may already be marked)", e),
    }

    // Start ProjFS.
    let virt_instance = virtualizer.clone().start(&working_dir)?;
    info!("ProjFS virtualizer running");

    // Set up shared state.
    let state = Arc::new(MountState {
        enlistment: enlistment.clone(),
        lock: GvfsLock::new(),
        running: AtomicBool::new(true),
        status: parking_lot::RwLock::new("Ready".to_string()),
        virtualizer: parking_lot::RwLock::new(Some(virtualizer.clone())),
    });

    // Start the named pipe server in a background thread.
    let pipe_state = state.clone();
    let pipe_name = enlistment.pipe_name();
    let _pipe_thread = thread::spawn(move || {
        let server = PipeServer::new(&pipe_name);
        info!("Named pipe server started: {}", pipe_name);

        if let Err(e) = server.run(move |msg| handle_pipe_message(&pipe_state, msg)) {
            error!("Pipe server error: {}", e);
        }
    });

    info!("Mount ready — waiting for unmount signal...");

    // Main thread waits for the running flag to be cleared.
    while state.running.load(Ordering::SeqCst) {
        thread::sleep(std::time::Duration::from_millis(500));
    }

    info!("Unmount requested — shutting down...");

    // Stop ProjFS.
    drop(virt_instance);

    // Flush modified paths.
    if let Some(v) = state.virtualizer.read().as_ref() {
        let _ = v.modified_paths().flush();
    }

    info!("Mount daemon shutdown complete.");
    Ok(())
}

/// Handle an incoming named pipe message.
fn handle_pipe_message(state: &MountState, msg: PipeMessage) -> PipeMessage {
    match msg.header.as_str() {
        "GetStatus" => {
            let status = state.status.read().clone();
            // The CLI mount command polls for "Ready" in the response text.
            let response = format!(
                "Mount status: {}\nEnlistment root: {}\nVersion: {}\nLocked: {}\nLocked command: {}",
                status,
                state.enlistment.root.to_string_lossy(),
                constants::GVFS_VERSION,
                state.lock.is_locked(),
                state.lock.get_locked_command().unwrap_or_default(),
            );
            PipeMessage::new("S", Some(response))
        }

        "Unmount" => {
            info!("Unmount requested via pipe");
            state.running.store(false, Ordering::SeqCst);
            PipeMessage::new("S", Some("Unmount requested".to_string()))
        }

        "DLO" => {
            // Download Object: body = 40-char SHA.
            let sha = msg.body.as_deref().unwrap_or("");
            if sha.len() != 40 {
                return PipeMessage::new("F", None);
            }

            let virt_guard = state.virtualizer.read();
            if let Some(_virt) = virt_guard.as_ref() {
                // Try to download the object.
                let working_dir = state.enlistment.working_dir();
                match std::process::Command::new("git")
                    .args(["cat-file", "-e", sha])
                    .current_dir(&working_dir)
                    .status()
                {
                    Ok(status) if status.success() => {
                        PipeMessage::new("S", None)
                    }
                    _ => {
                        // Object not local — try HTTP download.
                        debug!("Object {} not found locally, attempting download", sha);
                        // We can't easily download here without async — for now,
                        // report success if the object exists, failure otherwise.
                        PipeMessage::new("F", None)
                    }
                }
            } else {
                PipeMessage::new("F", None)
            }
        }

        "AcquireLock" => {
            let body = msg.body.as_deref().unwrap_or("");
            match LockData::from_wire(body) {
                Some(lock_data) => {
                    let (acquired, existing) =
                        state.lock.try_acquire_for_external(lock_data);
                    if acquired {
                        PipeMessage::new("LockAcquired", None)
                    } else if let Some(holder) = existing {
                        PipeMessage::new(
                            "LockDeniedGit",
                            Some(holder.parsed_command.clone()),
                        )
                    } else if state.lock.is_gvfs_locked() {
                        PipeMessage::new("LockDeniedGVFS", None)
                    } else {
                        PipeMessage::new("LockDeniedGit", None)
                    }
                }
                None => PipeMessage::new("F", Some("Invalid lock data".to_string())),
            }
        }

        "ReleaseLock" => {
            let body = msg.body.as_deref().unwrap_or("");
            if let Some(lock_data) = LockData::from_wire(body) {
                state.lock.release_for_external(lock_data.pid);
                // Return release data (no failed placeholders in Rust implementation).
                PipeMessage::new("S", Some("0<0<".to_string()))
            } else {
                PipeMessage::new("F", None)
            }
        }

        "MPL" => {
            // Modified Paths List.
            let virt_guard = state.virtualizer.read();
            if let Some(virt) = virt_guard.as_ref() {
                let paths = virt.modified_paths().get_all();
                let response = paths.join("\0");
                PipeMessage::new("S", Some(response))
            } else {
                PipeMessage::new("F", None)
            }
        }

        "PICN" => {
            // Post Index Changed Notification — reload projection.
            info!("Post-index-change notification received");
            let virt_guard = state.virtualizer.read();
            if let Some(virt) = virt_guard.as_ref() {
                if let Err(e) = virt.load_projection() {
                    error!("Failed to reload projection: {}", e);
                    return PipeMessage::new("F", None);
                }
            }
            PipeMessage::new("S", None)
        }

        other => {
            warn!("Unknown pipe message header: {}", other);
            PipeMessage::new("F", Some(format!("Unknown command: {}", other)))
        }
    }
}
