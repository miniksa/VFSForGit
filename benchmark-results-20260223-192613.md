# VFSForGit .NET 10 Migration — Performance Comparison

**Date**: 2026-02-23 19:55
**Machine**: NIKSA-13900 (Intel64 Family 6 Model 183 Stepping 1, GenuineIntel)
**OS**: Microsoft Windows 11 Enterprise Build 26200
**Iterations**: 5 per benchmark (clones: 3)

| Build | Version | Framework | Config |
|-------|---------|-----------|--------|
| Production |  | .NET Framework 4.7.1 | Installed (C:\Program Files\GVFS) |
| .NET 10 | GVFS 1.0.26014.1 | .NET 10.0 (self-contained) | Build output (Release) |

## Results

| Benchmark | Production (ms) | .NET 10 (ms) | Delta | Change |
|-----------|---------------:|-------------:|------:|--------|| Startup (gvfs version) | 56 ± 3.2 | 37.4 ± 21.8 | -18.6 (-33.2%) | **faster** |
| Clone (small repo, no mount) | 6184.6 ± 415.4 | 8984.9 ± 1399.7 | +2800.3 (+45.3%) | slower |
| Mount (small repo) | 5177.5 ± 335.9 | 7331.1 ± 672.3 | +2153.6 (+41.6%) | slower |
| Status (pipe roundtrip) | 3201.3 ± 31.2 | 188.7 ± 26.2 | -3012.6 (-94.1%) | **faster** |
| Prefetch (*.md) | 1349.8 ± 1488.9 | 665 ± 365.9 | -684.8 (-50.7%) | **faster** |
| Git Status | 3073.2 ± 8 | 142.9 ± 18.9 | -2930.3 (-95.4%) | **faster** |
| Git Log (-100) | 115.1 ± 1.9 | 120 ± 5.6 | +4.9 (+4.3%) | ~same |
| Dir Enumeration (ProjFS) | 2.1 ± 2.3 | 185.1 ± 69 | +183 (+8714.3%) | slower |
| File Read (hydration) | 1.6 ± 2.1 | 6.4 ± 9.9 | +4.8 (+300%) | slower |
| Unmount | 1397.1 ± 329.5 | 1166.8 ± 36 | -230.3 (-16.5%) | **faster** |
| Large Repo Clone+Prefetch | 804109.8 ± 0 | 253702.6 ± 0 | -550407.2 (-68.4%) | **faster** |
| Hydration (dir listing) | 94442.7 ± 0 | 246.4 ± 0 | -94196.3 (-99.7%) | **faster** |
| HTTP Total Wait (ms) | 94842 ± 0 | 313 ± 0 | -94529 (-99.7%) | **faster** |

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
  Production: min=53.5 avg=56 max=61.4 stddev=3.2
  .NET 10:    min=25.8 avg=37.4 max=76.2 stddev=21.8

Clone (small repo, no mount):
  Production: min=5706.9 avg=6184.6 max=6461.8 stddev=415.4
  .NET 10:    min=7706 avg=8984.9 max=10480.2 stddev=1399.7

Mount (small repo):
  Production: min=4864.1 avg=5177.5 max=5611.5 stddev=335.9
  .NET 10:    min=6801.1 avg=7331.1 max=8479.4 stddev=672.3

Status (pipe roundtrip):
  Production: min=3172.6 avg=3201.3 max=3252.2 stddev=31.2
  .NET 10:    min=176 avg=188.7 max=261.9 stddev=26.2

Prefetch (*.md):
  Production: min=481.7 avg=1349.8 max=3069 stddev=1488.9
  .NET 10:    min=452.8 avg=665 max=1087.5 stddev=365.9

Git Status:
  Production: min=3062.5 avg=3073.2 max=3084.6 stddev=8
  .NET 10:    min=131.6 avg=142.9 max=176.5 stddev=18.9

Git Log (-100):
  Production: min=112.7 avg=115.1 max=117.4 stddev=1.9
  .NET 10:    min=114.2 avg=120 max=128 stddev=5.6

Dir Enumeration (ProjFS):
  Production: min=1 avg=2.1 max=6.2 stddev=2.3
  .NET 10:    min=152.4 avg=185.1 max=308.5 stddev=69

File Read (hydration):
  Production: min=0.3 avg=1.6 max=4 stddev=2.1
  .NET 10:    min=0.6 avg=6.4 max=17.9 stddev=9.9

Unmount:
  Production: min=1191.9 avg=1397.1 max=1777.2 stddev=329.5
  .NET 10:    min=1125.7 avg=1166.8 max=1193.1 stddev=36

Large Repo Clone+Prefetch:
  Production: min=804109.8 avg=804109.8 max=804109.8 stddev=0
  .NET 10:    min=253702.6 avg=253702.6 max=253702.6 stddev=0

Hydration (dir listing):
  Production: min=94442.7 avg=94442.7 max=94442.7 stddev=0
  .NET 10:    min=246.4 avg=246.4 max=246.4 stddev=0

HTTP Total Wait (ms):
  Production: min=0 avg=94842 max=0 stddev=0
  .NET 10:    min=0 avg=313 max=0 stddev=0

```

