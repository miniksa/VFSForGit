---
description: "Complete Rust rewrite of VFSForGit — handoff document for continuation"
applyTo: "d:\\src\\VFSForGit\\rust-gvfs\\**"
---

# VFSForGit Rust Rewrite — Session Handoff

## Goal

Rewrite VFSForGit entirely in Rust on branch `user/miniksa/rust-rewrite` (already created from `user/miniksa/net10-nativeaot`). The Rust version must be functionally equivalent to the NativeAOT C# version and benchmarkable against it using the existing `scripts/benchmark.ps1` script.

## Repository Location

- **Repo:** `D:\src\VFSForGit`
- **Branch:** `user/miniksa/rust-rewrite` (already checked out)
- **Rust workspace:** `D:\src\VFSForGit\rust-gvfs/` (cargo init already done)
- **C# NativeAOT version:** branch `user/miniksa/net10-nativeaot` (the comparison target)
- **Production .NET FW version:** `C:\Program Files\GVFS\` (baseline comparison)

## What Must Be Built

### Executables (7 binaries)

| Binary | Purpose | Priority |
|--------|---------|----------|
| `gvfs.exe` | Main CLI — 15 verb commands + version | P0 (benchmark) |
| `gvfs-mount.exe` | Mount daemon — ProjFS provider, named pipe server, git object download | P0 (benchmark) |
| `gvfs-service.exe` | Windows service — repo registry, auto-mount, ProjFS enablement | P1 (functional) |
| `gvfs-service-ui.exe` | Tray notification app | P2 (optional) |
| `gvfs-hooks.exe` | Git hook handler — lock acquisition/release | P1 (functional) |
| `read-object.exe` | Native git hook — downloads missing objects | P0 (benchmark) |
| `post-index-changed.exe` | Native git hook — notifies mount of index changes | P1 |
| `virtual-filesystem.exe` | Native git hook — provides modified paths list | P1 |

### Benchmark Operations (P0 — must work for benchmarking)

The benchmark script (`scripts/benchmark.ps1`) tests these operations:
1. `gvfs version` — startup time
2. `gvfs clone <url> <path> --branch <branch>` — clone (without mount variant using `--no-mount` internally)
3. `gvfs mount <path>` — mount with ProjFS
4. `gvfs status <path>` — named pipe roundtrip to mount daemon
5. `gvfs prefetch <path> --files "*.md"` — HTTP object download
6. `git status` (in mounted repo) — exercises ProjFS + hooks
7. `git log --oneline -100` — exercises git on projected repo
8. `Get-ChildItem -Recurse` — ProjFS directory enumeration
9. `Get-Content <file>` — ProjFS file hydration (GetFileData callback)
10. `gvfs unmount <path>` — unmount

## Architecture Overview

### Suggested Cargo Workspace Layout

```
rust-gvfs/
├── Cargo.toml                 # Workspace root
├── crates/
│   ├── projfs/                # ProjFS P/Invoke bindings (unsafe FFI to ProjectedFSLib.dll)
│   │   ├── src/lib.rs
│   │   └── Cargo.toml
│   ├── gvfs-common/           # Shared types: named pipe protocol, config, git objects, HTTP
│   │   ├── src/lib.rs
│   │   └── Cargo.toml
│   ├── gvfs-virtualization/   # ProjFS provider: callbacks, projection, modified paths
│   │   ├── src/lib.rs
│   │   └── Cargo.toml
│   ├── gvfs-cli/              # Main CLI binary (gvfs.exe)
│   │   ├── src/main.rs
│   │   └── Cargo.toml
│   ├── gvfs-mount/            # Mount daemon binary (gvfs-mount.exe)
│   │   ├── src/main.rs
│   │   └── Cargo.toml
│   ├── gvfs-service/          # Windows service binary
│   │   ├── src/main.rs
│   │   └── Cargo.toml
│   ├── gvfs-hooks/            # Managed hook binary
│   │   ├── src/main.rs
│   │   └── Cargo.toml
│   └── gvfs-native-hooks/     # Native git hooks (3 small binaries)
│       ├── src/bin/read_object.rs
│       ├── src/bin/post_index_changed.rs
│       ├── src/bin/virtual_filesystem.rs
│       └── Cargo.toml
```

### Suggested Crate Dependencies

```toml
# Key crates to use:
clap = "4"                    # CLI parsing (equivalent to System.CommandLine)
serde = { version = "1", features = ["derive"] }
serde_json = "1"              # JSON serialization
tokio = { version = "1", features = ["full"] }  # Async runtime
reqwest = { version = "0.12", features = ["json"] }  # HTTP client
windows = { version = "0.58", features = [...] }  # Windows API bindings
windows-service = "0.7"       # Windows service support
sha2 = "0.10"                 # SHA-256 hashing
tracing = "0.1"               # Structured logging
tracing-subscriber = "0.3"
rusqlite = "0.32"             # SQLite (replaces Microsoft.Data.Sqlite)
```

For ProjFS specifically, use the `windows` crate with raw FFI since ProjFS isn't in the `windows` crate's API surface. Define the structs manually using the exact layouts we verified:

## Critical Technical Details (Learned the Hard Way)

### ProjFS Struct Layouts (verified against Windows SDK 10.0.26100.0)

```
PRJ_FILE_BASIC_INFO:        56 bytes
  IsDirectory (BOOLEAN):    offset 0,  1 byte + 7 padding
  FileSize (INT64):         offset 8,  8 bytes
  CreationTime:             offset 16, 8 bytes
  LastAccessTime:           offset 24, 8 bytes
  LastWriteTime:            offset 32, 8 bytes
  ChangeTime:               offset 40, 8 bytes
  FileAttributes (UINT32):  offset 48, 4 bytes + 4 padding

PRJ_PLACEHOLDER_INFO:       344 bytes (NOT 336!)
  FileBasicInfo:            offset 0,  56 bytes
  EaBufferSize:             offset 56, 4 bytes
  OffsetToFirstEa:          offset 60, 4 bytes
  SecurityBufferSize:       offset 64, 4 bytes
  OffsetToSecurityDescriptor: offset 68, 4 bytes
  StreamsInfoBufferSize:    offset 72, 4 bytes
  OffsetToFirstStreamInfo:  offset 76, 4 bytes
  VersionInfo:              offset 80, 256 bytes (ProviderID[128] + ContentID[128])
  VariableData[1]:          offset 336, 1 byte + 7 padding = 344 total
  *** MUST include VariableData or PrjWritePlaceholderInfo returns ERROR_INSUFFICIENT_BUFFER ***

PRJ_EXTENDED_INFO:          16 bytes
  InfoType (UINT32):        offset 0
  NextInfoOffset (UINT32):  offset 4
  Symlink.TargetName (PCWSTR): offset 8

PRJ_CALLBACK_DATA:          96 bytes
  Size:                     offset 0
  Flags:                    offset 4
  NamespaceVirtualizationContext: offset 8
  CommandId:                offset 16
  FileId (GUID):            offset 20
  DataStreamId (GUID):      offset 36
  FilePathName (PCWSTR):    offset 56
  VersionInfo:              offset 64
  TriggeringProcessId:      offset 72
  TriggeringProcessImageFileName: offset 80
  InstanceContext:           offset 88

PRJ_CALLBACKS:              64 bytes (8 function pointers × 8 bytes)
PRJ_STARTVIRTUALIZING_OPTIONS: 32 bytes
PRJ_NOTIFICATION_MAPPING:   16 bytes (NotificationBitMask u32 @ 0, NotificationRoot PCWSTR @ 8)
```

### ProjFS Callback Signatures (all __stdcall on x64)

```rust
type StartDirEnumCb = unsafe extern "system" fn(*const PRJ_CALLBACK_DATA, *const GUID) -> HRESULT;
type EndDirEnumCb = unsafe extern "system" fn(*const PRJ_CALLBACK_DATA, *const GUID) -> HRESULT;
type GetDirEnumCb = unsafe extern "system" fn(*const PRJ_CALLBACK_DATA, *const GUID, PCWSTR, PRJ_DIR_ENTRY_BUFFER_HANDLE) -> HRESULT;
type GetPlaceholderInfoCb = unsafe extern "system" fn(*const PRJ_CALLBACK_DATA) -> HRESULT;
type GetFileDataCb = unsafe extern "system" fn(*const PRJ_CALLBACK_DATA, u64, u32) -> HRESULT;
type QueryFileNameCb = unsafe extern "system" fn(*const PRJ_CALLBACK_DATA) -> HRESULT;
type NotificationCb = unsafe extern "system" fn(*const PRJ_CALLBACK_DATA, u8, i32, PCWSTR, *mut c_void) -> HRESULT;
type CancelCommandCb = unsafe extern "system" fn(*const PRJ_CALLBACK_DATA);
```

### Pitfalls to Avoid

1. **String marshaling:** ProjFS uses `PCWSTR` (null-terminated UTF-16). Use `widestring` crate or manual encoding. NEVER pass Rust string references directly — they're UTF-8 and not null-terminated.

2. **Notification mapping lifetime:** The notification mapping array and root string pointers passed to `PrjStartVirtualizing` must remain valid for the ENTIRE lifetime of the virtualization instance. ProjFS caches these pointers.

3. **ReFS symlink limitation:** ProjFS symlinks don't work on ReFS volumes (only NTFS). The test repo is on `D:` (ReFS), standalone tests use `C:` (NTFS). The benchmark script uses `C:\Repos\`.

4. **Named pipe protocol:** Messages are `Header|Body` separated by `|`, terminated by `0x03` (ETX). JSON bodies use the exact field names from the C# classes. The pipe name for an enlistment is derived from the enlistment root path (SHA1 hash or similar).

5. **Git hooks:** The `read-object.exe` hook communicates via named pipe to GVFS.Mount, not via git's protocol. It sends `DLO|<sha>` and gets `S` or `F` back into the content.

6. **Process launching:** `gvfs mount` launches `gvfs-mount.exe` as a background process (not attached to console). Must use `UseShellExecute=true` equivalent or `CREATE_NEW_PROCESS_GROUP` to avoid inheriting stdin/stdout handles.

### Named Pipe Message Protocol

**Wire format:** UTF-8 text, `Header|Body`, terminated by `\x03` (ETX byte).

**Per-enlistment pipe** (GVFS.Mount):
- `GetStatus` → JSON response with mount status
- `Unmount` → triggers clean shutdown
- `AcquireLock|<pid>|<isElevated>|<checkOnly>|<cmdLen>|<cmd>|<sessIdLen>|<sessId>` → lock response
- `ReleaseLock|<lock-data>` → release + modified files list
- `DLO|<40-char-sha>` → `S` or `F`
- `MPL|1` → `S|<null-delimited-paths>`
- `PICN|<flags>` → `S` or `F`

**Service pipe** (GVFS.Service):
- JSON request/response for repo registration, ProjFS enablement

### HTTP Protocol (GVFS Protocol)

GVFS uses Azure DevOps GVFS protocol endpoints:
- `GET /gvfs/config` — server config (cache server URLs, version ranges)
- `POST /gvfs/objects` — batch object download (request: JSON list of SHAs, response: packfile)
- `GET /gvfs/objects/<sha>` — single object download (loose object format)  
- `POST /gvfs/sizes` — query object sizes
- `GET /gvfs/prefetch?lastPackTimestamp=<ts>` — prefetch pack

Authentication: Azure DevOps PAT or Azure Identity, passed as `Authorization: Bearer <token>` or via git credential helpers.

### Config File Locations

- `.gvfs/databases/RepoMetadata.dat` — INI-like key-value
- `.gvfs/databases/VFSForGit.sqlite` — SQLite with placeholders, sparse tables
- `.gvfs/databases/ModifiedPaths.dat` — one path per line, `A ` prefix for add, `D ` for delete
- `.git/config` — standard git config with `gvfs.*` extension keys
- `%LocalAppData%\GVFS\GVFS.config` — global config JSON
- `%ProgramData%\GVFS.Service\repo-registry` — JSON-per-line repo list

## Benchmark Script

The benchmark script at `scripts/benchmark.ps1` expects:
- `-UsePublished` flag to use the built binary (currently points to `D:\src\out\gvfs-aot-layout\GVFS.exe`)
- Production comparison against `C:\Program Files\GVFS\GVFS.exe`
- The Rust binary should be placed in a layout directory and the script updated to point to it
- Must support: `version`, `clone`, `mount`, `status`, `prefetch`, `unmount` subcommands
- Must work with git 2.53.0.vfs.0.0 at `C:\Program Files\Git\cmd\git.exe`
- Test repo: `https://gvfs.visualstudio.com/ci/_git/ForTests` branch `FunctionalTests/20201014`

## How to Run the Benchmark

```powershell
# From elevated PowerShell:
cd D:\src\VFSForGit
pwsh -File scripts\benchmark.ps1 -Iterations 5 -UsePublished
```

Update `scripts/benchmark.ps1` to add a Rust variant path, e.g.:
```powershell
$rustGVFS = "D:\src\VFSForGit\rust-gvfs\target\release\gvfs.exe"
```

## Existing Branch State

- `rust-gvfs/` directory exists with `cargo init` done
- Branch `user/miniksa/rust-rewrite` is checked out
- All C# code from the NativeAOT branch is still present (can be referenced)
- The NativeAOT benchmark results are in `benchmark-results-20260221-132525.md`

## Definition of Done

1. All Rust binaries compile with `cargo build --release`
2. `gvfs version` works
3. `gvfs clone` works against the ForTests repo
4. `gvfs mount` starts ProjFS provider, serves directory enumerations and file hydration
5. `gvfs status` returns mount status via named pipe
6. `gvfs unmount` cleanly shuts down
7. `git status` / `git log` work in the mounted repo
8. `Get-ChildItem -Recurse` enumerates all 864 files with 0 errors
9. Benchmark script runs successfully comparing Rust vs Production
10. Results committed as `benchmark-results-rust-<timestamp>.md`
