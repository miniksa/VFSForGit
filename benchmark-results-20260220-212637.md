# VFSForGit .NET 10 Migration â€” Performance Comparison

**Date**: 2026-02-20 21:30
**Machine**: NIKSA-13900 (Intel64 Family 6 Model 183 Stepping 1, GenuineIntel)
**OS**: Microsoft Windows 11 Enterprise Build 26200
**Iterations**: 5 per benchmark (clones: 3)

| Build | Version | Framework | Config |
|-------|---------|-----------|--------|
| Production |  | .NET Framework 4.7.1 | Installed (C:\Program Files\GVFS) |
| .NET 10 |  | .NET 10.0 (self-contained) | Build output (Release) |

## Results

| Benchmark | Production (ms) | .NET 10 (ms) | Delta | Change |
|-----------|---------------:|-------------:|------:|--------|| Startup (gvfs version) | 59.1 Â± 4.9 | 54.8 Â± 0.8 | -4.3 (-7.3%) | **faster** |
| Clone (small repo, no mount) | 5266.8 Â± 230.1 | 1040.8 Â± 64.6 | -4226 (-80.2%) | **faster** |
| Mount (small repo) | 4757.8 Â± 227.2 | 717.8 Â± 9.2 | -4040 (-84.9%) | **faster** |
| Status (pipe roundtrip) | 344.5 Â± 6.3 | 131.8 Â± 3.4 | -212.7 (-61.7%) | **faster** |
| Prefetch (*.md) | 1091.5 Â± 552.9 | 149 Â± 7.6 | -942.5 (-86.3%) | **faster** |
| Git Status | 187.2 Â± 6 | 33.4 Â± 1.5 | -153.8 (-82.2%) | **faster** |
| Git Log (-100) | 136.7 Â± 3.8 | 31.7 Â± 0.9 | -105 (-76.8%) | **faster** |
| Dir Enumeration (ProjFS) | 245.9 Â± 80.1 | 2.5 Â± 1.1 | -243.4 (-99%) | **faster** |
| File Read (hydration) | 8.4 Â± 13.1 | 0.8 Â± 0.8 | -7.6 (-90.5%) | **faster** |
| Unmount | 903.4 Â± 476.6 | 3092.6 Â± 6 | +2189.2 (+242.3%) | slower |
| Large Repo Git Status | 34.9 Â± 3.6 | N/A | N/A | baseline |

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
  Production: min=55 avg=59.1 max=67.6 stddev=4.9
  .NET 10:    min=53.6 avg=54.8 max=55.7 stddev=0.8

Clone (small repo, no mount):
  Production: min=5006.5 avg=5266.8 max=5443.1 stddev=230.1
  .NET 10:    min=986.9 avg=1040.8 max=1112.5 stddev=64.6

Mount (small repo):
  Production: min=4538.5 avg=4757.8 max=5114.9 stddev=227.2
  .NET 10:    min=705.5 avg=717.8 max=726.2 stddev=9.2

Status (pipe roundtrip):
  Production: min=334.4 avg=344.5 max=353.8 stddev=6.3
  .NET 10:    min=127.9 avg=131.8 max=137.6 stddev=3.4

Prefetch (*.md):
  Production: min=745.3 avg=1091.5 max=1729.2 stddev=552.9
  .NET 10:    min=143.3 avg=149 max=157.7 stddev=7.6

Git Status:
  Production: min=180.6 avg=187.2 max=194.2 stddev=6
  .NET 10:    min=32.5 avg=33.4 max=36.1 stddev=1.5

Git Log (-100):
  Production: min=132.4 avg=136.7 max=142.1 stddev=3.8
  .NET 10:    min=31.1 avg=31.7 max=33.2 stddev=0.9

Dir Enumeration (ProjFS):
  Production: min=186.6 avg=245.9 max=365 stddev=80.1
  .NET 10:    min=1.8 avg=2.5 max=4.5 stddev=1.1

File Read (hydration):
  Production: min=0.8 avg=8.4 max=23.5 stddev=13.1
  .NET 10:    min=0.4 avg=0.8 max=1.7 stddev=0.8

Unmount:
  Production: min=353.1 avg=903.4 max=1180.6 stddev=476.6
  .NET 10:    min=3086.4 avg=3092.6 max=3098.4 stddev=6

Large Repo Git Status:
  Production: min=31 avg=34.9 max=40.7 stddev=3.6

```

