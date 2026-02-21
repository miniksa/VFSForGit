# VFSForGit .NET 10 Migration â€” Performance Comparison

**Date**: 2026-02-20 21:23
**Machine**: NIKSA-13900 (Intel64 Family 6 Model 183 Stepping 1, GenuineIntel)
**OS**: Microsoft Windows 11 Enterprise Build 26200
**Iterations**: 5 per benchmark (clones: 3)

| Build | Version | Framework | Config |
|-------|---------|-----------|--------|
| Production |  | .NET Framework 4.7.1 | Installed (C:\Program Files\GVFS) |
| .NET 10 |  | .NET 10.0 (self-contained) | Build output (Release) |

## Results

| Benchmark | Production (ms) | .NET 10 (ms) | Delta | Change |
|-----------|---------------:|-------------:|------:|--------|| Startup (gvfs version) | 53.4 Â± 0.9 | 55.6 Â± 0.8 | +2.2 (+4.1%) | ~same |
| Clone (small repo, no mount) | 5351.1 Â± 597.4 | 968.6 Â± 73.4 | -4382.5 (-81.9%) | **faster** |
| Mount (small repo) | 4594.8 Â± 122.7 | 713.8 Â± 9.7 | -3881 (-84.5%) | **faster** |
| Status (pipe roundtrip) | 340.6 Â± 9.2 | 124.9 Â± 1.2 | -215.7 (-63.3%) | **faster** |
| Prefetch (*.md) | 1058.1 Â± 599.5 | 131 Â± 3.6 | -927.1 (-87.6%) | **faster** |
| Git Status | 169.1 Â± 5.2 | 30 Â± 1.2 | -139.1 (-82.3%) | **faster** |
| Git Log (-100) | 125.8 Â± 3.3 | 29.5 Â± 1.1 | -96.3 (-76.6%) | **faster** |
| Dir Enumeration (ProjFS) | 221.7 Â± 67.8 | 2.5 Â± 1.5 | -219.2 (-98.9%) | **faster** |
| File Read (hydration) | 9.7 Â± 14.6 | 1.2 Â± 1 | -8.5 (-87.6%) | **faster** |
| Unmount | 783.5 Â± 454.6 | 3098.9 Â± 1 | +2315.4 (+295.5%) | slower |
| Large Repo Git Status | 31.3 Â± 1.9 | N/A | N/A | baseline |

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
  Production: min=52 avg=53.4 max=54.3 stddev=0.9
  .NET 10:    min=54.3 avg=55.6 max=56.3 stddev=0.8

Clone (small repo, no mount):
  Production: min=4979.5 avg=5351.1 max=6040.2 stddev=597.4
  .NET 10:    min=923.9 avg=968.6 max=1053.3 stddev=73.4

Mount (small repo):
  Production: min=4479 avg=4594.8 max=4773.9 stddev=122.7
  .NET 10:    min=700.2 avg=713.8 max=726.4 stddev=9.7

Status (pipe roundtrip):
  Production: min=332.5 avg=340.6 max=364.6 stddev=9.2
  .NET 10:    min=123.6 avg=124.9 max=127.1 stddev=1.2

Prefetch (*.md):
  Production: min=706.2 avg=1058.1 max=1750.3 stddev=599.5
  .NET 10:    min=128.2 avg=131 max=135.1 stddev=3.6

Git Status:
  Production: min=163.5 avg=169.1 max=175 stddev=5.2
  .NET 10:    min=28.6 avg=30 max=31.7 stddev=1.2

Git Log (-100):
  Production: min=121.2 avg=125.8 max=130.2 stddev=3.3
  .NET 10:    min=28.4 avg=29.5 max=31.1 stddev=1.1

Dir Enumeration (ProjFS):
  Production: min=180.7 avg=221.7 max=342.2 stddev=67.8
  .NET 10:    min=1.8 avg=2.5 max=5.2 stddev=1.5

File Read (hydration):
  Production: min=1.1 avg=9.7 max=26.5 stddev=14.6
  .NET 10:    min=0.5 avg=1.2 max=2.4 stddev=1

Unmount:
  Production: min=301.8 avg=783.5 max=1205 stddev=454.6
  .NET 10:    min=3097.8 avg=3098.9 max=3099.9 stddev=1

Large Repo Git Status:
  Production: min=29.4 avg=31.3 max=33.9 stddev=1.9

```

