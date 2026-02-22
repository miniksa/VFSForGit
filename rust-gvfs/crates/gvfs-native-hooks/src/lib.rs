//! Shared utilities for native git hooks.

use std::path::{Path, PathBuf};

use gvfs_common::enlistment::GvfsEnlistment;
use gvfs_common::pipe;

/// Determine the enlistment root from the hook's argv[0] path.
/// Git hooks are installed at `<enlistment>/src/.git/hooks/<name>`.
pub fn get_enlistment_root_from_hook(hook_path: &str) -> Option<PathBuf> {
    let path = PathBuf::from(hook_path);
    // Walk up: hooks/ -> .git/ -> src/ -> <root>
    path.parent() // hooks
        .and_then(|p| p.parent()) // .git
        .and_then(|p| p.parent()) // src
        .and_then(|p| p.parent()) // root
        .map(|p| p.to_path_buf())
}

/// Determine the enlistment root from the current working directory.
pub fn get_enlistment_root_from_cwd() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    GvfsEnlistment::find_root(&cwd)
}

/// Send a DLO message to download a git object.
pub fn download_object(enlistment_root: &Path, sha: &str) -> bool {
    pipe::download_object(enlistment_root, sha).unwrap_or(false)
}
