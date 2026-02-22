//! The main GVFS virtualizer — orchestrates ProjFS, projection, and object download.

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use tracing::{debug, error, info};
use widestring::U16CString;
use windows::core::{HRESULT, PCWSTR};
use windows_core::GUID;

use projfs::ffi::*;

use gvfs_common::enlistment::GvfsEnlistment;
use gvfs_common::http::GvfsClient;
use gvfs_common::modified_paths::ModifiedPaths;

use crate::callbacks;
use crate::projection::{GitIndexProjection, ProjectedEntry};

// S_OK
const S_OK: HRESULT = HRESULT(0);
// HRESULT for ERROR_FILE_NOT_FOUND
const E_FILE_NOT_FOUND: HRESULT = HRESULT(0x80070002u32 as i32);
// HRESULT for ERROR_INSUFFICIENT_BUFFER
const E_INSUFFICIENT_BUFFER: HRESULT = HRESULT(0x8007007Au32 as i32);

/// Active directory enumeration session.
struct EnumSession {
    entries: Vec<ProjectedEntry>,
    index: usize,
    search_set: bool,
}

/// The main virtualizer that implements all ProjFS callbacks.
pub struct GvfsVirtualizer {
    enlistment: GvfsEnlistment,
    projection: Arc<GitIndexProjection>,
    modified_paths: Arc<ModifiedPaths>,
    enumerations: Mutex<HashMap<GUID, EnumSession>>,
    http_client: Option<Arc<GvfsClient>>,
    tokio_rt: tokio::runtime::Runtime,
}

impl GvfsVirtualizer {
    pub fn new(
        enlistment: GvfsEnlistment,
        http_client: Option<Arc<GvfsClient>>,
    ) -> anyhow::Result<Self> {
        let projection = Arc::new(GitIndexProjection::new());
        let modified_paths = Arc::new(
            ModifiedPaths::load_or_create(enlistment.modified_paths_path())?,
        );

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;

        Ok(Self {
            enlistment,
            projection,
            modified_paths,
            enumerations: Mutex::new(HashMap::new()),
            http_client,
            tokio_rt: rt,
        })
    }

    /// Load the git index projection.
    pub fn load_projection(&self) -> anyhow::Result<()> {
        self.projection.load_from_index(&self.enlistment.git_dir())?;
        info!(
            "Projection loaded: {} entries",
            self.projection.entry_count()
        );
        Ok(())
    }

    /// Get the projection (for external queries like MPL).
    pub fn projection(&self) -> &GitIndexProjection {
        &self.projection
    }

    /// Get the modified paths database.
    pub fn modified_paths(&self) -> &ModifiedPaths {
        &self.modified_paths
    }

    // ─────────────────────────────────────────────────────────────────────
    // ProjFS callback implementations
    // ─────────────────────────────────────────────────────────────────────

    pub fn start_dir_enum(&self, path: &str, enumeration_id: GUID) -> HRESULT {
        let entries = self.projection.get_entries(path);
        let mut enums = self.enumerations.lock();
        enums.insert(
            enumeration_id,
            EnumSession {
                entries,
                index: 0,
                search_set: false,
            },
        );
        S_OK
    }

    pub fn end_dir_enum(&self, _path: &str, enumeration_id: GUID) -> HRESULT {
        let mut enums = self.enumerations.lock();
        enums.remove(&enumeration_id);
        S_OK
    }

    pub fn get_dir_enum(
        &self,
        _path: &str,
        enumeration_id: GUID,
        search_expression: Option<&str>,
        dir_entry_buffer: PRJ_DIR_ENTRY_BUFFER_HANDLE,
    ) -> HRESULT {
        let mut enums = self.enumerations.lock();
        let session = match enums.get_mut(&enumeration_id) {
            Some(s) => s,
            None => return E_FILE_NOT_FOUND,
        };

        // If a new search expression is provided and we haven't set one yet,
        // reset the index. ProjFS may call this multiple times for the same enum.
        if search_expression.is_some() && !session.search_set {
            session.index = 0;
            session.search_set = true;
        }

        let search = search_expression.unwrap_or("*");
        let mut added_any = false;

        while session.index < session.entries.len() {
            let entry = &session.entries[session.index];
            let name = entry.name();

            // Check if entry matches the search pattern.
            if search != "*" && !projfs::file_name_match(name, search) {
                session.index += 1;
                continue;
            }

            let basic_info = match entry {
                ProjectedEntry::File { size, .. } => {
                    let mut info = PRJ_FILE_BASIC_INFO::default();
                    info.IsDirectory = 0;
                    info.FileSize = *size as i64;
                    info
                }
                ProjectedEntry::Directory { .. } => {
                    let mut info = PRJ_FILE_BASIC_INFO::default();
                    info.IsDirectory = 1;
                    info.FileAttributes = 0x10; // FILE_ATTRIBUTE_DIRECTORY
                    info
                }
            };

            match projfs::fill_dir_entry(dir_entry_buffer, name, &basic_info) {
                Ok(()) => {
                    added_any = true;
                    session.index += 1;
                }
                Err(_) => {
                    // Buffer full — return what we have so far.
                    if added_any {
                        return S_OK;
                    } else {
                        return E_INSUFFICIENT_BUFFER;
                    }
                }
            }
        }

        S_OK
    }

    pub fn get_placeholder_info(
        &self,
        path: &str,
        ctx: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    ) -> HRESULT {
        let entry = match self.projection.get_entry(path) {
            Some(e) => e,
            None => return E_FILE_NOT_FOUND,
        };

        let placeholder = match &entry {
            ProjectedEntry::File { sha, size, .. } => {
                projfs::build_file_placeholder(sha, *size as i64)
            }
            ProjectedEntry::Directory { .. } => projfs::build_dir_placeholder(),
        };

        match projfs::write_placeholder_info(ctx, path, &placeholder) {
            Ok(()) => S_OK,
            Err(e) => {
                error!("Failed to write placeholder for {}: {:?}", path, e);
                HRESULT(0x80004005u32 as i32) // E_FAIL
            }
        }
    }

    pub fn get_file_data(
        &self,
        path: &str,
        ctx: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
        data_stream_id: GUID,
        byte_offset: u64,
        length: u32,
    ) -> HRESULT {
        // Look up the SHA for this file.
        let sha = match self.projection.get_entry(path) {
            Some(ProjectedEntry::File { sha, .. }) => sha,
            _ => return E_FILE_NOT_FOUND,
        };

        // Try to read from local git object store first.
        let working_dir = self.enlistment.working_dir();
        let data = match self.read_git_object(&sha, &working_dir) {
            Ok(data) => data,
            Err(e) => {
                debug!("Object {} not local, downloading: {}", sha, e);
                // Download via HTTP.
                match self.download_and_read_object(&sha, &working_dir) {
                    Ok(data) => data,
                    Err(e) => {
                        error!("Failed to download object {}: {}", sha, e);
                        return HRESULT(0x80004005u32 as i32);
                    }
                }
            }
        };

        // Write the requested portion of the file.
        let offset = byte_offset as usize;
        let len = length as usize;
        let end = (offset + len).min(data.len());
        let slice = &data[offset..end];

        match projfs::write_file_data(ctx, &data_stream_id, slice, byte_offset) {
            Ok(()) => S_OK,
            Err(e) => {
                error!("Failed to write file data for {}: {:?}", path, e);
                HRESULT(0x80004005u32 as i32)
            }
        }
    }

    pub fn query_file_name(&self, path: &str) -> HRESULT {
        if self.projection.path_exists(path) {
            S_OK
        } else {
            E_FILE_NOT_FOUND
        }
    }

    pub fn notification(
        &self,
        path: &str,
        is_directory: bool,
        notification: u32,
        dest_file_name: Option<&str>,
    ) -> HRESULT {
        // Track modifications in the modified paths database.
        match notification {
            PRJ_NOTIFY_NEW_FILE_CREATED => {
                if is_directory {
                    self.modified_paths.add_folder(path);
                } else {
                    self.modified_paths.add_file(path);
                }
            }
            PRJ_NOTIFY_FILE_OVERWRITTEN
            | PRJ_NOTIFY_FILE_HANDLE_CLOSED_FILE_MODIFIED => {
                self.modified_paths.add_file(path);
            }
            PRJ_NOTIFY_FILE_RENAMED => {
                if let Some(dest) = dest_file_name {
                    if is_directory {
                        self.modified_paths.add_folder(dest);
                    } else {
                        self.modified_paths.add_file(dest);
                    }
                }
            }
            PRJ_NOTIFY_FILE_HANDLE_CLOSED_FILE_DELETED => {
                // File deleted — nothing to track (it will be re-projected).
            }
            PRJ_NOTIFY_FILE_PRE_CONVERT_TO_FULL => {
                self.modified_paths.add_file(path);
            }
            _ => {}
        }

        S_OK
    }

    // ─────────────────────────────────────────────────────────────────────
    // Git object store
    // ─────────────────────────────────────────────────────────────────────

    /// Read a git object from the local object database.
    fn read_git_object(&self, sha: &str, working_dir: &Path) -> anyhow::Result<Vec<u8>> {
        // Use `git cat-file -p <sha>` to get the content.
        let output = std::process::Command::new("git")
            .args(["cat-file", "blob", sha])
            .current_dir(working_dir)
            .output()?;

        if !output.status.success() {
            anyhow::bail!(
                "git cat-file failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(output.stdout)
    }

    /// Download an object via HTTP and then read it.
    fn download_and_read_object(
        &self,
        sha: &str,
        working_dir: &Path,
    ) -> anyhow::Result<Vec<u8>> {
        let client = self
            .http_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No HTTP client configured"))?;

        // Download the object.
        let data = self.tokio_rt.block_on(async {
            client.download_object(sha).await
        })?;

        // Write to loose object store so git can find it.
        let objects_dir = working_dir.join(".git").join("objects");
        let prefix = &sha[..2];
        let suffix = &sha[2..];
        let obj_dir = objects_dir.join(prefix);
        std::fs::create_dir_all(&obj_dir)?;
        let obj_path = obj_dir.join(suffix);

        if !obj_path.exists() {
            std::fs::write(&obj_path, &data)?;
        }

        // Now cat-file should work.
        self.read_git_object(sha, working_dir)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Start / Stop
    // ─────────────────────────────────────────────────────────────────────

    /// Start the ProjFS virtualizer. Returns the virtualization instance.
    /// The virtualizer must be kept alive (pinned) for the duration.
    pub fn start(
        self: Arc<Self>,
        root_path: &Path,
    ) -> anyhow::Result<projfs::VirtualizationInstance> {
        let callbacks = PRJ_CALLBACKS {
            StartDirectoryEnumerationCallback: Some(callbacks::start_dir_enum_cb),
            EndDirectoryEnumerationCallback: Some(callbacks::end_dir_enum_cb),
            GetDirectoryEnumerationCallback: Some(callbacks::get_dir_enum_cb),
            GetPlaceholderInfoCallback: Some(callbacks::get_placeholder_info_cb),
            GetFileDataCallback: Some(callbacks::get_file_data_cb),
            QueryFileNameCallback: Some(callbacks::query_file_name_cb),
            NotificationCallback: Some(callbacks::notification_cb),
            CancelCommandCallback: Some(callbacks::cancel_command_cb),
        };

        // Build notification mappings.
        let empty_root = U16CString::from_str("").unwrap();
        let notification_mappings = vec![PRJ_NOTIFICATION_MAPPING {
            NotificationBitMask: PRJ_NOTIFY_ALL,
            _pad0: 0,
            NotificationRoot: PCWSTR(empty_root.as_ptr()),
        }];

        // SAFETY: We pass `self` (Arc<GvfsVirtualizer>) as the instance context.
        // The Arc is leaked here (preventing drop) and will be reclaimed on stop.
        let self_ptr = Arc::into_raw(self.clone()) as *const c_void;

        let instance = unsafe {
            projfs::start_virtualizing(
                root_path,
                callbacks,
                self_ptr,
                &notification_mappings,
            )?
        };

        info!("ProjFS virtualizer started at {:?}", root_path);
        Ok(instance)
    }
}
