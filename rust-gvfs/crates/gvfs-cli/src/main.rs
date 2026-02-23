//! gvfs.exe — Main CLI for VFSForGit (Rust rewrite).
//!
//! Supports all major verb commands needed for the benchmark:
//! version, clone, mount, status, prefetch, unmount
//! Plus additional commands: config, log, diagnose, health, repair,
//! service, sparse, dehydrate, upgrade, cache-server.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Parser, Subcommand};
use tracing::debug;
use tracing_subscriber::EnvFilter;

use gvfs_common::config;
use gvfs_common::constants;
use gvfs_common::enlistment::GvfsEnlistment;
use gvfs_common::http::{GitAuth, GvfsClient};
use gvfs_common::pipe::{self, PipeClient, PipeMessage};
use gvfs_common::repo_metadata::RepoMetadata;

#[derive(Parser)]
#[command(name = "gvfs", about = "VFSForGit — Virtual File System for Git (Rust)")]
#[command(version = constants::GVFS_VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Display the version of GVFS.
    Version,

    /// Clone a repository with GVFS support.
    Clone {
        /// Repository URL.
        #[arg(index = 1)]
        url: String,
        /// Target directory.
        #[arg(index = 2)]
        path: String,
        /// Branch to checkout.
        #[arg(long)]
        branch: Option<String>,
        /// Don't mount after cloning.
        #[arg(long = "no-mount")]
        no_mount: bool,
    },

    /// Mount a GVFS enlistment.
    Mount {
        /// Path to the enlistment root.
        #[arg(index = 1)]
        path: Option<String>,
        /// Internal use: JSON config passed during auto-mount.
        #[arg(long = "internal_use_only", hide = true)]
        internal_config: Option<String>,
    },

    /// Get mount status.
    Status {
        /// Path to the enlistment root.
        #[arg(index = 1)]
        path: Option<String>,
    },

    /// Unmount a GVFS enlistment.
    Unmount {
        /// Path to the enlistment root.
        #[arg(index = 1)]
        path: Option<String>,
    },

    /// Prefetch git objects.
    Prefetch {
        /// Path to the enlistment root.
        #[arg(index = 1)]
        path: Option<String>,
        /// File patterns to prefetch.
        #[arg(long)]
        files: Option<String>,
        /// Prefetch commits and trees.
        #[arg(long)]
        commits: bool,
    },

    /// Configure the cache server.
    CacheServer {
        #[arg(index = 1)]
        path: Option<String>,
        /// Set the cache server URL.
        #[arg(long)]
        set: Option<String>,
        /// List available cache servers.
        #[arg(long)]
        list: bool,
    },

    /// Manage local GVFS configuration.
    Config {
        /// Configuration key.
        #[arg(index = 1)]
        key: Option<String>,
        /// Configuration value.
        #[arg(index = 2)]
        value: Option<String>,
    },

    /// Dehydrate folders in the enlistment.
    Dehydrate {
        #[arg(index = 1)]
        path: Option<String>,
        /// Confirm dehydration (required).
        #[arg(long)]
        confirm: bool,
    },

    /// Run diagnostic checks.
    Diagnose {
        #[arg(index = 1)]
        path: Option<String>,
    },

    /// Show enlistment health information.
    Health {
        #[arg(index = 1)]
        path: Option<String>,
        /// Show directory-level statistics.
        #[arg(long)]
        directory: Option<String>,
    },

    /// Open GVFS log.
    Log {
        #[arg(index = 1)]
        path: Option<String>,
    },

    /// Repair a GVFS enlistment.
    Repair {
        #[arg(index = 1)]
        path: Option<String>,
        /// Confirm repair (required).
        #[arg(long)]
        confirm: bool,
    },

    /// Manage the GVFS service.
    Service {
        /// List registered repos.
        #[arg(long)]
        list_repos: bool,
    },

    /// Manage sparse checkout.
    Sparse {
        #[arg(index = 1)]
        path: Option<String>,
        /// Set sparse folders.
        #[arg(long)]
        set: Option<String>,
        /// Add sparse folders.
        #[arg(long)]
        add: Option<String>,
        /// Remove sparse folders.
        #[arg(long)]
        remove: Option<String>,
        /// List sparse folders.
        #[arg(long)]
        list: bool,
    },

    /// Check for upgrades.
    Upgrade,
}

fn main() {
    // Initialize tracing.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Version => cmd_version(),
        Commands::Clone {
            url,
            path,
            branch,
            no_mount,
        } => cmd_clone(&url, &path, branch.as_deref(), no_mount),
        Commands::Mount {
            path,
            internal_config,
        } => cmd_mount(path.as_deref(), internal_config.as_deref()),
        Commands::Status { path } => cmd_status(path.as_deref()),
        Commands::Unmount { path } => cmd_unmount(path.as_deref()),
        Commands::Prefetch {
            path,
            files,
            commits,
        } => cmd_prefetch(path.as_deref(), files.as_deref(), commits),
        Commands::CacheServer { path, set, list } => {
            cmd_cache_server(path.as_deref(), set.as_deref(), list)
        }
        Commands::Config { key, value } => cmd_config(key.as_deref(), value.as_deref()),
        Commands::Dehydrate { path, confirm } => cmd_dehydrate(path.as_deref(), confirm),
        Commands::Diagnose { path } => cmd_diagnose(path.as_deref()),
        Commands::Health { path, directory } => {
            cmd_health(path.as_deref(), directory.as_deref())
        }
        Commands::Log { path } => cmd_log(path.as_deref()),
        Commands::Repair { path, confirm } => cmd_repair(path.as_deref(), confirm),
        Commands::Service { list_repos } => cmd_service(list_repos),
        Commands::Sparse {
            path,
            set,
            add,
            remove,
            list,
        } => cmd_sparse(
            path.as_deref(),
            set.as_deref(),
            add.as_deref(),
            remove.as_deref(),
            list,
        ),
        Commands::Upgrade => cmd_upgrade(),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Command implementations
// ─────────────────────────────────────────────────────────────────────────────

fn cmd_version() -> anyhow::Result<()> {
    println!("GVFS version {}", constants::GVFS_VERSION);
    Ok(())
}

fn cmd_clone(
    url: &str,
    path: &str,
    branch: Option<&str>,
    no_mount: bool,
) -> anyhow::Result<()> {
    let target = PathBuf::from(path);
    println!("Cloning {} into {:?}...", url, target);

    // Create the enlistment structure.
    let mut enlistment = GvfsEnlistment::new(&target).with_remote(url.to_string());
    if let Some(b) = branch {
        enlistment = enlistment.with_branch(b.to_string());
    }
    enlistment.create_directories()?;

    // Create .gvfs directory and RepoMetadata.
    let enlistment_id = uuid::Uuid::new_v4().to_string();
    let git_objects_root = enlistment.git_objects_root().to_string_lossy().to_string();
    let local_cache_root = enlistment.local_cache_root().to_string_lossy().to_string();

    RepoMetadata::create(
        enlistment.repo_metadata_path(),
        &enlistment_id,
        &git_objects_root,
        &local_cache_root,
    )?;

    // Clone the repo (fetch + checkout).
    gvfs_common::git::clone_repo(url, &target, branch, no_mount)?;

    println!("Clone complete.");

    // Mount unless --no-mount was specified.
    if !no_mount {
        println!("Mounting...");
        let exe = std::env::current_exe()?;
        let target_str = target.to_string_lossy().to_string();
        let mount_status = Command::new(&exe)
            .args(["mount", &target_str])
            .status()?;
        if !mount_status.success() {
            eprintln!("Warning: mount failed, you can mount manually with: gvfs mount {}", path);
        }
    }

    Ok(())
}

fn cmd_mount(path: Option<&str>, _internal_config: Option<&str>) -> anyhow::Result<()> {
    let root = resolve_enlistment(path)?;
    let enlistment = GvfsEnlistment::new(&root);

    if !enlistment.is_valid() {
        anyhow::bail!("{:?} is not a valid GVFS enlistment", root);
    }

    // Check if already mounted.
    if pipe::is_mounted(&root) {
        println!("Already mounted at {:?}", root);
        return Ok(());
    }

    // Launch gvfs-mount as a background process.
    let exe_dir = std::env::current_exe()?
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let mount_exe = exe_dir.join("gvfs-mount.exe");

    if !mount_exe.exists() {
        // Fall back to looking next to gvfs.exe
        anyhow::bail!(
            "gvfs-mount.exe not found at {:?}. Build all binaries first.",
            mount_exe
        );
    }

    println!("Mounting {:?}...", root);

    // Launch as a detached background process.
    let root_str = root.to_string_lossy().to_string();
    let _child = Command::new(&mount_exe)
        .arg(&root_str)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    // Wait for mount to become ready (poll the named pipe).
    let max_wait = std::time::Duration::from_secs(60);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > max_wait {
            anyhow::bail!("Mount timed out after 30 seconds");
        }

        std::thread::sleep(std::time::Duration::from_millis(250));

        match pipe::get_mount_status(&root) {
            Ok(status) => {
                if status.contains("Ready") {
                    println!("Mount successful.");
                    // Register with service (best-effort).
                    let _ = register_with_service(&root);
                    return Ok(());
                }
                // Show progress dots every 2 seconds
                if start.elapsed().as_secs() % 2 == 0 {
                    eprint!(".");
                }
            }
            Err(_) => {
                // Not ready yet.
                continue;
            }
        }
    }
}

fn cmd_status(path: Option<&str>) -> anyhow::Result<()> {
    let root = resolve_enlistment(path)?;

    match pipe::get_mount_status(&root) {
        Ok(status) => {
            // Extract the body from the pipe response (format: "S|body")
            if let Some(body) = status.strip_prefix("S|") {
                println!("{}", body);
            } else {
                println!("{}", status);
            }
        }
        Err(_) => {
            println!("Not mounted: {:?}", root);
        }
    }
    Ok(())
}

fn cmd_unmount(path: Option<&str>) -> anyhow::Result<()> {
    let root = resolve_enlistment(path)?;

    println!("Unmounting {:?}...", root);
    match pipe::request_unmount(&root) {
        Ok(response) => {
            println!("Unmount: {}", response);
        }
        Err(e) => {
            println!("Not mounted or unmount failed: {}", e);
        }
    }
    Ok(())
}

fn cmd_prefetch(
    path: Option<&str>,
    files: Option<&str>,
    _commits: bool,
) -> anyhow::Result<()> {
    let root = resolve_enlistment(path)?;
    let enlistment = GvfsEnlistment::new(&root);

    // Get authentication.
    let mut enlistment_obj = enlistment.clone();
    enlistment_obj.resolve_remote_url()?;

    let remote_url = enlistment_obj
        .remote_url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("No remote URL configured"))?;

    let auth = GitAuth::from_repo(&enlistment.working_dir(), remote_url).ok();
    let client = GvfsClient::new(remote_url, auth);

    let rt = tokio::runtime::Runtime::new()?;

    if let Some(file_pattern) = files {
        println!("Prefetching files matching '{}'...", file_pattern);
        // For file prefetch, we need to get the list of matching objects
        // and batch download them.
        println!("File prefetch not yet implemented for pattern filters.");
        println!("Use --commits for commit/tree prefetch.");
    } else {
        println!("Prefetching commits and trees...");
        let pack_dir = enlistment.git_pack_dir();
        let packs = rt.block_on(async { client.prefetch(0, &pack_dir).await })?;
        println!("Downloaded {} pack files.", packs.len());

        // Index the downloaded packs.
        for pack in &packs {
            let idx_path = pack.with_extension("idx");
            if !idx_path.exists() {
                let _ = std::process::Command::new("git")
                    .args(["index-pack", &pack.to_string_lossy()])
                    .current_dir(enlistment.working_dir())
                    .status();
            }
        }
    }

    Ok(())
}

fn cmd_cache_server(
    path: Option<&str>,
    set: Option<&str>,
    list: bool,
) -> anyhow::Result<()> {
    let root = resolve_enlistment(path)?;
    let enlistment = GvfsEnlistment::new(&root);

    if list {
        println!("Querying cache servers...");
        let mut enl = enlistment.clone();
        enl.resolve_remote_url()?;

        if let Some(url) = &enl.remote_url {
            let auth = GitAuth::from_repo(&enl.working_dir(), url).ok();
            let client = GvfsClient::new(url, auth);
            let rt = tokio::runtime::Runtime::new()?;
            match rt.block_on(async { client.get_config().await }) {
                Ok(config) => {
                    if let Some(servers) = config.cache_servers {
                        for server in &servers {
                            println!(
                                "  {} ({})",
                                server.url,
                                server.name.as_deref().unwrap_or("unnamed")
                            );
                        }
                    } else {
                        println!("No cache servers configured.");
                    }
                }
                Err(e) => {
                    eprintln!("Failed to query config: {}", e);
                }
            }
        }
    }

    if let Some(url) = set {
        gvfs_common::git::run_git(
            &enlistment.working_dir(),
            &["config", "gvfs.cache-server", url],
        )?;
        println!("Cache server set to: {}", url);
    }

    Ok(())
}

fn cmd_config(key: Option<&str>, value: Option<&str>) -> anyhow::Result<()> {
    let mut config = config::LocalConfig::load_or_create()?;

    match (key, value) {
        (Some(k), Some(v)) => {
            config.set(k, v);
            println!("Set {} = {}", k, v);
        }
        (Some(k), None) => {
            match config.get(k) {
                Some(v) => println!("{}", v),
                None => println!("(not set)"),
            }
        }
        _ => {
            println!("Usage: gvfs config <key> [value]");
        }
    }
    Ok(())
}

fn cmd_dehydrate(path: Option<&str>, confirm: bool) -> anyhow::Result<()> {
    if !confirm {
        println!("To confirm dehydration, run: gvfs dehydrate --confirm");
        return Ok(());
    }
    let root = resolve_enlistment(path)?;
    println!("Dehydrating {:?}... (not yet implemented)", root);
    Ok(())
}

fn cmd_diagnose(path: Option<&str>) -> anyhow::Result<()> {
    let root = resolve_enlistment(path)?;
    let enlistment = GvfsEnlistment::new(&root);

    println!("GVFS Diagnostics for {:?}", root);
    println!("  .gvfs exists: {}", enlistment.dot_gvfs_dir().exists());
    println!(
        "  src/.git exists: {}",
        enlistment.git_dir().exists()
    );
    println!("  Mounted: {}", pipe::is_mounted(&root));

    if let Ok(meta) = RepoMetadata::load(enlistment.repo_metadata_path()) {
        println!(
            "  Disk layout version: {}",
            meta.get("DiskLayoutVersion").unwrap_or("unknown")
        );
        println!(
            "  Enlistment ID: {}",
            meta.enlistment_id().unwrap_or("unknown")
        );
    }

    Ok(())
}

fn cmd_health(path: Option<&str>, _directory: Option<&str>) -> anyhow::Result<()> {
    let root = resolve_enlistment(path)?;
    println!("Health check for {:?} (not yet implemented)", root);
    Ok(())
}

fn cmd_log(path: Option<&str>) -> anyhow::Result<()> {
    let root = resolve_enlistment(path)?;
    let enlistment = GvfsEnlistment::new(&root);
    let log_dir = enlistment.logs_dir();

    if log_dir.exists() {
        println!("Log directory: {:?}", log_dir);
        // Open the most recent log file.
        let mut entries: Vec<_> = std::fs::read_dir(&log_dir)?
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());
        if let Some(last) = entries.last() {
            let _ = Command::new("notepad").arg(last.path()).spawn();
        }
    } else {
        println!("No logs found at {:?}", log_dir);
    }
    Ok(())
}

fn cmd_repair(path: Option<&str>, confirm: bool) -> anyhow::Result<()> {
    if !confirm {
        println!("To confirm repair, run: gvfs repair --confirm");
        return Ok(());
    }
    let root = resolve_enlistment(path)?;
    println!("Repairing {:?}... (not yet implemented)", root);
    Ok(())
}

fn cmd_service(list_repos: bool) -> anyhow::Result<()> {
    if list_repos {
        match PipeClient::connect_to_service() {
            Ok(client) => {
                let resp = client
                    .send_receive(&PipeMessage::new("GetActiveRepoListRequest", None))?;
                println!("Active repos: {}", resp.to_wire());
            }
            Err(_) => {
                println!("GVFS service is not running.");
            }
        }
    } else {
        println!("Usage: gvfs service --list-repos");
    }
    Ok(())
}

fn cmd_sparse(
    path: Option<&str>,
    _set: Option<&str>,
    _add: Option<&str>,
    _remove: Option<&str>,
    _list: bool,
) -> anyhow::Result<()> {
    let root = resolve_enlistment(path)?;
    println!("Sparse checkout management for {:?} (not yet implemented)", root);
    Ok(())
}

fn cmd_upgrade() -> anyhow::Result<()> {
    println!("GVFS version {}", constants::GVFS_VERSION);
    println!("No upgrades available (Rust build).");
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve the enlistment root from a path argument or current directory.
fn resolve_enlistment(path: Option<&str>) -> anyhow::Result<PathBuf> {
    let start = match path {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir()?,
    };

    // If the path itself has .gvfs, use it directly.
    if start.join(".gvfs").is_dir() {
        return Ok(start);
    }

    // Try to find the enlistment root by walking up.
    GvfsEnlistment::find_root(&start)
        .ok_or_else(|| anyhow::anyhow!("Not a GVFS enlistment: {:?}", start))
}

/// Register this enlistment with the GVFS service (best-effort).
fn register_with_service(root: &Path) -> anyhow::Result<()> {
    let client = PipeClient::connect_to_service()?;
    let body = serde_json::json!({
        "EnlistmentRoot": root.to_string_lossy(),
    });
    let msg = PipeMessage::new("RegisterRepoRequest", Some(body.to_string()));
    let _ = client.send_receive(&msg)?;
    Ok(())
}
