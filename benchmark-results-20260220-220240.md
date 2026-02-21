# VFSForGit .NET 10 Migration â€” Performance Comparison

**Date**: 2026-02-20 22:06
**Machine**: NIKSA-13900 (Intel64 Family 6 Model 183 Stepping 1, GenuineIntel)
**OS**: Microsoft Windows 11 Enterprise Build 26200
**Iterations**: 5 per benchmark (clones: 3)

| Build | Version | Framework | Config |
|-------|---------|-----------|--------|
| Production |  | .NET Framework 4.7.1 | Installed (C:\Program Files\GVFS) |
| .NET 10 |  | .NET 10.0 (self-contained) | Build output (Release) |

## Results

| Benchmark | Production (ms) | .NET 10 (ms) | Delta | Change |
|-----------|---------------:|-------------:|------:|--------|| Startup (gvfs version) | 53.2 Â± 1.3 | 50.5 Â± 0.5 | -2.7 (-5.1%) | **faster** |
| Clone (small repo, no mount) | 5029.8 Â± 193.4 | 5714.5 Â± 270.8 | +684.7 (+13.6%) | slower |
| Mount (small repo) | 4491.8 Â± 80.3 | 6278.7 Â± 243 | +1786.9 (+39.8%) | slower |
| Status (pipe roundtrip) | 343.2 Â± 20.1 | 291.5 Â± 6.7 | -51.7 (-15.1%) | **faster** |
| Prefetch (*.md) | 1021.5 Â± 463.6 | 1204 Â± 853.1 | +182.5 (+17.9%) | slower |
| Git Status | 191.3 Â± 14 | 222.1 Â± 4.9 | +30.8 (+16.1%) | slower |
| Git Log (-100) | 133.4 Â± 2.5 | 192 Â± 13.2 | +58.6 (+43.9%) | slower |
| Dir Enumeration (ProjFS) | 243.8 Â± 80.3 | 215.4 Â± 62.1 | -28.4 (-11.6%) | **faster** |
| File Read (hydration) | 8.1 Â± 12.7 | 2.8 Â± 3.5 | -5.3 (-65.4%) | **faster** |
| Unmount | 895.5 Â± 522 | 876.6 Â± 598.3 | -18.9 (-2.1%) | ~same |
| Large Repo Git Status | 31.1 Â± 1.9 | N/A | N/A | baseline |

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
  Production: min=51.4 avg=53.2 max=54.8 stddev=1.3
  .NET 10:    min=49.7 avg=50.5 max=51.1 stddev=0.5

Clone (small repo, no mount):
  Production: min=4840.9 avg=5029.8 max=5227.5 stddev=193.4
  .NET 10:    min=5411.5 avg=5714.5 max=5932.9 stddev=270.8

Mount (small repo):
  Production: min=4417.7 avg=4491.8 max=4622.8 stddev=80.3
  .NET 10:    min=5851.5 avg=6278.7 max=6462.7 stddev=243

Status (pipe roundtrip):
  Production: min=331.5 avg=343.2 max=397.7 stddev=20.1
  .NET 10:    min=285.4 avg=291.5 max=307 stddev=6.7

Prefetch (*.md):
  Production: min=752.5 avg=1021.5 max=1556.7 stddev=463.6
  .NET 10:    min=697.7 avg=1204 max=2188.9 stddev=853.1

Git Status:
  Production: min=180.7 avg=191.3 max=209.9 stddev=14
  .NET 10:    min=218.5 avg=222.1 max=230.3 stddev=4.9

Git Log (-100):
  Production: min=129.8 avg=133.4 max=135.6 stddev=2.5
  .NET 10:    min=182.6 avg=192 max=214.7 stddev=13.2

Dir Enumeration (ProjFS):
  Production: min=180.8 avg=243.8 max=384.4 stddev=80.3
  .NET 10:    min=182.2 avg=215.4 max=325.8 stddev=62.1

File Read (hydration):
  Production: min=0.7 avg=8.1 max=22.8 stddev=12.7
  .NET 10:    min=0.7 avg=2.8 max=6.8 stddev=3.5

Unmount:
  Production: min=294.3 avg=895.5 max=1234.3 stddev=522
  .NET 10:    min=185.9 avg=876.6 max=1236 stddev=598.3

Large Repo Git Status:
  Production: min=29.4 avg=31.1 max=33.9 stddev=1.9

```

