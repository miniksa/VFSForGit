//! virtual-filesystem — Native git hook that provides the modified paths list.
//!
//! Called by git (as the virtual-filesystem hook) to enumerate which paths
//! are modified/present on disk. Sends an MPL request to the GVFS mount
//! daemon and returns the list to git via stdout.

use std::io::Write;

use gvfs_native_hooks::{get_enlistment_root_from_cwd, get_enlistment_root_from_hook};
use gvfs_common::pipe::{PipeClient, PipeMessage};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Determine enlistment root.
    let root = args
        .first()
        .and_then(|a| get_enlistment_root_from_hook(a))
        .or_else(get_enlistment_root_from_cwd)
        .unwrap_or_else(|| {
            std::process::exit(0);
        });

    // Request modified paths list from mount daemon.
    match PipeClient::connect_to_mount(&root) {
        Ok(client) => {
            let msg = PipeMessage::new("MPL", Some("1".to_string()));
            match client.send_receive(&msg) {
                Ok(response) => {
                    if response.header == "S" {
                        if let Some(body) = &response.body {
                            // Body is null-delimited paths. Convert to newline-delimited
                            // for git's virtual-filesystem hook.
                            let stdout = std::io::stdout();
                            let mut out = stdout.lock();
                            for path in body.split('\0') {
                                if !path.is_empty() {
                                    let _ = writeln!(out, "{}", path);
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    // Communication error — silently exit.
                }
            }
        }
        Err(_) => {
            // Mount not running — silently exit.
        }
    }
}
