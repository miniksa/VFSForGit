//! Named pipe client/server and message protocol.
//!
//! Wire format: UTF-8 text, `Header|Body`, terminated by `\x03` (ETX byte).
//! The pipe name is derived from the enlistment root path.

use std::io::{self, Read, Write};
use std::path::Path;

use sha1::{Digest, Sha1};
use tracing::{debug, trace, warn};

use crate::constants::{ETX, PIPE_NAME_PREFIX, PIPE_SEPARATOR};

// ─────────────────────────────────────────────────────────────────────────────
// Message types
// ─────────────────────────────────────────────────────────────────────────────

/// A named pipe message with header and optional body.
#[derive(Debug, Clone)]
pub struct PipeMessage {
    pub header: String,
    pub body: Option<String>,
}

impl PipeMessage {
    pub fn new(header: impl Into<String>, body: Option<String>) -> Self {
        Self {
            header: header.into(),
            body,
        }
    }

    /// Parse a raw message string into header and body.
    pub fn from_str(raw: &str) -> Self {
        if let Some(sep_idx) = raw.find(PIPE_SEPARATOR) {
            let header = &raw[..sep_idx];
            let body = &raw[sep_idx + 1..];
            Self {
                header: header.to_string(),
                body: if body.is_empty() {
                    None
                } else {
                    Some(body.to_string())
                },
            }
        } else {
            Self {
                header: raw.to_string(),
                body: None,
            }
        }
    }

    /// Serialize to wire format (without ETX terminator).
    pub fn to_wire(&self) -> String {
        match &self.body {
            Some(b) => format!("{}{}{}", self.header, PIPE_SEPARATOR, b),
            None => self.header.clone(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Pipe name derivation
// ─────────────────────────────────────────────────────────────────────────────

/// Derive the named pipe name for an enlistment.
/// Uses SHA1 hash of the normalized enlistment root path.
pub fn get_pipe_name(enlistment_root: &Path) -> String {
    let normalized = enlistment_root
        .to_string_lossy()
        .to_uppercase()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_string();

    let mut hasher = Sha1::new();
    hasher.update(normalized.as_bytes());
    let hash = hex::encode(hasher.finalize());

    format!("{}{}", PIPE_NAME_PREFIX, hash)
}

/// Get the full Windows named pipe path.
pub fn get_pipe_path(enlistment_root: &Path) -> String {
    format!("\\\\.\\pipe\\{}", get_pipe_name(enlistment_root))
}

/// Get the service pipe name (fixed name).
pub fn get_service_pipe_name() -> String {
    "GVFS.Service".to_string()
}

pub fn get_service_pipe_path() -> String {
    "\\\\.\\pipe\\GVFS.Service".to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Named pipe client/server using std::fs::File over raw Win32 handles.
// ─────────────────────────────────────────────────────────────────────────────

use std::os::windows::io::FromRawHandle;

use widestring::U16CString;
use windows::core::PCWSTR;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_NONE, OPEN_EXISTING,
    FILE_GENERIC_READ, FILE_GENERIC_WRITE,
};
use windows::Win32::System::Pipes::{
    CreateNamedPipeW, ConnectNamedPipe, DisconnectNamedPipe,
    PIPE_TYPE_BYTE, PIPE_READMODE_BYTE, PIPE_WAIT,
};

// PIPE_ACCESS_DUPLEX is just the raw value 0x3, and PIPE_UNLIMITED_INSTANCES is 255.
const PIPE_ACCESS_DUPLEX_RAW: u32 = 0x0000_0003;
const PIPE_UNLIMITED_INSTANCES_RAW: u32 = 255;

/// A client connection to a named pipe.
pub struct PipeClient {
    handle: HANDLE,
}

impl PipeClient {
    /// Connect to a named pipe by path (e.g., `\\.\pipe\GVFS_<hash>`).
    pub fn connect(pipe_path: &str) -> io::Result<Self> {
        let pipe_wide = U16CString::from_str(pipe_path)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;

        let handle = unsafe {
            CreateFileW(
                PCWSTR(pipe_wide.as_ptr()),
                (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }
        .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, e.to_string()))?;

        Ok(Self { handle })
    }

    /// Connect to the mount daemon pipe for a given enlistment.
    pub fn connect_to_mount(enlistment_root: &Path) -> io::Result<Self> {
        let path = get_pipe_path(enlistment_root);
        debug!("Connecting to mount pipe: {}", path);
        Self::connect(&path)
    }

    /// Connect to the GVFS service pipe.
    pub fn connect_to_service() -> io::Result<Self> {
        let path = get_service_pipe_path();
        Self::connect(&path)
    }

    /// Send a message and read the response.
    pub fn send_receive(&self, msg: &PipeMessage) -> io::Result<PipeMessage> {
        self.send(msg)?;
        self.receive()
    }

    /// Send a message.
    pub fn send(&self, msg: &PipeMessage) -> io::Result<()> {
        let mut wire = msg.to_wire().into_bytes();
        wire.push(ETX);

        // Use a temporary File wrapper (does NOT close handle on drop because we don't own it).
        let mut file = unsafe {
            std::fs::File::from_raw_handle(self.handle.0 as *mut std::ffi::c_void)
        };
        let result = file.write_all(&wire);
        // Prevent File from closing the handle.
        std::mem::forget(file);
        result?;
        trace!("Sent {} bytes to pipe", wire.len());
        Ok(())
    }

    /// Receive a message (reads until ETX byte).
    pub fn receive(&self) -> io::Result<PipeMessage> {
        let mut buf = Vec::with_capacity(4096);
        let mut byte_buf = [0u8; 1];

        let mut file = unsafe {
            std::fs::File::from_raw_handle(self.handle.0 as *mut std::ffi::c_void)
        };

        loop {
            match file.read(&mut byte_buf) {
                Ok(0) => break,
                Ok(_) => {
                    if byte_buf[0] == ETX {
                        break;
                    }
                    buf.push(byte_buf[0]);
                }
                Err(e) => {
                    if buf.is_empty() {
                        std::mem::forget(file);
                        return Err(e);
                    }
                    break;
                }
            }
        }

        std::mem::forget(file);

        let raw = String::from_utf8(buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        trace!("Received from pipe: {}", raw);
        Ok(PipeMessage::from_str(&raw))
    }
}

impl Drop for PipeClient {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            let _ = unsafe { windows::Win32::Foundation::CloseHandle(self.handle) };
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Named pipe server
// ─────────────────────────────────────────────────────────────────────────────

/// A named pipe server that accepts connections and dispatches messages.
pub struct PipeServer {
    pipe_name: String,
    running: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl PipeServer {
    pub fn new(pipe_name: &str) -> Self {
        Self {
            pipe_name: format!("\\\\.\\pipe\\{}", pipe_name),
            running: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn for_enlistment(enlistment_root: &Path) -> Self {
        Self::new(&get_pipe_name(enlistment_root))
    }

    /// Start the server. Calls `handler` for each received message.
    /// Runs in a loop until `stop()` is called.
    pub fn run<F>(&self, handler: F) -> io::Result<()>
    where
        F: Fn(PipeMessage) -> PipeMessage + Send + Sync + 'static,
    {
        self.running
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let pipe_wide = U16CString::from_str(&self.pipe_name)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;

        while self.running.load(std::sync::atomic::Ordering::SeqCst) {
            // Create a new pipe instance for each connection.
            let pipe_handle = unsafe {
                CreateNamedPipeW(
                    PCWSTR(pipe_wide.as_ptr()),
                    windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(PIPE_ACCESS_DUPLEX_RAW),
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                    PIPE_UNLIMITED_INSTANCES_RAW,
                    4096,
                    4096,
                    0,
                    None,
                )
            };

            if pipe_handle.is_invalid() {
                return Err(io::Error::last_os_error());
            }

            // Wait for a client to connect.
            let connected = unsafe { ConnectNamedPipe(pipe_handle, None) };
            if connected.is_err() {
                let err = io::Error::last_os_error();
                if err.raw_os_error() != Some(535) {
                    warn!("ConnectNamedPipe error: {}", err);
                    let _ = unsafe { windows::Win32::Foundation::CloseHandle(pipe_handle) };
                    continue;
                }
            }

            // Read the message using std::fs::File.
            let mut buf = Vec::with_capacity(4096);
            {
                let mut file = unsafe {
                    std::fs::File::from_raw_handle(pipe_handle.0 as *mut std::ffi::c_void)
                };
                let mut byte_buf = [0u8; 512];
                loop {
                    match file.read(&mut byte_buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let chunk = &byte_buf[..n];
                            if let Some(etx_pos) = chunk.iter().position(|&b| b == ETX) {
                                buf.extend_from_slice(&chunk[..etx_pos]);
                                break;
                            }
                            buf.extend_from_slice(chunk);
                        }
                        Err(_) => break,
                    }
                }

                let raw = String::from_utf8_lossy(&buf).to_string();
                let request = PipeMessage::from_str(&raw);
                debug!("Pipe server received: {:?}", request.header);

                let response = handler(request);

                // Write response.
                let mut wire = response.to_wire().into_bytes();
                wire.push(ETX);
                let _ = file.write_all(&wire);

                // Prevent File from closing the handle.
                std::mem::forget(file);
            }

            let _ = unsafe { DisconnectNamedPipe(pipe_handle) };
            let _ = unsafe { windows::Win32::Foundation::CloseHandle(pipe_handle) };
        }

        Ok(())
    }

    /// Signal the server to stop accepting new connections.
    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Convenience functions for common pipe operations
// ─────────────────────────────────────────────────────────────────────────────

/// Send a GetStatus request and parse the response.
pub fn get_mount_status(enlistment_root: &Path) -> io::Result<String> {
    let client = PipeClient::connect_to_mount(enlistment_root)?;
    let response = client.send_receive(&PipeMessage::new("GetStatus", None))?;
    Ok(response.to_wire())
}

/// Send an Unmount request.
pub fn request_unmount(enlistment_root: &Path) -> io::Result<String> {
    let client = PipeClient::connect_to_mount(enlistment_root)?;
    let response = client.send_receive(&PipeMessage::new("Unmount", None))?;
    Ok(response.to_wire())
}

/// Send a DLO (Download Object) request.
pub fn download_object(enlistment_root: &Path, sha: &str) -> io::Result<bool> {
    let client = PipeClient::connect_to_mount(enlistment_root)?;
    let msg = PipeMessage::new("DLO", Some(sha.to_string()));
    let response = client.send_receive(&msg)?;
    Ok(response.header == "S")
}

/// Check if the mount daemon is running by trying to connect.
pub fn is_mounted(enlistment_root: &Path) -> bool {
    PipeClient::connect_to_mount(enlistment_root).is_ok()
}
