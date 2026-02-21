# VFSForGit .NET 10 Migration — Performance Comparison

**Date**: 2026-02-21 13:29
**Machine**: NIKSA-13900 (Intel64 Family 6 Model 183 Stepping 1, GenuineIntel)
**OS**: Microsoft Windows 11 Enterprise Build 26200
**Iterations**: 5 per benchmark (clones: 3)

| Build | Version | Framework | Config |
|-------|---------|-----------|--------|
| Production |  | .NET Framework 4.7.1 | Installed (C:\Program Files\GVFS) |
| .NET 10 | GVFS 0.2.173.2 | .NET 10.0 (self-contained) | Build output (Release) |

## Results

| Benchmark | Production (ms) | .NET 10 (ms) | Delta | Change |
|-----------|---------------:|-------------:|------:|--------|| Startup (gvfs version) | 53.5 ± 2.6 | 15.3 ± 1.1 | -38.2 (-71.4%) | **faster** |
| Clone (small repo, no mount) | 5234.2 ± 101.4 | 5287.2 ± 127.4 | +53 (+1%) | ~same |
| Mount (small repo) | 4603.4 ± 81.6 | 4147.7 ± 177.8 | -455.7 (-9.9%) | **faster** |
| Status (pipe roundtrip) | 341.3 ± 13.8 | 179.4 ± 9.7 | -161.9 (-47.4%) | **faster** |
| Prefetch (*.md) | 1067.2 ± 509 | 824.3 ± 683.9 | -242.9 (-22.8%) | **faster** |
| Git Status | 191.6 ± 15.7 | 139.2 ± 17.8 | -52.4 (-27.3%) | **faster** |
| Git Log (-100) | 140 ± 6 | 119.1 ± 2.8 | -20.9 (-14.9%) | **faster** |
| Dir Enumeration (ProjFS) | 187.3 ± 70.5 | 186.2 ± 55.6 | -1.1 (-0.6%) | ~same |
| File Read (hydration) | 8.3 ± 13.3 | 1.8 ± 1.9 | -6.5 (-78.3%) | **faster** |
| Unmount | 873.9 ± 509.2 | 778.7 ± 620.7 | -95.2 (-10.9%) | **faster** |
| Large Repo Git Status | 29.8 ± 1.8 | N/A | N/A | baseline |

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
  Production: min=51.2 avg=53.5 max=57.9 stddev=2.6
  .NET 10:    min=14.2 avg=15.3 max=17.1 stddev=1.1

Clone (small repo, no mount):
  Production: min=5174.1 avg=5234.2 max=5351.2 stddev=101.4
  .NET 10:    min=5141.3 avg=5287.2 max=5376.7 stddev=127.4

Mount (small repo):
  Production: min=4526.2 avg=4603.4 max=4711.5 stddev=81.6
  .NET 10:    min=3955.4 avg=4147.7 max=4439.1 stddev=177.8

Status (pipe roundtrip):
  Production: min=327.9 avg=341.3 max=373.6 stddev=13.8
  .NET 10:    min=169.1 avg=179.4 max=196.6 stddev=9.7

Prefetch (*.md):
  Production: min=739.7 avg=1067.2 max=1653.6 stddev=509
  .NET 10:    min=412.8 avg=824.3 max=1613.7 stddev=683.9

Git Status:
  Production: min=177.3 avg=191.6 max=217.8 stddev=15.7
  .NET 10:    min=130 avg=139.2 max=171 stddev=17.8

Git Log (-100):
  Production: min=134.8 avg=140 max=149.3 stddev=6
  .NET 10:    min=114.5 avg=119.1 max=121.3 stddev=2.8

Dir Enumeration (ProjFS):
  Production: min=147 avg=187.3 max=312.5 stddev=70.5
  .NET 10:    min=141.7 avg=186.2 max=278.1 stddev=55.6

File Read (hydration):
  Production: min=0.5 avg=8.3 max=23.6 stddev=13.3
  .NET 10:    min=0.6 avg=1.8 max=4 stddev=1.9

Unmount:
  Production: min=285.9 avg=873.9 max=1174.3 stddev=509.2
  .NET 10:    min=62.1 avg=778.7 max=1146 stddev=620.7

Large Repo Git Status:
  Production: min=28.1 avg=29.8 max=32.6 stddev=1.8

```

