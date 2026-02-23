//! Git utilities — running git commands, parsing index, hook management.

use std::path::Path;
use std::process::Command;

use tracing::debug;

/// Run a git command in the given working directory.
pub fn run_git(working_dir: &Path, args: &[&str]) -> anyhow::Result<String> {
    debug!("Running: git {} in {:?}", args.join(" "), working_dir);
    let output = Command::new("git")
        .args(args)
        .current_dir(working_dir)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {} failed: {}", args.join(" "), stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Run a git command, returning the output without checking exit code.
pub fn run_git_unchecked(working_dir: &Path, args: &[&str]) -> anyhow::Result<(i32, String, String)> {
    let output = Command::new("git")
        .args(args)
        .current_dir(working_dir)
        .output()?;

    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Ok((code, stdout, stderr))
}

/// Clone a repository with GVFS protocol support.
/// Clone a repository with GVFS protocol support.
/// git fetch handles authentication natively through GCM:
/// - First clone: GCM prompts interactively (browser/device code), caches token
/// - Subsequent clones: GCM returns cached token, no prompt
/// git internally calls credential fill/approve, so the token is persisted
/// automatically. Mount can then retrieve it silently.
pub fn clone_repo(
    remote_url: &str,
    target_dir: &Path,
    branch: Option<&str>,
    _no_mount: bool,
) -> anyhow::Result<()> {
    let src_dir = target_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;

    // Initialize the git repo.
    let output = Command::new("git")
        .args(["init", src_dir.to_str().unwrap()])
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "git init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Set the remote.
    run_git(&src_dir, &["remote", "add", "origin", remote_url])?;

    // Configure GVFS settings (same as C# TrySetRequiredGitConfigSettings).
    run_git(&src_dir, &["config", "core.gvfs", "7"])?;
    run_git(&src_dir, &["config", "core.bare", "false"])?;
    run_git(&src_dir, &["config", "core.sparseCheckout", "true"])?;
    run_git(&src_dir, &["config", "core.sparseCheckoutCone", "false"])?;
    run_git(&src_dir, &["config", "core.untrackedCache", "false"])?;
    run_git(&src_dir, &["config", "core.filemode", "false"])?;
    run_git(&src_dir, &["config", "core.fscache", "true"])?;
    run_git(&src_dir, &["config", "core.multiPackIndex", "true"])?;
    run_git(&src_dir, &["config", "core.preloadIndex", "true"])?;
    run_git(&src_dir, &["config", "core.protectNTFS", "false"])?;
    run_git(&src_dir, &["config", "gc.auto", "0"])?;
    run_git(&src_dir, &["config", "credential.validate", "false"])?;
    run_git(&src_dir, &["config", "credential.https://dev.azure.com.useHttpPath", "true"])?;

    // Configure hook paths.
    run_git(&src_dir, &["config", "core.hookspath", ".git/hooks"])?;
    configure_git_hooks(&src_dir)?;

    let branch_name = branch.unwrap_or("main");

    // Fetch the branch. GCM handles authentication:
    // - Prompts interactively on first use (browser/device code)
    // - Returns cached token on subsequent uses
    // git internally calls credential fill → use → approve, so the
    // token is persisted in GCM automatically after successful fetch.
    debug!("Fetching branch {} from {}", branch_name, remote_url);
    let fetch_ref = format!("{}:{}", branch_name, branch_name);
    run_git(
        &src_dir,
        &["fetch", "origin", &fetch_ref, "--no-tags", "--depth=1"],
    )?;

    // Point HEAD at the branch WITHOUT checking out files.
    // ProjFS requires the working directory to be empty so it can project virtually.
    run_git(&src_dir, &["symbolic-ref", "HEAD", &format!("refs/heads/{}", branch_name)])?;

    // Populate the git index from HEAD (no working tree files created).
    run_git(&src_dir, &["read-tree", "HEAD"])?;

    debug!("Clone completed to {:?}", target_dir);
    Ok(())
}

/// Configure git hooks for GVFS.
pub fn configure_git_hooks(git_working_dir: &Path) -> anyhow::Result<()> {
    let hooks_dir = git_working_dir.join(".git").join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;

    // Find our binaries relative to the current executable.
    let exe_dir = std::env::current_exe()?
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    // Write hook scripts that delegate to our binaries.
    let hooks = [
        (
            "read-object",
            exe_dir.join("read-object.exe").to_string_lossy().to_string(),
        ),
        (
            "post-index-change",
            exe_dir
                .join("post-index-changed.exe")
                .to_string_lossy()
                .to_string(),
        ),
        (
            "virtual-filesystem",
            exe_dir
                .join("virtual-filesystem.exe")
                .to_string_lossy()
                .to_string(),
        ),
    ];

    for (name, exe_path) in &hooks {
        let hook_path = hooks_dir.join(name);
        // On Windows, git can run .exe directly via the hook name.
        // Just create a shell script that invokes the exe.
        let script = format!("#!/bin/sh\nexec \"{}\" \"$@\"\n", exe_path.replace('\\', "/"));
        std::fs::write(&hook_path, script)?;
    }

    // Also write the pre-command and post-command hooks for gvfs-hooks.
    let gvfs_hooks_exe = exe_dir.join("gvfs-hooks.exe").to_string_lossy().to_string();
    for hook_name in &["pre-command", "post-command"] {
        let hook_path = hooks_dir.join(hook_name);
        let script = format!(
            "#!/bin/sh\nexec \"{}\" {} \"$@\"\n",
            gvfs_hooks_exe.replace('\\', "/"),
            hook_name
        );
        std::fs::write(&hook_path, script)?;
    }

    Ok(())
}

/// Parse git index to get the list of entries (path + SHA).
/// This is a simplified parser for the git index v2+ format.
#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub path: String,
    pub sha: String,
    pub mode: u32,
    pub file_size: u32,
    pub flags: u16,
}

pub fn parse_git_index(git_dir: &Path) -> anyhow::Result<Vec<IndexEntry>> {
    use byteorder::{BigEndian, ReadBytesExt};
    use std::io::{Cursor, Read};

    let index_path = git_dir.join("index");
    if !index_path.exists() {
        return Ok(Vec::new());
    }

    let data = std::fs::read(&index_path)?;
    let mut cursor = Cursor::new(&data);

    // Header: DIRC + version (4 bytes) + entry count (4 bytes)
    let mut sig = [0u8; 4];
    cursor.read_exact(&mut sig)?;
    if &sig != b"DIRC" {
        anyhow::bail!("Invalid git index signature");
    }

    let version = cursor.read_u32::<BigEndian>()?;
    let entry_count = cursor.read_u32::<BigEndian>()?;
    debug!("Git index version {}, {} entries", version, entry_count);

    let mut entries = Vec::with_capacity(entry_count as usize);

    for _ in 0..entry_count {
        let entry_start = cursor.position();

        // Skip ctime, mtime (8 bytes each), dev, ino (4 bytes each)
        let _ctime_s = cursor.read_u32::<BigEndian>()?;
        let _ctime_n = cursor.read_u32::<BigEndian>()?;
        let _mtime_s = cursor.read_u32::<BigEndian>()?;
        let _mtime_n = cursor.read_u32::<BigEndian>()?;
        let _dev = cursor.read_u32::<BigEndian>()?;
        let _ino = cursor.read_u32::<BigEndian>()?;
        let mode = cursor.read_u32::<BigEndian>()?;
        let _uid = cursor.read_u32::<BigEndian>()?;
        let _gid = cursor.read_u32::<BigEndian>()?;
        let file_size = cursor.read_u32::<BigEndian>()?;

        // SHA-1 (20 bytes)
        let mut sha_bytes = [0u8; 20];
        cursor.read_exact(&mut sha_bytes)?;
        let sha = hex::encode(sha_bytes);

        // Flags (2 bytes) — lower 12 bits = name length
        let flags = cursor.read_u16::<BigEndian>()?;
        let name_len = (flags & 0x0FFF) as usize;

        // Extended flags for version 3+
        if version >= 3 && (flags & 0x4000) != 0 {
            let _ext_flags = cursor.read_u16::<BigEndian>()?;
        }

        // Path name (null-terminated, padded to 8-byte boundary)
        let mut name_buf = vec![0u8; name_len];
        cursor.read_exact(&mut name_buf)?;
        let path = String::from_utf8_lossy(&name_buf).to_string();

        // Read padding (1-8 null bytes to align to 8-byte boundary)
        let entry_size = (cursor.position() - entry_start) as usize;
        let padded_size = (entry_size + 8) & !7;
        let padding = padded_size - entry_size;
        let mut pad = vec![0u8; padding];
        cursor.read_exact(&mut pad)?;

        entries.push(IndexEntry {
            path,
            sha,
            mode,
            file_size,
            flags,
        });
    }

    Ok(entries)
}

/// Get the tree entries for a given path from the git index.
/// Returns (directories, files) at the given directory level.
pub fn get_tree_entries(
    entries: &[IndexEntry],
    dir_path: &str,
) -> (Vec<String>, Vec<(String, IndexEntry)>) {
    let prefix = if dir_path.is_empty() {
        String::new()
    } else {
        format!("{}/", dir_path.trim_end_matches('/'))
    };

    let mut dirs = std::collections::BTreeSet::new();
    let mut files = Vec::new();

    for entry in entries {
        if !entry.path.starts_with(&prefix) && !prefix.is_empty() {
            continue;
        }
        if prefix.is_empty() && entry.path.is_empty() {
            continue;
        }

        let relative = if prefix.is_empty() {
            &entry.path
        } else {
            &entry.path[prefix.len()..]
        };

        if let Some(slash_pos) = relative.find('/') {
            // This is a subdirectory
            let dir_name = &relative[..slash_pos];
            dirs.insert(dir_name.to_string());
        } else {
            // This is a file at this level
            files.push((relative.to_string(), entry.clone()));
        }
    }

    (dirs.into_iter().collect(), files)
}
