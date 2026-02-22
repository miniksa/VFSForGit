//! GVFS constants — paths, protocol strings, defaults.

/// The GVFS dot directory name.
pub const DOT_GVFS: &str = ".gvfs";

/// Databases sub-directory inside .gvfs.
pub const DATABASES_DIR: &str = "databases";

/// Repo metadata file name.
pub const REPO_METADATA_FILE: &str = "RepoMetadata.dat";

/// Modified paths file name.
pub const MODIFIED_PATHS_FILE: &str = "ModifiedPaths.dat";

/// Placeholder list file name.
pub const PLACEHOLDER_LIST_FILE: &str = "PlaceholderList.dat";

/// SQLite database file name.
pub const SQLITE_DB_FILE: &str = "VFSForGit.sqlite";

/// Working directory name inside enlistment root.
pub const WORKING_DIR: &str = "src";

/// Named pipe message terminator byte (ETX).
pub const ETX: u8 = 0x03;

/// Named pipe message header/body separator.
pub const PIPE_SEPARATOR: char = '|';

/// The pipe name prefix.
pub const PIPE_NAME_PREFIX: &str = "GVFS_";

/// GVFS protocol endpoints.
pub mod endpoints {
    pub const GVFS_CONFIG: &str = "/gvfs/config";
    pub const GVFS_OBJECTS: &str = "/gvfs/objects";
    pub const GVFS_PREFETCH: &str = "/gvfs/prefetch";
    pub const GVFS_SIZES: &str = "/gvfs/sizes";
    pub const INFO_REFS: &str = "/info/refs?service=git-upload-pack";
}

/// Named pipe message headers.
pub mod pipe_headers {
    pub const GET_STATUS: &str = "QueryGVFSConfig";
    pub const UNMOUNT: &str = "Unmount";
    pub const ACQUIRE_LOCK: &str = "AcquireLock";
    pub const RELEASE_LOCK: &str = "ReleaseLock";
    pub const DOWNLOAD_OBJECT: &str = "DLO";
    pub const MODIFIED_PATHS_LIST: &str = "MPL";
    pub const POST_INDEX_CHANGED: &str = "PICN";
    pub const POST_FETCH: &str = "PostFetch";
    pub const DEHYDRATE: &str = "Dehydrate";

    // Service pipe headers
    pub const REGISTER_REPO: &str = "RegisterRepoRequest";
    pub const UNREGISTER_REPO: &str = "UnregisterRepoRequest";
    pub const ENABLE_PROJFS: &str = "EnableAndAttachProjFSRequest";
    pub const GET_ACTIVE_REPOS: &str = "GetActiveRepoListRequest";
    pub const NOTIFICATION: &str = "Notification";
}

/// Lock acquisition result strings.
pub mod lock_responses {
    pub const LOCK_ACQUIRED: &str = "LockAcquired";
    pub const LOCK_AVAILABLE: &str = "LockAvailable";
    pub const LOCK_DENIED_GVFS: &str = "LockDeniedGVFS";
    pub const LOCK_DENIED_GIT: &str = "LockDeniedGit";
    pub const MOUNT_NOT_READY: &str = "MountNotReady";
    pub const UNMOUNT_IN_PROGRESS: &str = "UnmountInProgress";
}

/// Git config keys used by GVFS.
pub mod git_config {
    pub const CORE_GVFS: &str = "core.gvfs";
    pub const GVFS_CACHE_SERVER: &str = "gvfs.cache-server";
    pub const GVFS_MAX_RETRIES: &str = "gvfs.max-retries";
    pub const GVFS_TIMEOUT: &str = "gvfs.timeout-seconds";
    pub const GVFS_MOUNT_ID: &str = "gvfs.mount-id";
    pub const GVFS_ENLISTMENT_ID: &str = "gvfs.enlistment-id";
    pub const GVFS_TELEMETRY_ID: &str = "gvfs.telemetry-id";
}

/// Default values.
pub const DEFAULT_MAX_RETRIES: u32 = 6;
pub const DEFAULT_TIMEOUT_SECS: u64 = 300;
pub const GVFS_VERSION: &str = "0.1.0-rust";
pub const NULL_SHA: &str = "0000000000000000000000000000000000000000";
