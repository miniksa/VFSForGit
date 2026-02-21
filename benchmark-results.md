# VFSForGit .NET 10 Migration â€” Performance Comparison

**Date**: 2026-02-20 16:45
**Machine**: NIKSA-13900 (Intel64 Family 6 Model 183 Stepping 1, GenuineIntel)
**OS**: Microsoft Windows 11 Enterprise Build 26200
**Iterations**: 5 per benchmark (clones: 3)

| Build | Version | Framework | Config |
|-------|---------|-----------|--------|
| Production | GVFS.exe : GVFS 1.0.26014.1
At D:\src\VFSForGit\scripts\benchmark.ps1:284 char:17
+ $prodVersion = (& $prodGVFS version 2>&1 | Out-String).Trim()
+                 ~~~~~~~~~~~~~~~~~~~~~~~~
    + CategoryInfo          : NotSpecified: (GVFS 1.0.26014.1:String) [], RemoteException
    + FullyQualifiedErrorId : NativeCommandError | .NET Framework 4.7.1 | Installed (C:\Program Files\GVFS) |
| .NET 10 | GVFS.exe : GVFS 0.2.173.2
At D:\src\VFSForGit\scripts\benchmark.ps1:285 char:18
+ $net10Version = (& $net10GVFS version 2>&1 | Out-String).Trim()
+                  ~~~~~~~~~~~~~~~~~~~~~~~~~
    + CategoryInfo          : NotSpecified: (GVFS 0.2.173.2:String) [], RemoteException
    + FullyQualifiedErrorId : NativeCommandError | .NET 10.0 (self-contained) | Build output (Release) |

## Results

| Benchmark | Production (ms) | .NET 10 (ms) | Delta | Change |
|-----------|---------------:|-------------:|------:|--------|| Startup (gvfs version) | 52.4 Â± 1 | 54.5 Â± 0.7 | +2.1 (+4%) | ~same |
| Clone (small repo, no mount) | 5501.5 Â± 209.6 | 8312.5 Â± 2793 | +2811 (+51.1%) | slower |
| Mount (small repo) | 4522.7 Â± 58.7 | 6418.5 Â± 244.1 | +1895.8 (+41.9%) | slower |
| Status (pipe roundtrip) | 336.9 Â± 9.4 | 345.8 Â± 17 | +8.9 (+2.6%) | ~same |
| Prefetch (*.md) | 1038.8 Â± 435.4 | 1567.8 Â± 1189.5 | +529 (+50.9%) | slower |
| Git Status | 201.5 Â± 20.3 | 229.5 Â± 14.8 | +28 (+13.9%) | slower |
| Git Log (-100) | 136.2 Â± 2.2 | 192 Â± 7.6 | +55.8 (+41%) | slower |
| Dir Enumeration (ProjFS) | 227 Â± 66.8 | 268.9 Â± 128.6 | +41.9 (+18.5%) | slower |
| File Read (hydration) | 5.2 Â± 7.9 | 3.5 Â± 4.7 | -1.7 (-32.7%) | **faster** |
| Unmount | 930.7 Â± 513.3 | 879.2 Â± 563.4 | -51.5 (-5.5%) | **faster** |
| Large Repo Git Status | 29.5 Â± 0.5 | N/A | N/A | baseline |

## Details

### Methodology
- Each benchmark runs the specified number of iterations
- Times measured with [Stopwatch] (wall clock, includes process creation overhead)
- Clone benchmarks limited to 3 iterations due to network I/O
- Status benchmarks run 10 iterations for better precision
- Both builds use the same git version: git version 2.53.0.vfs.0.0
- Test repo: `https://gvfs.visualstudio.com/ci/_git/ForTests` (branch: `FunctionalTests/20201014`)

### Notes
- **Startup time** includes .NET runtime initialization. Self-contained .NET 10 apps
  have slightly different startup characteristics vs framework-dependent .NET Framework apps.
- **Mount** includes ProjFS virtualization root creation, git index parsing, and
  named pipe server startup.
- **Status** measures named pipe roundtrip latency to the GVFS.Mount daemon.
- **Prefetch** is primarily network-bound; differences reflect HTTP stack changes
  between .NET Framework's HttpWebRequest and .NET 10's HttpClient.
- **ProjFS operations** (enumeration, hydration) are largely kernel-mode; managed
  code differences are minimal for these operations.

### Raw Data

```Startup (gvfs version):
  Production: min=51.2 avg=52.4 max=53.6 stddev=1
  .NET 10:    min=53.3 avg=54.5 max=55.1 stddev=0.7

Clone (small repo, no mount):
  Production: min=5275 avg=5501.5 max=5688.7 stddev=209.6
  .NET 10:    min=5639.8 avg=8312.5 max=11211.9 stddev=2793

Mount (small repo):
  Production: min=4461.6 avg=4522.7 max=4608.5 stddev=58.7
  .NET 10:    min=5996.7 avg=6418.5 max=6607.3 stddev=244.1

Status (pipe roundtrip):
  Production: min=329.3 avg=336.9 max=361.4 stddev=9.4
  .NET 10:    min=333.1 avg=345.8 max=392.1 stddev=17

Prefetch (*.md):
  Production: min=756.6 avg=1038.8 max=1540.2 stddev=435.4
  .NET 10:    min=864.7 avg=1567.8 max=2941.2 stddev=1189.5

Git Status:
  Production: min=185.7 avg=201.5 max=234.8 stddev=20.3
  .NET 10:    min=218.5 avg=229.5 max=254.6 stddev=14.8

Git Log (-100):
  Production: min=133.4 avg=136.2 max=138.4 stddev=2.2
  .NET 10:    min=180.8 avg=192 max=199.8 stddev=7.6

Dir Enumeration (ProjFS):
  Production: min=183.9 avg=227 max=341.3 stddev=66.8
  .NET 10:    min=185.1 avg=268.9 max=493.4 stddev=128.6

File Read (hydration):
  Production: min=0.6 avg=5.2 max=14.2 stddev=7.9
  .NET 10:    min=0.6 avg=3.5 max=8.9 stddev=4.7

Unmount:
  Production: min=338 avg=930.7 max=1232.6 stddev=513.3
  .NET 10:    min=233.1 avg=879.2 max=1268.3 stddev=563.4

Large Repo Git Status:
  Production: min=28.9 avg=29.5 max=30.2 stddev=0.5

```

