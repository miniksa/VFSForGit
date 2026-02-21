# VFSForGit .NET 10 Migration â€” Performance Comparison

**Date**: 2026-02-20 16:37
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
| Startup (gvfs version) | 54.3 Â± 4 | 55.3 Â± 1.4 | +1 (+1.8%) | ~same |
| Clone (small repo, no mount) | 5456.7 Â± 294.7 | 7864.5 Â± 2532.3 | +2407.8 (+44.1%) | slower |
| Mount (small repo) | 4664 Â± 244.7 | 6403.2 Â± 168 | +1739.2 (+37.3%) | slower |
| Status (pipe roundtrip) | 336.6 Â± 9.5 | 335.4 Â± 5.7 | -1.2 (-0.4%) | ~same |
| Prefetch (*.md) | 983.9 Â± 512.9 | 1261.6 Â± 832.6 | +277.7 (+28.2%) | slower |
| Git Status | 174.9 Â± 19.6 | 263 Â± 121.6 | +88.1 (+50.4%) | slower |
| Git Log (-100) | 121.1 Â± 1.5 | 188 Â± 27.9 | +66.9 (+55.2%) | slower |
| Dir Enumeration (ProjFS) | 240.8 Â± 75.4 | 213.7 Â± 74.4 | -27.1 (-11.3%) | **faster** |
| File Read (hydration) | 61.1 Â± 104.5 | 8.8 Â± 13.9 | -52.3 (-85.6%) | **faster** |
| Unmount | 777.3 Â± 434.7 | 798.4 Â± 526.3 | +21.1 (+2.7%) | ~same |
| Large Repo Git Status | 28.8 Â± 1.2 | N/A | N/A | baseline |

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
  Production: min=51.3 avg=54.3 max=61.2 stddev=4
  .NET 10:    min=53.7 avg=55.3 max=57 stddev=1.4

Clone (small repo, no mount):
  Production: min=5140.3 avg=5456.7 max=5723.6 stddev=294.7
  .NET 10:    min=5450.2 avg=7864.5 max=10500.3 stddev=2532.3

Mount (small repo):
  Production: min=4498.5 avg=4664 max=5094.7 stddev=244.7
  .NET 10:    min=6121.1 avg=6403.2 max=6563.1 stddev=168

Status (pipe roundtrip):
  Production: min=327.3 avg=336.6 max=359 stddev=9.5
  .NET 10:    min=329.4 avg=335.4 max=343.3 stddev=5.7

Prefetch (*.md):
  Production: min=686.8 avg=983.9 max=1576.1 stddev=512.9
  .NET 10:    min=732.8 avg=1261.6 max=2221.4 stddev=832.6

Git Status:
  Production: min=160.1 avg=174.9 max=208.5 stddev=19.6
  .NET 10:    min=206.4 avg=263 max=480.5 stddev=121.6

Git Log (-100):
  Production: min=119.6 avg=121.1 max=123.5 stddev=1.5
  .NET 10:    min=174.2 avg=188 max=237.8 stddev=27.9

Dir Enumeration (ProjFS):
  Production: min=181 avg=240.8 max=359.4 stddev=75.4
  .NET 10:    min=171.2 avg=213.7 max=346.4 stddev=74.4

File Read (hydration):
  Production: min=0.6 avg=61.1 max=181.8 stddev=104.5
  .NET 10:    min=0.7 avg=8.8 max=24.9 stddev=13.9

Unmount:
  Production: min=312.5 avg=777.3 max=1173.8 stddev=434.7
  .NET 10:    min=192.1 avg=798.4 max=1137 stddev=526.3

Large Repo Git Status:
  Production: min=27.7 avg=28.8 max=30.6 stddev=1.2

```

