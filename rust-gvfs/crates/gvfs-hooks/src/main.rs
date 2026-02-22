//! gvfs-hooks.exe — Git hook handler for VFSForGit.
//!
//! Manages lock acquisition/release around git commands.
//! Called as pre-command and post-command hooks by git.
//!
//! Usage:
//!   gvfs-hooks pre-command <git-verb> [args...]
//!   gvfs-hooks post-command <git-verb> [args...]

use std::path::{Path, PathBuf};
use std::process;
use std::thread;
use std::time::Duration;

use tracing::{debug, error, warn};
use tracing_subscriber::EnvFilter;

use gvfs_common::enlistment::GvfsEnlistment;
use gvfs_common::lock::LockData;
use gvfs_common::pipe::{PipeClient, PipeMessage};

/// Commands that DON'T require the GVFS lock.
const PASSTHROUGH_COMMANDS: &[&str] = &[
    "blame", "branch", "cat-file", "config", "diff", "fetch", "for-each-ref",
    "help", "log", "ls-files", "push", "remote", "rev-list", "rev-parse",
    "show", "tag", "version", "submodule",
];

/// Commands that are blocked entirely.
const BLOCKED_COMMANDS: &[&str] = &["gui"];

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_target(false)
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        // Not enough args — silently exit (might be called incorrectly).
        process::exit(0);
    }

    let hook_type = &args[1]; // "pre-command" or "post-command"
    let git_verb = &args[2];

    // Find the enlistment root.
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let root = match GvfsEnlistment::find_root(&cwd) {
        Some(r) => r,
        None => {
            // Not in a GVFS enlistment — pass through.
            process::exit(0);
        }
    };

    match hook_type.as_str() {
        "pre-command" => {
            if let Err(e) = pre_command(&root, git_verb, &args[3..]) {
                error!("pre-command hook failed: {}", e);
                process::exit(1);
            }
        }
        "post-command" => {
            if let Err(e) = post_command(&root, git_verb, &args[3..]) {
                error!("post-command hook failed: {}", e);
                // Don't fail on post-command errors.
            }
        }
        _ => {
            process::exit(0);
        }
    }
}

fn pre_command(root: &Path, git_verb: &str, _args: &[String]) -> anyhow::Result<()> {
    // Check for blocked commands.
    if BLOCKED_COMMANDS.contains(&git_verb) {
        eprintln!("GVFS: '{}' is not supported in a GVFS enlistment.", git_verb);
        process::exit(1);
    }

    // Check for passthrough commands (no lock needed).
    if is_passthrough(git_verb) {
        return Ok(());
    }

    // Special handling for reset --soft.
    if git_verb == "reset" {
        let full_cmd: String = std::env::args().skip(2).collect::<Vec<_>>().join(" ");
        if full_cmd.contains("--soft") {
            return Ok(());
        }
    }

    // Acquire the GVFS lock.
    let pid = process::id();
    let is_elevated = is_elevated();
    let full_command = format!("git {}", std::env::args().skip(2).collect::<Vec<_>>().join(" "));
    let session_id = std::env::var("SESSIONNAME").unwrap_or_else(|_| "Console".to_string());

    let lock_data = LockData {
        pid,
        is_elevated,
        check_availability_only: false,
        parsed_command: full_command.clone(),
        session_id,
    };

    // Check the mount is running first.
    if !gvfs_common::pipe::is_mounted(root) {
        debug!("Mount not running, allowing git command without lock");
        return Ok(());
    }

    let msg = PipeMessage::new("AcquireLock", Some(lock_data.to_wire()));

    // Retry loop with spinner.
    let max_retries = 120; // 30 seconds at 250ms intervals
    for attempt in 0..max_retries {
        let current_client = match PipeClient::connect_to_mount(root) {
            Ok(c) => c,
            Err(_) => {
                if attempt == 0 {
                    debug!("Mount not running, allowing git command without lock");
                    return Ok(());
                }
                return Ok(());
            }
        };

        let response = current_client.send_receive(&msg)?;

        match response.header.as_str() {
            "LockAcquired" | "LockAvailable" => {
                debug!("Lock acquired for: {}", full_command);
                return Ok(());
            }
            "LockDeniedGit" => {
                let holder = response.body.as_deref().unwrap_or("unknown");
                if attempt == 0 {
                    eprint!(
                        "GVFS: waiting for '{}' to finish...",
                        holder
                    );
                } else if attempt % 4 == 0 {
                    eprint!(".");
                }
                thread::sleep(Duration::from_millis(250));
                continue;
            }
            "LockDeniedGVFS" => {
                if attempt == 0 {
                    eprint!("GVFS: waiting for GVFS operation to complete...");
                }
                thread::sleep(Duration::from_millis(250));
                continue;
            }
            "MountNotReady" => {
                debug!("Mount not ready yet");
                thread::sleep(Duration::from_millis(250));
                continue;
            }
            "UnmountInProgress" => {
                eprintln!("GVFS: unmount in progress.");
                process::exit(1);
            }
            other => {
                warn!("Unexpected lock response: {}", other);
                return Ok(());
            }
        }
    }

    eprintln!("\nGVFS: lock acquisition timed out.");
    process::exit(1);
}

fn post_command(root: &Path, git_verb: &str, _args: &[String]) -> anyhow::Result<()> {
    // If this was a passthrough command, nothing to release.
    if is_passthrough(git_verb) {
        return Ok(());
    }

    // Release the lock.
    let pid = process::id();
    let lock_data = LockData {
        pid,
        is_elevated: false,
        check_availability_only: false,
        parsed_command: String::new(),
        session_id: String::new(),
    };

    let client = match PipeClient::connect_to_mount(root) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    let msg = PipeMessage::new("ReleaseLock", Some(lock_data.to_wire()));
    let response = client.send_receive(&msg)?;

    // Check for failed placeholder updates.
    if let Some(body) = &response.body {
        // Format: FailedUpdateCount<FailedDeleteCount<FailedUpdatePaths|separated<FailedDeletePaths
        let sections: Vec<&str> = body.split('<').collect();
        if sections.len() >= 2 {
            let update_count: u32 = sections[0].parse().unwrap_or(0);
            let delete_count: u32 = sections[1].parse().unwrap_or(0);
            if update_count > 0 || delete_count > 0 {
                eprintln!(
                    "GVFS: {} placeholder(s) failed to update, {} failed to delete.",
                    update_count, delete_count
                );
            }
        }
    }

    // Special post-command actions.
    if git_verb == "fetch" || git_verb == "pull" {
        // Run prefetch for commits.
        debug!("Running post-fetch prefetch");
        let pipe_msg = PipeMessage::new("PostFetch", Some("[]".to_string()));
        let _ = client.send_receive(&pipe_msg);
    }

    Ok(())
}

fn is_passthrough(verb: &str) -> bool {
    PASSTHROUGH_COMMANDS.contains(&verb)
}

fn is_elevated() -> bool {
    // Simplified elevation check — use `whoami /priv` or just check env.
    // Full elevation detection via TOKEN_ELEVATION has complex Windows API surface issues.
    // For the hooks, this is best-effort.
    std::env::var("__COMPAT_LAYER")
        .map(|v| v.contains("RunAsAdmin"))
        .unwrap_or(false)
}
