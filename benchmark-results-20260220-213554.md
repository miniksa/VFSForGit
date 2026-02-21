# VFSForGit .NET 10 Migration â€” Performance Comparison

**Date**: 2026-02-20 21:40
**Machine**: NIKSA-13900 (Intel64 Family 6 Model 183 Stepping 1, GenuineIntel)
**OS**: Microsoft Windows 11 Enterprise Build 26200
**Iterations**: 5 per benchmark (clones: 3)

| Build | Version | Framework | Config |
|-------|---------|-----------|--------|
| Production |  | .NET Framework 4.7.1 | Installed (C:\Program Files\GVFS) |
| .NET 10 |  | .NET 10.0 (self-contained) | Build output (Release) |

## Results

| Benchmark | Production (ms) | .NET 10 (ms) | Delta | Change |
|-----------|---------------:|-------------:|------:|--------|| Startup (gvfs version) | 53.7 Â± 1.5 | 54.4 Â± 0.8 | +0.7 (+1.3%) | ~same |
| Clone (small repo, no mount) | 5147.5 Â± 437.9 | 6624.4 Â± 1134 | +1476.9 (+28.7%) | slower |
| Mount (small repo) | 4778.6 Â± 152.7 | 6589.3 Â± 360.1 | +1810.7 (+37.9%) | slower |
| Status (pipe roundtrip) | 360.1 Â± 22.1 | 331.4 Â± 19.8 | -28.7 (-8%) | **faster** |
| Prefetch (*.md) | 1024.8 Â± 447.8 | 1353.1 Â± 838 | +328.3 (+32%) | slower |
| Git Status | 198.1 Â± 2.8 | 246.1 Â± 6 | +48 (+24.2%) | slower |
| Git Log (-100) | 150.5 Â± 7.3 | 211.9 Â± 17.7 | +61.4 (+40.8%) | slower |
| Dir Enumeration (ProjFS) | 273.5 Â± 75 | 263.4 Â± 82.1 | -10.1 (-3.7%) | ~same |
| File Read (hydration) | 6.7 Â± 10.1 | 4.3 Â± 6 | -2.4 (-35.8%) | **faster** |
| Unmount | 874.6 Â± 505.1 | 830.4 Â± 569.5 | -44.2 (-5.1%) | **faster** |
| Large Repo Git Status | 29.6 Â± 0.7 | N/A | N/A | baseline |

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
  Production: min=51.6 avg=53.7 max=55.3 stddev=1.5
  .NET 10:    min=53.3 avg=54.4 max=55.2 stddev=0.8

Clone (small repo, no mount):
  Production: min=4887.1 avg=5147.5 max=5653.1 stddev=437.9
  .NET 10:    min=5701.2 avg=6624.4 max=7890.1 stddev=1134

Mount (small repo):
  Production: min=4552.6 avg=4778.6 max=4912.7 stddev=152.7
  .NET 10:    min=6125.7 avg=6589.3 max=7132.5 stddev=360.1

Status (pipe roundtrip):
  Production: min=332.8 avg=360.1 max=408.8 stddev=22.1
  .NET 10:    min=316.8 avg=331.4 max=385.7 stddev=19.8

Prefetch (*.md):
  Production: min=744.4 avg=1024.8 max=1541.2 stddev=447.8
  .NET 10:    min=855.8 avg=1353.1 max=2320.7 stddev=838

Git Status:
  Production: min=194.9 avg=198.1 max=201.5 stddev=2.8
  .NET 10:    min=240.8 avg=246.1 max=255.2 stddev=6

Git Log (-100):
  Production: min=144.8 avg=150.5 max=161.3 stddev=7.3
  .NET 10:    min=200.6 avg=211.9 max=242.1 stddev=17.7

Dir Enumeration (ProjFS):
  Production: min=232.5 avg=273.5 max=407.2 stddev=75
  .NET 10:    min=222 avg=263.4 max=410 stddev=82.1

File Read (hydration):
  Production: min=0.9 avg=6.7 max=18.4 stddev=10.1
  .NET 10:    min=0.8 avg=4.3 max=11.2 stddev=6

Unmount:
  Production: min=292.4 avg=874.6 max=1194.2 stddev=505.1
  .NET 10:    min=173 avg=830.4 max=1171.6 stddev=569.5

Large Repo Git Status:
  Production: min=29 avg=29.6 max=30.7 stddev=0.7

```

