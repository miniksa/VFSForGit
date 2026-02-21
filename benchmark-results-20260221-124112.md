# VFSForGit .NET 10 Migration — Performance Comparison

**Date**: 2026-02-21 12:44
**Machine**: NIKSA-13900 (Intel64 Family 6 Model 183 Stepping 1, GenuineIntel)
**OS**: Microsoft Windows 11 Enterprise Build 26200
**Iterations**: 5 per benchmark (clones: 3)

| Build | Version | Framework | Config |
|-------|---------|-----------|--------|
| Production |  | .NET Framework 4.7.1 | Installed (C:\Program Files\GVFS) |
| .NET 10 | GVFS 0.2.173.2 | .NET 10.0 (self-contained) | Build output (Release) |

## Results

| Benchmark | Production (ms) | .NET 10 (ms) | Delta | Change |
|-----------|---------------:|-------------:|------:|--------|| Startup (gvfs version) | 52.9 ± 1.6 | 15.3 ± 0.7 | -37.6 (-71.1%) | **faster** |
| Clone (small repo, no mount) | 5264.5 ± 202.2 | 5284.8 ± 130.3 | +20.3 (+0.4%) | ~same |
| Mount (small repo) | 4841.8 ± 317 | 4102.3 ± 51.5 | -739.5 (-15.3%) | **faster** |
| Status (pipe roundtrip) | 360.9 ± 33.2 | 182.1 ± 12.3 | -178.8 (-49.5%) | **faster** |
| Prefetch (*.md) | 1106.8 ± 514.3 | 857 ± 691.9 | -249.8 (-22.6%) | **faster** |
| Git Status | 189.5 ± 9.5 | 133.9 ± 3.8 | -55.6 (-29.3%) | **faster** |
| Git Log (-100) | 140.5 ± 4.8 | 124.8 ± 8.2 | -15.7 (-11.2%) | **faster** |
| Dir Enumeration (ProjFS) | 191.9 ± 80.7 | 99.6 ± 46.9 | -92.3 (-48.1%) | **faster** |
| File Read (hydration) | 10.3 ± 16.7 | 2.8 ± 1 | -7.5 (-72.8%) | **faster** |
| Unmount | 887.8 ± 520.1 | 819.3 ± 657.7 | -68.5 (-7.7%) | **faster** |
| Large Repo Git Status | 29.6 ± 1 | N/A | N/A | baseline |

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
  Production: min=51.2 avg=52.9 max=54.8 stddev=1.6
  .NET 10:    min=14.4 avg=15.3 max=16.3 stddev=0.7

Clone (small repo, no mount):
  Production: min=5108.1 avg=5264.5 max=5492.8 stddev=202.2
  .NET 10:    min=5206.1 avg=5284.8 max=5435.2 stddev=130.3

Mount (small repo):
  Production: min=4505 avg=4841.8 max=5226.4 stddev=317
  .NET 10:    min=4020.7 avg=4102.3 max=4159 stddev=51.5

Status (pipe roundtrip):
  Production: min=333.1 avg=360.9 max=425.8 stddev=33.2
  .NET 10:    min=171.9 avg=182.1 max=212.9 stddev=12.3

Prefetch (*.md):
  Production: min=800.6 avg=1106.8 max=1700.7 stddev=514.3
  .NET 10:    min=413.9 avg=857 max=1654.3 stddev=691.9

Git Status:
  Production: min=181.9 avg=189.5 max=205.6 stddev=9.5
  .NET 10:    min=131.1 avg=133.9 max=140.3 stddev=3.8

Git Log (-100):
  Production: min=134.7 avg=140.5 max=145.5 stddev=4.8
  .NET 10:    min=116 avg=124.8 max=137.2 stddev=8.2

Dir Enumeration (ProjFS):
  Production: min=147.6 avg=191.9 max=335.3 stddev=80.7
  .NET 10:    min=72.3 avg=99.6 max=182.6 stddev=46.9

File Read (hydration):
  Production: min=0.5 avg=10.3 max=29.6 stddev=16.7
  .NET 10:    min=2.1 avg=2.8 max=4 stddev=1

Unmount:
  Production: min=287.3 avg=887.8 max=1188.5 stddev=520.1
  .NET 10:    min=59.9 avg=819.3 max=1209.8 stddev=657.7

Large Repo Git Status:
  Production: min=28 avg=29.6 max=30.7 stddev=1

```

