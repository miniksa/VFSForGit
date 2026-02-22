//! post-index-changed — Native git hook that notifies the mount daemon
//! when the git index has changed.
//!
//! Called by git with two args: <updated-workdir> <updated-skipworktree>
//! Sends a PICN message to the GVFS mount daemon via named pipe.


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
            // Can't determine root — silently exit.
            std::process::exit(0);
        });

    // Parse the two flag arguments.
    let updated_workdir = args.get(1).map(|s| s.as_str()).unwrap_or("0");
    let updated_skip_worktree = args.get(2).map(|s| s.as_str()).unwrap_or("0");

    let body = format!("{}{}", updated_workdir, updated_skip_worktree);

    // Send PICN notification to mount daemon.
    match PipeClient::connect_to_mount(&root) {
        Ok(client) => {
            let msg = PipeMessage::new("PICN", Some(body));
            let _ = client.send_receive(&msg);
        }
        Err(_) => {
            // Mount not running — silently exit.
        }
    }
}
