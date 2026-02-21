# VFSForGit .NET 10 Migration â€” Performance Comparison

**Date**: 2026-02-20 17:01
**Machine**: NIKSA-13900 (Intel64 Family 6 Model 183 Stepping 1, GenuineIntel)
**OS**: Microsoft Windows 11 Enterprise Build 26200
**Iterations**: 5 per benchmark (clones: 3)

| Build | Version | Framework | Config |
|-------|---------|-----------|--------|
| Production |  | .NET Framework 4.7.1 | Installed (C:\Program Files\GVFS) |
| .NET 10 |  | .NET 10.0 (self-contained) | Build output (Release) |

## Results

| Benchmark | Production (ms) | .NET 10 (ms) | Delta | Change |
|-----------|---------------:|-------------:|------:|--------|
| Startup (gvfs version) | 52 Â± 0.5 | 50.6 Â± 0.8 | -1.4 (-2.7%) | ~same |
| Clone (small repo, no mount) | 5573.3 Â± 368.2 | 4963.1 Â± 197.9 | -610.2 (-10.9%) | **faster** |
| Mount (small repo) | 4680.7 Â± 291.1 | 705.2 Â± 18.1 | -3975.5 (-84.9%) | **faster** |
| Status (pipe roundtrip) | 334.7 Â± 5.6 | 3282.7 Â± 11 | +2948 (+880.8%) | slower |
| Prefetch (*.md) | 961.8 Â± 473 | 1791.4 Â± 75.3 | +829.6 (+86.3%) | slower |
| Git Status | 164.4 Â± 3.6 | 3108.4 Â± 5.5 | +2944 (+1790.8%) | slower |
| Git Log (-100) | 122.3 Â± 1.7 | 175.6 Â± 2.8 | +53.3 (+43.6%) | slower |
| Dir Enumeration (ProjFS) | 235.3 Â± 64.1 | 1.1 Â± 1.1 | -234.2 (-99.5%) | **faster** |
| File Read (hydration) | 8.2 Â± 12.9 | 0.8 Â± 0.7 | -7.4 (-90.2%) | **faster** |
| Unmount | 914.8 Â± 502.1 | 3083.5 Â± 5.6 | +2168.7 (+237.1%) | slower |
| Large Repo Git Status | 36 Â± 2.9 | N/A | N/A | baseline |

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
  Production: min=51.4 avg=52 max=52.7 stddev=0.5
  .NET 10:    min=49.7 avg=50.6 max=51.7 stddev=0.8

Clone (small repo, no mount):
  Production: min=5270.7 avg=5573.3 max=5983.3 stddev=368.2
  .NET 10:    min=4834.6 avg=4963.1 max=5191 stddev=197.9

Mount (small repo):
  Production: min=4459.7 avg=4680.7 max=5190.6 stddev=291.1
  .NET 10:    min=688.7 avg=705.2 max=736.4 stddev=18.1

Status (pipe roundtrip):
  Production: min=327.7 avg=334.7 max=345.9 stddev=5.6
  .NET 10:    min=3262.2 avg=3282.7 max=3299.1 stddev=11

Prefetch (*.md):
  Production: min=683.4 avg=961.8 max=1508 stddev=473
  .NET 10:    min=1714.4 avg=1791.4 max=1864.9 stddev=75.3

Git Status:
  Production: min=159.8 avg=164.4 max=168.8 stddev=3.6
  .NET 10:    min=3103.1 avg=3108.4 max=3115.2 stddev=5.5

Git Log (-100):
  Production: min=121.1 avg=122.3 max=125.3 stddev=1.7
  .NET 10:    min=173.6 avg=175.6 max=180.6 stddev=2.8

Dir Enumeration (ProjFS):
  Production: min=186 avg=235.3 max=338.5 stddev=64.1
  .NET 10:    min=0.5 avg=1.1 max=3.1 stddev=1.1

File Read (hydration):
  Production: min=0.7 avg=8.2 max=23.1 stddev=12.9
  .NET 10:    min=0.4 avg=0.8 max=1.7 stddev=0.7

Unmount:
  Production: min=335.1 avg=914.8 max=1214.9 stddev=502.1
  .NET 10:    min=3080.1 avg=3083.5 max=3089.9 stddev=5.6

Large Repo Git Status:
  Production: min=32.9 avg=36 max=39.8 stddev=2.9

```

