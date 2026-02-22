//! Raw FFI bindings to ProjectedFSLib.dll.
//!
//! Struct layouts verified against Windows SDK 10.0.26100.0 using sizeof/offsetof.
//! All structs use `#[repr(C)]` to match the exact memory layout expected by ProjFS.

use std::ffi::c_void;
use windows::core::{HRESULT, PCWSTR};
use windows_core::GUID;

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Alignment size for write buffers used with `PrjWriteFileData`.
pub const PRJ_PLACEHOLDER_ID_LENGTH: usize = 128;

/// Notification types (bitmask).
pub const PRJ_NOTIFY_NONE: u32 = 0x0000_0000;
pub const PRJ_NOTIFY_SUPPRESS_NOTIFICATIONS: u32 = 0x0000_0001;
pub const PRJ_NOTIFY_FILE_OPENED: u32 = 0x0000_0002;
pub const PRJ_NOTIFY_NEW_FILE_CREATED: u32 = 0x0000_0004;
pub const PRJ_NOTIFY_FILE_OVERWRITTEN: u32 = 0x0000_0008;
pub const PRJ_NOTIFY_PRE_DELETE: u32 = 0x0000_0010;
pub const PRJ_NOTIFY_PRE_RENAME: u32 = 0x0000_0020;
pub const PRJ_NOTIFY_PRE_SET_HARDLINK: u32 = 0x0000_0040;
pub const PRJ_NOTIFY_FILE_RENAMED: u32 = 0x0000_0080;
pub const PRJ_NOTIFY_HARDLINK_CREATED: u32 = 0x0000_0100;
pub const PRJ_NOTIFY_FILE_HANDLE_CLOSED_NO_MODIFICATION: u32 = 0x0000_0200;
pub const PRJ_NOTIFY_FILE_HANDLE_CLOSED_FILE_MODIFIED: u32 = 0x0000_0400;
pub const PRJ_NOTIFY_FILE_HANDLE_CLOSED_FILE_DELETED: u32 = 0x0000_0800;
pub const PRJ_NOTIFY_FILE_PRE_CONVERT_TO_FULL: u32 = 0x0000_1000;
pub const PRJ_NOTIFY_USE_EXISTING_MASK: u32 = 0xFFFF_FFFF;

/// Standard notification sets for VFSForGit.
pub const PRJ_NOTIFY_ALL: u32 = PRJ_NOTIFY_NEW_FILE_CREATED
    | PRJ_NOTIFY_FILE_OVERWRITTEN
    | PRJ_NOTIFY_PRE_DELETE
    | PRJ_NOTIFY_PRE_RENAME
    | PRJ_NOTIFY_PRE_SET_HARDLINK
    | PRJ_NOTIFY_FILE_RENAMED
    | PRJ_NOTIFY_HARDLINK_CREATED
    | PRJ_NOTIFY_FILE_HANDLE_CLOSED_FILE_MODIFIED
    | PRJ_NOTIFY_FILE_HANDLE_CLOSED_FILE_DELETED
    | PRJ_NOTIFY_FILE_PRE_CONVERT_TO_FULL;

/// ProjFS start flags.
pub const PRJ_FLAG_NONE: u32 = 0x0000_0000;
pub const PRJ_FLAG_USE_NEGATIVE_PATH_CACHE: u32 = 0x0000_0001;

/// Placeholder version byte stored in providerId[0].
pub const PLACEHOLDER_VERSION: u8 = 1;

/// Notification types used in callback parameters.
pub const PRJ_NOTIFICATION_FILE_OPENED: u8 = 0x02;
pub const PRJ_NOTIFICATION_NEW_FILE_CREATED: u8 = 0x04;
pub const PRJ_NOTIFICATION_FILE_OVERWRITTEN: u8 = 0x08;
pub const PRJ_NOTIFICATION_PRE_DELETE: u8 = 0x10;
pub const PRJ_NOTIFICATION_PRE_RENAME: u8 = 0x20;
pub const PRJ_NOTIFICATION_FILE_RENAMED: u8 = 0x80;
pub const PRJ_NOTIFICATION_FILE_HANDLE_CLOSED_FILE_MODIFIED: u8 = 0x02; // in the closed group
pub const PRJ_NOTIFICATION_FILE_PRE_CONVERT_TO_FULL: u8 = 0x10; // in the convert group

// ─────────────────────────────────────────────────────────────────────────────
// Opaque handle types
// ─────────────────────────────────────────────────────────────────────────────

/// Opaque handle for a ProjFS virtualization namespace.
pub type PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT = *mut c_void;

/// Opaque handle for a directory entry buffer.
pub type PRJ_DIR_ENTRY_BUFFER_HANDLE = *mut c_void;

// ─────────────────────────────────────────────────────────────────────────────
// Structs — exact layouts verified against Windows SDK 10.0.26100.0
// ─────────────────────────────────────────────────────────────────────────────

/// 56 bytes. File attributes & timestamps for a placeholder.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PRJ_FILE_BASIC_INFO {
    /// BOOLEAN (u8) at offset 0, 1 byte + 7 padding.
    pub IsDirectory: u8,
    pub _pad0: [u8; 7],
    /// INT64 at offset 8.
    pub FileSize: i64,
    /// LARGE_INTEGER at offset 16.
    pub CreationTime: i64,
    /// LARGE_INTEGER at offset 24.
    pub LastAccessTime: i64,
    /// LARGE_INTEGER at offset 32.
    pub LastWriteTime: i64,
    /// LARGE_INTEGER at offset 40.
    pub ChangeTime: i64,
    /// UINT32 at offset 48, 4 bytes + 4 padding.
    pub FileAttributes: u32,
    pub _pad1: u32,
}
const _: () = assert!(std::mem::size_of::<PRJ_FILE_BASIC_INFO>() == 56);

/// 256 bytes. Version info with provider-controlled content.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PRJ_PLACEHOLDER_VERSION_INFO {
    pub ProviderID: [u8; PRJ_PLACEHOLDER_ID_LENGTH],
    pub ContentID: [u8; PRJ_PLACEHOLDER_ID_LENGTH],
}
const _: () = assert!(std::mem::size_of::<PRJ_PLACEHOLDER_VERSION_INFO>() == 256);

impl Default for PRJ_PLACEHOLDER_VERSION_INFO {
    fn default() -> Self {
        Self {
            ProviderID: [0u8; PRJ_PLACEHOLDER_ID_LENGTH],
            ContentID: [0u8; PRJ_PLACEHOLDER_ID_LENGTH],
        }
    }
}

impl std::fmt::Debug for PRJ_PLACEHOLDER_VERSION_INFO {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PRJ_PLACEHOLDER_VERSION_INFO")
            .field("ProviderID[0]", &self.ProviderID[0])
            .field("ContentID[..8]", &&self.ContentID[..8])
            .finish()
    }
}

/// 344 bytes (NOT 336!). Full placeholder info including VariableData.
/// The VariableData[1] field at the end is critical — PrjWritePlaceholderInfo
/// returns ERROR_INSUFFICIENT_BUFFER without it.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PRJ_PLACEHOLDER_INFO {
    pub FileBasicInfo: PRJ_FILE_BASIC_INFO,
    pub EaBufferSize: u32,
    pub OffsetToFirstEa: u32,
    pub SecurityBufferSize: u32,
    pub OffsetToSecurityDescriptor: u32,
    pub StreamsInfoBufferSize: u32,
    pub OffsetToFirstStreamInfo: u32,
    pub VersionInfo: PRJ_PLACEHOLDER_VERSION_INFO,
    /// VariableData[1] at offset 336, 1 byte + 7 padding = 344 total.
    pub VariableData: [u8; 1],
    pub _pad_variable: [u8; 7],
}
const _: () = assert!(std::mem::size_of::<PRJ_PLACEHOLDER_INFO>() == 344);

impl Default for PRJ_PLACEHOLDER_INFO {
    fn default() -> Self {
        Self {
            FileBasicInfo: PRJ_FILE_BASIC_INFO::default(),
            EaBufferSize: 0,
            OffsetToFirstEa: 0,
            SecurityBufferSize: 0,
            OffsetToSecurityDescriptor: 0,
            StreamsInfoBufferSize: 0,
            OffsetToFirstStreamInfo: 0,
            VersionInfo: PRJ_PLACEHOLDER_VERSION_INFO::default(),
            VariableData: [0],
            _pad_variable: [0; 7],
        }
    }
}

/// 16 bytes. Extended info for symlinks etc.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PRJ_EXTENDED_INFO {
    pub InfoType: u32,
    pub NextInfoOffset: u32,
    pub TargetName: PCWSTR,
}
const _: () = assert!(std::mem::size_of::<PRJ_EXTENDED_INFO>() == 16);

impl Default for PRJ_EXTENDED_INFO {
    fn default() -> Self {
        Self {
            InfoType: 0,
            NextInfoOffset: 0,
            TargetName: PCWSTR::null(),
        }
    }
}

/// 96 bytes. Passed to every callback.
#[repr(C)]
#[derive(Debug)]
pub struct PRJ_CALLBACK_DATA {
    pub Size: u32,
    pub Flags: u32,
    pub NamespaceVirtualizationContext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    pub CommandId: i32,
    pub FileId: GUID,
    pub DataStreamId: GUID,
    pub FilePathName: PCWSTR,
    pub VersionInfo: *const PRJ_PLACEHOLDER_VERSION_INFO,
    pub TriggeringProcessId: u32,
    pub _pad0: u32,
    pub TriggeringProcessImageFileName: PCWSTR,
    pub InstanceContext: *mut c_void,
}
// Note: The handoff document says 96 bytes but the exact size depends on
// pointer widths. On x64 this is 96 bytes.

/// 64 bytes. Set of callback function pointers (8 pointers × 8 bytes each).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PRJ_CALLBACKS {
    pub StartDirectoryEnumerationCallback: Option<StartDirEnumCb>,
    pub EndDirectoryEnumerationCallback: Option<EndDirEnumCb>,
    pub GetDirectoryEnumerationCallback: Option<GetDirEnumCb>,
    pub GetPlaceholderInfoCallback: Option<GetPlaceholderInfoCb>,
    pub GetFileDataCallback: Option<GetFileDataCb>,
    pub QueryFileNameCallback: Option<QueryFileNameCb>,
    pub NotificationCallback: Option<NotificationCb>,
    pub CancelCommandCallback: Option<CancelCommandCb>,
}
const _: () = assert!(std::mem::size_of::<PRJ_CALLBACKS>() == 64);

impl Default for PRJ_CALLBACKS {
    fn default() -> Self {
        Self {
            StartDirectoryEnumerationCallback: None,
            EndDirectoryEnumerationCallback: None,
            GetDirectoryEnumerationCallback: None,
            GetPlaceholderInfoCallback: None,
            GetFileDataCallback: None,
            QueryFileNameCallback: None,
            NotificationCallback: None,
            CancelCommandCallback: None,
        }
    }
}

/// 16 bytes. Maps notification bitmask to a root path.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PRJ_NOTIFICATION_MAPPING {
    pub NotificationBitMask: u32,
    pub _pad0: u32,
    pub NotificationRoot: PCWSTR,
}
const _: () = assert!(std::mem::size_of::<PRJ_NOTIFICATION_MAPPING>() == 16);

/// 32 bytes. Options for starting virtualization.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PRJ_STARTVIRTUALIZING_OPTIONS {
    pub Flags: u32,
    pub PoolThreadCount: u32,
    pub ConcurrentThreadCount: u32,
    pub _pad0: u32,
    pub NotificationMappings: *const PRJ_NOTIFICATION_MAPPING,
    pub NotificationMappingsCount: u32,
    pub _pad1: u32,
}
const _: () = assert!(std::mem::size_of::<PRJ_STARTVIRTUALIZING_OPTIONS>() == 32);

impl Default for PRJ_STARTVIRTUALIZING_OPTIONS {
    fn default() -> Self {
        Self {
            Flags: 0,
            PoolThreadCount: 0,
            ConcurrentThreadCount: 0,
            _pad0: 0,
            NotificationMappings: std::ptr::null(),
            NotificationMappingsCount: 0,
            _pad1: 0,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback type aliases (all `extern "system"` = __stdcall on x64)
// ─────────────────────────────────────────────────────────────────────────────

pub type StartDirEnumCb =
    unsafe extern "system" fn(callbackData: *const PRJ_CALLBACK_DATA, enumerationId: *const GUID) -> HRESULT;

pub type EndDirEnumCb =
    unsafe extern "system" fn(callbackData: *const PRJ_CALLBACK_DATA, enumerationId: *const GUID) -> HRESULT;

pub type GetDirEnumCb = unsafe extern "system" fn(
    callbackData: *const PRJ_CALLBACK_DATA,
    enumerationId: *const GUID,
    searchExpression: PCWSTR,
    dirEntryBufferHandle: PRJ_DIR_ENTRY_BUFFER_HANDLE,
) -> HRESULT;

pub type GetPlaceholderInfoCb =
    unsafe extern "system" fn(callbackData: *const PRJ_CALLBACK_DATA) -> HRESULT;

pub type GetFileDataCb = unsafe extern "system" fn(
    callbackData: *const PRJ_CALLBACK_DATA,
    byteOffset: u64,
    length: u32,
) -> HRESULT;

pub type QueryFileNameCb =
    unsafe extern "system" fn(callbackData: *const PRJ_CALLBACK_DATA) -> HRESULT;

pub type NotificationCb = unsafe extern "system" fn(
    callbackData: *const PRJ_CALLBACK_DATA,
    isDirectory: u8,
    notification: u32,
    destinationFileName: PCWSTR,
    operationParameters: *mut PRJ_NOTIFICATION_PARAMETERS,
) -> HRESULT;

pub type CancelCommandCb =
    unsafe extern "system" fn(callbackData: *const PRJ_CALLBACK_DATA);

/// Union for notification parameters. We use a simple byte array since we
/// mainly just pass this through.
#[repr(C)]
#[derive(Clone, Copy)]
pub union PRJ_NOTIFICATION_PARAMETERS {
    pub PostCreate: PRJ_NOTIFY_TYPES,
    pub FileRenamed: PRJ_NOTIFY_TYPES,
    pub FileDeletedOnHandleClose: PRJ_NOTIFY_TYPES,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PRJ_NOTIFY_TYPES {
    pub NotificationMask: u32,
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI function declarations — linked from ProjectedFSLib.dll at runtime
// ─────────────────────────────────────────────────────────────────────────────

#[link(name = "ProjectedFSLib")]
extern "system" {
    pub fn PrjStartVirtualizing(
        virtualizationRootPath: PCWSTR,
        callbacks: *const PRJ_CALLBACKS,
        instanceContext: *const c_void,
        options: *const PRJ_STARTVIRTUALIZING_OPTIONS,
        namespaceVirtualizationContext: *mut PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    ) -> HRESULT;

    pub fn PrjStopVirtualizing(
        namespaceVirtualizationContext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    );

    pub fn PrjWritePlaceholderInfo(
        namespaceVirtualizationContext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
        destinationFileName: PCWSTR,
        placeholderInfo: *const PRJ_PLACEHOLDER_INFO,
        placeholderInfoSize: u32,
    ) -> HRESULT;

    pub fn PrjWritePlaceholderInfo2(
        namespaceVirtualizationContext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
        destinationFileName: PCWSTR,
        placeholderInfo: *const PRJ_PLACEHOLDER_INFO,
        placeholderInfoSize: u32,
        extendedInfo: *const PRJ_EXTENDED_INFO,
    ) -> HRESULT;

    pub fn PrjWriteFileData(
        namespaceVirtualizationContext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
        dataStreamId: *const GUID,
        buffer: *const c_void,
        byteOffset: u64,
        length: u32,
    ) -> HRESULT;

    pub fn PrjAllocateAlignedBuffer(
        namespaceVirtualizationContext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
        size: usize,
    ) -> *mut c_void;

    pub fn PrjFreeAlignedBuffer(buffer: *mut c_void);

    pub fn PrjFillDirEntryBuffer(
        fileName: PCWSTR,
        fileBasicInfo: *const PRJ_FILE_BASIC_INFO,
        dirEntryBufferHandle: PRJ_DIR_ENTRY_BUFFER_HANDLE,
    ) -> HRESULT;

    pub fn PrjFillDirEntryBuffer2(
        dirEntryBufferHandle: PRJ_DIR_ENTRY_BUFFER_HANDLE,
        fileName: PCWSTR,
        fileBasicInfo: *const PRJ_FILE_BASIC_INFO,
        extendedInfo: *const PRJ_EXTENDED_INFO,
    ) -> HRESULT;

    pub fn PrjFileNameMatch(fileNameToCheck: PCWSTR, pattern: PCWSTR) -> u8;

    pub fn PrjFileNameCompare(fileName1: PCWSTR, fileName2: PCWSTR) -> i32;

    pub fn PrjMarkDirectoryAsPlaceholder(
        rootPathName: PCWSTR,
        targetPathName: PCWSTR,
        versionInfo: *const PRJ_PLACEHOLDER_VERSION_INFO,
        virtualizationInstanceID: *const GUID,
    ) -> HRESULT;

    pub fn PrjClearNegativePathCache(
        namespaceVirtualizationContext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
        totalEntryNumber: *mut u32,
    ) -> HRESULT;

    pub fn PrjCompleteCommand(
        namespaceVirtualizationContext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
        commandId: i32,
        completionResult: HRESULT,
        extendedParameters: *const c_void,
    ) -> HRESULT;

    pub fn PrjGetVirtualizationInstanceInfo(
        namespaceVirtualizationContext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
        virtualizationInstanceInfo: *mut PRJ_VIRTUALIZATION_INSTANCE_INFO,
    ) -> HRESULT;
}

/// Info about a running virtualization instance.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PRJ_VIRTUALIZATION_INSTANCE_INFO {
    pub InstanceID: GUID,
    pub WriteAlignment: u32,
}
