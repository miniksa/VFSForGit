//! GVFS Lock — exclusive lock for coordinating git and GVFS operations.
//!
//! The mount daemon holds a single exclusive lock that can be acquired by
//! either GVFS itself (internal operations like prefetch) or an external
//! process (git commands dispatched through hooks).

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// Data about who holds the lock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockData {
    pub pid: u32,
    pub is_elevated: bool,
    pub check_availability_only: bool,
    pub parsed_command: String,
    pub session_id: String,
}

impl LockData {
    /// Parse lock data from the wire format:
    /// `PID|isElevated|checkAvailabilityOnly|parsedCommandLength|parsedCommand|sessionIdLength|sessionId`
    pub fn from_wire(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.splitn(7, '|').collect();
        if parts.len() < 7 {
            return None;
        }

        let pid = parts[0].parse().ok()?;
        let is_elevated = parts[1] == "1" || parts[1].eq_ignore_ascii_case("true");
        let check_availability_only = parts[2] == "1" || parts[2].eq_ignore_ascii_case("true");
        let _cmd_len: usize = parts[3].parse().ok()?;
        let parsed_command = parts[4].to_string();
        let _session_len: usize = parts[5].parse().ok()?;
        let session_id = parts[6].to_string();

        Some(Self {
            pid,
            is_elevated,
            check_availability_only,
            parsed_command,
            session_id,
        })
    }

    /// Serialize to wire format.
    pub fn to_wire(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}",
            self.pid,
            if self.is_elevated { "1" } else { "0" },
            if self.check_availability_only {
                "1"
            } else {
                "0"
            },
            self.parsed_command.len(),
            self.parsed_command,
            self.session_id.len(),
            self.session_id,
        )
    }
}

/// The GVFS lock state.
enum LockHolder {
    /// No one holds the lock.
    Free,
    /// GVFS internal operation holds the lock.
    Gvfs,
    /// An external process holds the lock.
    External(LockData),
}

/// The GVFS lock manager.
pub struct GvfsLock {
    state: Mutex<LockHolder>,
}

impl GvfsLock {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(LockHolder::Free),
        }
    }

    /// Try to acquire the lock for an internal GVFS operation.
    pub fn try_acquire_for_gvfs(&self) -> bool {
        let mut state = self.state.lock();
        match &*state {
            LockHolder::Free => {
                *state = LockHolder::Gvfs;
                true
            }
            _ => false,
        }
    }

    /// Try to acquire the lock for an external process (git command via hooks).
    /// Returns `(acquired, existing_holder)`.
    pub fn try_acquire_for_external(
        &self,
        requestor: LockData,
    ) -> (bool, Option<LockData>) {
        let mut state = self.state.lock();
        match &*state {
            LockHolder::Free => {
                if requestor.check_availability_only {
                    // Don't actually acquire, just report it's available.
                    return (true, None);
                }
                *state = LockHolder::External(requestor);
                (true, None)
            }
            LockHolder::Gvfs => (false, None),
            LockHolder::External(existing) => {
                // Check if the existing holder is still alive.
                if is_process_alive(existing.pid) {
                    (false, Some(existing.clone()))
                } else {
                    // Process died, we can take the lock.
                    warn!(
                        "Previous lock holder (PID {}) is dead, granting to PID {}",
                        existing.pid, requestor.pid
                    );
                    if requestor.check_availability_only {
                        return (true, None);
                    }
                    *state = LockHolder::External(requestor);
                    (true, None)
                }
            }
        }
    }

    /// Release the lock held by GVFS.
    pub fn release_for_gvfs(&self) {
        let mut state = self.state.lock();
        if matches!(&*state, LockHolder::Gvfs) {
            *state = LockHolder::Free;
            debug!("GVFS internal lock released");
        }
    }

    /// Release the lock held by an external process.
    pub fn release_for_external(&self, pid: u32) -> Option<LockData> {
        let mut state = self.state.lock();
        if let LockHolder::External(data) = &*state {
            if data.pid == pid {
                let data = if let LockHolder::External(d) =
                    std::mem::replace(&mut *state, LockHolder::Free)
                {
                    Some(d)
                } else {
                    None
                };
                debug!("External lock released by PID {}", pid);
                return data;
            }
        }
        None
    }

    /// Get the command of the current external lock holder.
    pub fn get_locked_command(&self) -> Option<String> {
        let state = self.state.lock();
        match &*state {
            LockHolder::External(data) => Some(data.parsed_command.clone()),
            _ => None,
        }
    }

    /// Check if the lock is held.
    pub fn is_locked(&self) -> bool {
        let state = self.state.lock();
        !matches!(&*state, LockHolder::Free)
    }

    /// Check if GVFS holds the lock.
    pub fn is_gvfs_locked(&self) -> bool {
        let state = self.state.lock();
        matches!(&*state, LockHolder::Gvfs)
    }
}

impl Default for GvfsLock {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a process with the given PID is still alive.
fn is_process_alive(pid: u32) -> bool {
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    let result = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) };
    match result {
        Ok(handle) => {
            let _ = unsafe { windows::Win32::Foundation::CloseHandle(handle) };
            true
        }
        Err(_) => false,
    }
}
