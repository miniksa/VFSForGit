# VFSForGit .NET 10 Migration â€” Performance Comparison

**Date**: 2026-02-20 17:11
**Machine**: NIKSA-13900 (Intel64 Family 6 Model 183 Stepping 1, GenuineIntel)
**OS**: Microsoft Windows 11 Enterprise Build 26200
**Iterations**: 5 per benchmark (clones: 3)

| Build | Version | Framework | Config |
|-------|---------|-----------|--------|
| Production |  | .NET Framework 4.7.1 | Installed (C:\Program Files\GVFS) |
| .NET 10 |  | .NET 10.0 (self-contained) | Build output (Release) |

## Results

| Benchmark | Production (ms) | .NET 10 (ms) | Delta | Change |
|-----------|---------------:|-------------:|------:|--------|| Startup (gvfs version) | 52.2 Â± 1.8 | 54.2 Â± 0.6 | +2 (+3.8%) | ~same |
| Clone (small repo, no mount) | 5647.5 Â± 456.6 | 8239.4 Â± 1554.5 | +2591.9 (+45.9%) | slower |
| Mount (small repo) | 4664.6 Â± 402.4 | 6489.5 Â± 183.9 | +1824.9 (+39.1%) | slower |
| Status (pipe roundtrip) | 333.1 Â± 5.4 | 336.7 Â± 3.9 | +3.6 (+1.1%) | ~same |
| Prefetch (*.md) | 984.9 Â± 499.7 | 1293.4 Â± 936.3 | +308.5 (+31.3%) | slower |
| Git Status | 163.2 Â± 3 | 214.7 Â± 1.5 | +51.5 (+31.6%) | slower |
| Git Log (-100) | 121.2 Â± 1.2 | 179.7 Â± 4.1 | +58.5 (+48.3%) | slower |
| Dir Enumeration (ProjFS) | 220.4 Â± 73.6 | 216.1 Â± 70.2 | -4.3 (-2%) | ~same |
| File Read (hydration) | 8.9 Â± 14.2 | 3.5 Â± 4.9 | -5.4 (-60.7%) | **faster** |
| Unmount | 993.7 Â± 623.3 | 824.2 Â± 558.4 | -169.5 (-17.1%) | **faster** |
| Large Repo Git Status | 29.2 Â± 1.1 | N/A | N/A | baseline |

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
  Production: min=50.7 avg=52.2 max=55.1 stddev=1.8
  .NET 10:    min=53.3 avg=54.2 max=55 stddev=0.6

Clone (small repo, no mount):
  Production: min=5283.9 avg=5647.5 max=6160 stddev=456.6
  .NET 10:    min=7033.8 avg=8239.4 max=9993.9 stddev=1554.5

Mount (small repo):
  Production: min=4427.5 avg=4664.6 max=5374.4 stddev=402.4
  .NET 10:    min=6175.3 avg=6489.5 max=6624.4 stddev=183.9

Status (pipe roundtrip):
  Production: min=328.5 avg=333.1 max=343.6 stddev=5.4
  .NET 10:    min=331.9 avg=336.7 max=344.9 stddev=3.9

Prefetch (*.md):
  Production: min=695.3 avg=984.9 max=1562 stddev=499.7
  .NET 10:    min=750.3 avg=1293.4 max=2374.6 stddev=936.3

Git Status:
  Production: min=161.4 avg=163.2 max=168.4 stddev=3
  .NET 10:    min=212.6 avg=214.7 max=216.4 stddev=1.5

Git Log (-100):
  Production: min=119.7 avg=121.2 max=122.7 stddev=1.2
  .NET 10:    min=176.4 avg=179.7 max=186.4 stddev=4.1

Dir Enumeration (ProjFS):
  Production: min=179.5 avg=220.4 max=351.3 stddev=73.6
  .NET 10:    min=178.8 avg=216.1 max=340.6 stddev=70.2

File Read (hydration):
  Production: min=0.6 avg=8.9 max=25.4 stddev=14.2
  .NET 10:    min=0.6 avg=3.5 max=9.1 stddev=4.9

Unmount:
  Production: min=283.6 avg=993.7 max=1450.5 stddev=623.3
  .NET 10:    min=179.6 avg=824.2 max=1159.8 stddev=558.4

Large Repo Git Status:
  Production: min=28 avg=29.2 max=30.8 stddev=1.1

```

