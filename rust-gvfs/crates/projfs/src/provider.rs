//! Safe Rust wrappers around ProjFS operations.
//!
//! Provides a `VirtualizationInstance` that manages the lifetime of a ProjFS
//! namespace and safe helpers for writing placeholders, file data, and
//! directory entries.

use std::ffi::c_void;
use std::path::Path;

use tracing::{debug, error};
use widestring::U16CString;
use windows::core::{HRESULT, PCWSTR};
use windows_core::GUID;

use crate::ffi::*;

/// Errors from ProjFS operations.
#[derive(Debug, thiserror::Error)]
pub enum ProjFsError {
    #[error("ProjFS HRESULT error: 0x{0:08X}")]
    HResult(u32),
    #[error("String conversion error: {0}")]
    StringConversion(String),
    #[error("ProjFS not started")]
    NotStarted,
}

impl From<HRESULT> for ProjFsError {
    fn from(hr: HRESULT) -> Self {
        ProjFsError::HResult(hr.0 as u32)
    }
}

/// Trait that ProjFS callback implementations must satisfy.
/// Each method corresponds to a ProjFS callback.
pub trait ProjFsCallbacks: Send + Sync + 'static {
    fn start_dir_enum(&self, file_path: &str, enumeration_id: GUID) -> HRESULT;
    fn end_dir_enum(&self, file_path: &str, enumeration_id: GUID) -> HRESULT;
    fn get_dir_enum(
        &self,
        file_path: &str,
        enumeration_id: GUID,
        search_expression: Option<&str>,
        dir_entry_buffer: PRJ_DIR_ENTRY_BUFFER_HANDLE,
    ) -> HRESULT;
    fn get_placeholder_info(&self, file_path: &str) -> HRESULT;
    fn get_file_data(&self, file_path: &str, data_stream_id: GUID, byte_offset: u64, length: u32) -> HRESULT;
    fn notify(
        &self,
        file_path: &str,
        is_directory: bool,
        notification: u32,
        dest_file_name: Option<&str>,
    ) -> HRESULT;
}

/// A running ProjFS virtualization instance.
pub struct VirtualizationInstance {
    context: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    _callbacks: Box<PRJ_CALLBACKS>,
    _root: U16CString,
}

// SAFETY: ProjFS context handles are thread-safe — ProjFS internally
// synchronizes access.
unsafe impl Send for VirtualizationInstance {}
unsafe impl Sync for VirtualizationInstance {}

impl VirtualizationInstance {
    /// The raw ProjFS namespace context. Needed for write operations.
    pub fn context(&self) -> PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT {
        self.context
    }

    /// Stop and clean up the virtualization instance.
    pub fn stop(self) {
        // Drop will handle it
    }
}

impl Drop for VirtualizationInstance {
    fn drop(&mut self) {
        if !self.context.is_null() {
            debug!("Stopping ProjFS virtualization instance");
            unsafe { PrjStopVirtualizing(self.context) };
        }
    }
}

/// Start a ProjFS virtualization instance.
///
/// # Safety
/// `callbacks` must be a valid `PRJ_CALLBACKS` struct whose function pointers
/// remain valid for the lifetime of the returned `VirtualizationInstance`.
/// `instance_context` must remain valid for that same lifetime.
pub unsafe fn start_virtualizing(
    root_path: &Path,
    callbacks: PRJ_CALLBACKS,
    instance_context: *const c_void,
    notification_mappings: &[PRJ_NOTIFICATION_MAPPING],
) -> Result<VirtualizationInstance, ProjFsError> {
    let root_wide = U16CString::from_os_str(root_path.as_os_str())
        .map_err(|e| ProjFsError::StringConversion(e.to_string()))?;

    let callbacks_box = Box::new(callbacks);

    let options = PRJ_STARTVIRTUALIZING_OPTIONS {
        Flags: PRJ_FLAG_USE_NEGATIVE_PATH_CACHE,
        PoolThreadCount: 0,
        ConcurrentThreadCount: 0,
        _pad0: 0,
        NotificationMappings: if notification_mappings.is_empty() {
            std::ptr::null()
        } else {
            notification_mappings.as_ptr()
        },
        NotificationMappingsCount: notification_mappings.len() as u32,
        _pad1: 0,
    };

    let mut ctx: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT = std::ptr::null_mut();

    let hr = unsafe {
        PrjStartVirtualizing(
            PCWSTR(root_wide.as_ptr()),
            &*callbacks_box as *const PRJ_CALLBACKS,
            instance_context,
            &options as *const PRJ_STARTVIRTUALIZING_OPTIONS,
            &mut ctx,
        )
    };

    if hr.is_err() {
        error!("PrjStartVirtualizing failed: 0x{:08X}", hr.0);
        return Err(ProjFsError::HResult(hr.0 as u32));
    }

    debug!("ProjFS virtualization started at {:?}", root_path);
    Ok(VirtualizationInstance {
        context: ctx,
        _callbacks: callbacks_box,
        _root: root_wide,
    })
}

/// Write placeholder information for a file or directory.
pub fn write_placeholder_info(
    ctx: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    relative_path: &str,
    placeholder: &PRJ_PLACEHOLDER_INFO,
) -> Result<(), ProjFsError> {
    let path_wide = U16CString::from_str(relative_path)
        .map_err(|e| ProjFsError::StringConversion(e.to_string()))?;

    let hr = unsafe {
        PrjWritePlaceholderInfo(
            ctx,
            PCWSTR(path_wide.as_ptr()),
            placeholder as *const PRJ_PLACEHOLDER_INFO,
            std::mem::size_of::<PRJ_PLACEHOLDER_INFO>() as u32,
        )
    };

    if hr.is_err() {
        return Err(ProjFsError::HResult(hr.0 as u32));
    }
    Ok(())
}

/// Write file data for a hydration request.
pub fn write_file_data(
    ctx: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    data_stream_id: &GUID,
    data: &[u8],
    byte_offset: u64,
) -> Result<(), ProjFsError> {
    // Allocate aligned buffer
    let aligned_buf = unsafe { PrjAllocateAlignedBuffer(ctx, data.len()) };
    if aligned_buf.is_null() {
        return Err(ProjFsError::HResult(0x8007000E)); // E_OUTOFMEMORY
    }

    // Copy data into aligned buffer
    unsafe {
        std::ptr::copy_nonoverlapping(data.as_ptr(), aligned_buf as *mut u8, data.len());
    }

    let hr = unsafe {
        PrjWriteFileData(
            ctx,
            data_stream_id as *const GUID,
            aligned_buf,
            byte_offset,
            data.len() as u32,
        )
    };

    unsafe { PrjFreeAlignedBuffer(aligned_buf) };

    if hr.is_err() {
        return Err(ProjFsError::HResult(hr.0 as u32));
    }
    Ok(())
}

/// Fill a directory entry buffer with a single entry.
pub fn fill_dir_entry(
    buffer_handle: PRJ_DIR_ENTRY_BUFFER_HANDLE,
    file_name: &str,
    basic_info: &PRJ_FILE_BASIC_INFO,
) -> Result<(), ProjFsError> {
    let name_wide = U16CString::from_str(file_name)
        .map_err(|e| ProjFsError::StringConversion(e.to_string()))?;

    let hr = unsafe {
        PrjFillDirEntryBuffer(
            PCWSTR(name_wide.as_ptr()),
            basic_info as *const PRJ_FILE_BASIC_INFO,
            buffer_handle,
        )
    };

    if hr.is_err() {
        // ERROR_INSUFFICIENT_BUFFER is expected when buffer is full
        if hr.0 as u32 == 0x8007007A {
            return Err(ProjFsError::HResult(hr.0 as u32));
        }
        return Err(ProjFsError::HResult(hr.0 as u32));
    }
    Ok(())
}

/// Check if a file name matches a ProjFS search pattern.
pub fn file_name_match(file_name: &str, pattern: &str) -> bool {
    let name_wide = match U16CString::from_str(file_name) {
        Ok(w) => w,
        Err(_) => return false,
    };
    let pat_wide = match U16CString::from_str(pattern) {
        Ok(w) => w,
        Err(_) => return false,
    };
    unsafe { PrjFileNameMatch(PCWSTR(name_wide.as_ptr()), PCWSTR(pat_wide.as_ptr())) != 0 }
}

/// Compare two file names in ProjFS sort order.
pub fn file_name_compare(a: &str, b: &str) -> std::cmp::Ordering {
    let a_wide = U16CString::from_str(a).unwrap_or_default();
    let b_wide = U16CString::from_str(b).unwrap_or_default();
    let result = unsafe { PrjFileNameCompare(PCWSTR(a_wide.as_ptr()), PCWSTR(b_wide.as_ptr())) };
    result.cmp(&0)
}

/// Mark a directory as a ProjFS placeholder (used during initial setup).
pub fn mark_directory_as_placeholder(
    root_path: &Path,
    target_path: &str,
    instance_id: &GUID,
) -> Result<(), ProjFsError> {
    let root_wide = U16CString::from_os_str(root_path.as_os_str())
        .map_err(|e| ProjFsError::StringConversion(e.to_string()))?;
    let target_wide = U16CString::from_str(target_path)
        .map_err(|e| ProjFsError::StringConversion(e.to_string()))?;

    let hr = unsafe {
        PrjMarkDirectoryAsPlaceholder(
            PCWSTR(root_wide.as_ptr()),
            PCWSTR(target_wide.as_ptr()),
            std::ptr::null(),
            instance_id as *const GUID,
        )
    };

    if hr.is_err() {
        return Err(ProjFsError::HResult(hr.0 as u32));
    }
    Ok(())
}

/// Build placeholder version info with the standard GVFS format.
/// `provider_id[0]` = PLACEHOLDER_VERSION, `content_id` = UTF-16LE of SHA hex string.
pub fn build_version_info(sha: &str) -> PRJ_PLACEHOLDER_VERSION_INFO {
    let mut info = PRJ_PLACEHOLDER_VERSION_INFO::default();
    info.ProviderID[0] = PLACEHOLDER_VERSION;

    // Encode SHA as UTF-16LE into ContentID (128 bytes = 64 UTF-16 chars max).
    let sha_utf16: Vec<u16> = sha.encode_utf16().collect();
    let byte_len = (sha_utf16.len() * 2).min(PRJ_PLACEHOLDER_ID_LENGTH);
    unsafe {
        std::ptr::copy_nonoverlapping(
            sha_utf16.as_ptr() as *const u8,
            info.ContentID.as_mut_ptr(),
            byte_len,
        );
    }
    info
}

/// Build a PRJ_PLACEHOLDER_INFO for a file.
pub fn build_file_placeholder(sha: &str, file_size: i64) -> PRJ_PLACEHOLDER_INFO {
    let mut info = PRJ_PLACEHOLDER_INFO::default();
    info.FileBasicInfo.IsDirectory = 0;
    info.FileBasicInfo.FileSize = file_size;
    info.VersionInfo = build_version_info(sha);
    info
}

/// Build a PRJ_PLACEHOLDER_INFO for a directory.
pub fn build_dir_placeholder() -> PRJ_PLACEHOLDER_INFO {
    let mut info = PRJ_PLACEHOLDER_INFO::default();
    info.FileBasicInfo.IsDirectory = 1;
    info.FileBasicInfo.FileSize = 0;
    info.FileBasicInfo.FileAttributes = 0x10; // FILE_ATTRIBUTE_DIRECTORY
    // Use zero SHA for folders
    let zero_sha = "0000000000000000000000000000000000000000";
    info.VersionInfo = build_version_info(zero_sha);
    info
}
