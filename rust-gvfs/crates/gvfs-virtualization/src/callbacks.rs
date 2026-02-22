//! ProjFS callback trampolines — unsafe extern "system" functions that bridge
//! ProjFS callbacks to our safe Rust implementation in `GvfsVirtualizer`.


use tracing::{debug, trace};
use widestring::U16CStr;
use windows::core::{HRESULT, PCWSTR};
use windows_core::GUID;

use projfs::ffi::*;

use crate::virtualizer::GvfsVirtualizer;

/// Convert a PCWSTR to a Rust String.
/// Returns empty string for null pointers.
pub(crate) unsafe fn pcwstr_to_string(p: PCWSTR) -> String {
    if p.0.is_null() {
        return String::new();
    }
    let wstr = U16CStr::from_ptr_str(p.0);
    wstr.to_string_lossy()
}

/// Get the virtualizer from the instance context.
unsafe fn get_virtualizer<'a>(callback_data: *const PRJ_CALLBACK_DATA) -> &'a GvfsVirtualizer {
    let ctx = (*callback_data).InstanceContext;
    &*(ctx as *const GvfsVirtualizer)
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trampolines
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) unsafe extern "system" fn start_dir_enum_cb(
    callback_data: *const PRJ_CALLBACK_DATA,
    enumeration_id: *const GUID,
) -> HRESULT {
    let virt = get_virtualizer(callback_data);
    let path = pcwstr_to_string((*callback_data).FilePathName);
    let enum_id = *enumeration_id;
    trace!("StartDirEnum: {:?}", path);
    virt.start_dir_enum(&path, enum_id)
}

pub(crate) unsafe extern "system" fn end_dir_enum_cb(
    callback_data: *const PRJ_CALLBACK_DATA,
    enumeration_id: *const GUID,
) -> HRESULT {
    let virt = get_virtualizer(callback_data);
    let path = pcwstr_to_string((*callback_data).FilePathName);
    let enum_id = *enumeration_id;
    trace!("EndDirEnum: {:?}", path);
    virt.end_dir_enum(&path, enum_id)
}

pub(crate) unsafe extern "system" fn get_dir_enum_cb(
    callback_data: *const PRJ_CALLBACK_DATA,
    enumeration_id: *const GUID,
    search_expression: PCWSTR,
    dir_entry_buffer: PRJ_DIR_ENTRY_BUFFER_HANDLE,
) -> HRESULT {
    let virt = get_virtualizer(callback_data);
    let path = pcwstr_to_string((*callback_data).FilePathName);
    let enum_id = *enumeration_id;
    let search = if search_expression.0.is_null() {
        None
    } else {
        Some(pcwstr_to_string(search_expression))
    };
    trace!("GetDirEnum: {:?}, search: {:?}", path, search);
    virt.get_dir_enum(&path, enum_id, search.as_deref(), dir_entry_buffer)
}

pub(crate) unsafe extern "system" fn get_placeholder_info_cb(
    callback_data: *const PRJ_CALLBACK_DATA,
) -> HRESULT {
    let virt = get_virtualizer(callback_data);
    let path = pcwstr_to_string((*callback_data).FilePathName);
    let ctx = (*callback_data).NamespaceVirtualizationContext;
    trace!("GetPlaceholderInfo: {:?}", path);
    virt.get_placeholder_info(&path, ctx)
}

pub(crate) unsafe extern "system" fn get_file_data_cb(
    callback_data: *const PRJ_CALLBACK_DATA,
    byte_offset: u64,
    length: u32,
) -> HRESULT {
    let virt = get_virtualizer(callback_data);
    let path = pcwstr_to_string((*callback_data).FilePathName);
    let ctx = (*callback_data).NamespaceVirtualizationContext;
    let stream_id = (*callback_data).DataStreamId;
    debug!("GetFileData: {:?} offset={} length={}", path, byte_offset, length);
    virt.get_file_data(&path, ctx, stream_id, byte_offset, length)
}

pub(crate) unsafe extern "system" fn query_file_name_cb(
    callback_data: *const PRJ_CALLBACK_DATA,
) -> HRESULT {
    let virt = get_virtualizer(callback_data);
    let path = pcwstr_to_string((*callback_data).FilePathName);
    trace!("QueryFileName: {:?}", path);
    virt.query_file_name(&path)
}

pub(crate) unsafe extern "system" fn notification_cb(
    callback_data: *const PRJ_CALLBACK_DATA,
    is_directory: u8,
    notification: u32,
    destination_file_name: PCWSTR,
    _operation_parameters: *mut PRJ_NOTIFICATION_PARAMETERS,
) -> HRESULT {
    let virt = get_virtualizer(callback_data);
    let path = pcwstr_to_string((*callback_data).FilePathName);
    let dest = if destination_file_name.0.is_null() {
        None
    } else {
        Some(pcwstr_to_string(destination_file_name))
    };
    trace!("Notification: {:?} type={} dir={}", path, notification, is_directory);
    virt.notification(&path, is_directory != 0, notification, dest.as_deref())
}

pub(crate) unsafe extern "system" fn cancel_command_cb(
    callback_data: *const PRJ_CALLBACK_DATA,
) {
    let command_id = (*callback_data).CommandId;
    debug!("CancelCommand: {}", command_id);
    // Not much to do — we handle everything synchronously.
}
