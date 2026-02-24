<#
.SYNOPSIS
    VFSForGit .NET Framework 4.7.1 vs .NET 10 NativeAOT Performance Comparison

.DESCRIPTION
    Benchmarks common GVFS operations against two builds:
    - Production: .NET Framework 4.7.1 (from installer or backup)
    - .NET 10:    NativeAOT build from the migration branch

    Includes both small-repo micro-benchmarks and large-repo (OS) hydration
    benchmarks that exercise the HTTP stack (WinHttpHandler vs original).

    Must be run as Administrator (service install requires elevation).

.PARAMETER Iterations
    Number of iterations per benchmark (default: 5)

.PARAMETER TestRepo
    URL of the test repo to clone (default: ForTests repo)

.PARAMETER LargeRepo
    URL of the OS repo for clone/hydration tests (default: os.2020)

.PARAMETER LargeRepoBranch
    Branch for large repo clone (default: official/ge_current_directes_corebuild)

.PARAMETER CacheServer
    Cache server URL (default: global traffic manager)

.PARAMETER OutputFile
    Path to write the markdown report (default: benchmark-results.md)

.PARAMETER SkipSmallRepo
    Skip small repo benchmarks and only run large repo tests

.PARAMETER SkipLargeRepo
    Skip large repo benchmarks and only run small repo tests
#>
param(
    [int]$Iterations = 5,
    [string]$TestRepo = "https://gvfs.visualstudio.com/ci/_git/ForTests",
    [string]$TestBranch = "FunctionalTests/20201014",
    [string]$LargeRepoUrl = "https://dev.azure.com/microsoft/os/_git/os.2020",
    [string]$LargeRepoBranch = "official/ge_current_directes_corebuild",
    [string]$CacheServer = "",
    [string]$OutputFile = "D:\src\VFSForGit\benchmark-results-$(Get-Date -Format 'yyyyMMdd-HHmmss').md",
    [switch]$UsePublished,
    [switch]$SkipSmallRepo,
    [switch]$SkipLargeRepo
)

$ErrorActionPreference = "Continue"

# --- Configuration ---
$prodDir = "D:\src\out\gvfs-production"
$aotDir  = "D:\src\out\gvfs-aot-layout"
$gvfsInstall = "C:\Program Files\GVFS"
$prodGVFS = "$gvfsInstall\GVFS.exe"
$net10GVFS = "$gvfsInstall\GVFS.exe"  # Same path — we swap the installed files
$net10Label = ".NET 10 (NativeAOT)"
$git = "C:\Program Files\Git\cmd\git.exe"
$cloneRoot = "C:\Repos\GVFSBenchmark"
$largeCloneRoot = "D:\"
$cacheArg = if ($CacheServer) { @("--cache-server-url", $CacheServer) } else { @() }

# Verify both builds exist
if (-not (Test-Path "$prodDir\GVFS.exe")) { throw "Production GVFS backup not found at $prodDir" }
if (-not (Test-Path "$aotDir\GVFS.exe"))  { throw "AOT GVFS build not found at $aotDir" }

function Install-GVFSBuild {
    param([string]$SourceDir, [string]$Label)
    Stop-Process -Name "GVFS*" -Force -EA 0
    Start-Sleep 2
    Copy-Item "$SourceDir\*" $gvfsInstall -Recurse -Force
    Start-Service GVFS.Service -EA 0
    Start-Sleep 1
    Write-Host "  Installed $Label`: $(& $prodGVFS version 2>&1)"
}

Write-Host "=== VFSForGit Performance Benchmark ===" -ForegroundColor Cyan
Write-Host "Production: $prodDir"
Write-Host "AOT:        $aotDir"
Write-Host "Iterations: $Iterations"
Write-Host ""

# --- Helper Functions ---
function Measure-Operation {
    param(
        [string]$Name,
        [scriptblock]$Setup,
        [scriptblock]$Operation,
        [scriptblock]$Cleanup,
        [int]$Iterations = 5
    )

    $times = @()
    for ($i = 0; $i -lt $Iterations; $i++) {
        if ($Setup) { & $Setup | Out-Null }
        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        & $Operation | Out-Null
        $sw.Stop()
        $times += $sw.Elapsed.TotalMilliseconds
        if ($Cleanup) { & $Cleanup | Out-Null }
        Write-Host "  [$Name] Iteration $($i+1): $([math]::Round($sw.Elapsed.TotalMilliseconds, 1)) ms"
    }

    return [PSCustomObject]@{
        Name = $Name
        Min = [math]::Round(($times | Measure-Object -Minimum).Minimum, 1)
        Max = [math]::Round(($times | Measure-Object -Maximum).Maximum, 1)
        Avg = [math]::Round(($times | Measure-Object -Average).Average, 1)
        Median = [math]::Round(($times | Sort-Object)[[math]::Floor($times.Count / 2)], 1)
        StdDev = if ($times.Count -gt 1) {
            $avg = ($times | Measure-Object -Average).Average
            [math]::Round([math]::Sqrt(($times | ForEach-Object { ($_ - $avg) * ($_ - $avg) } | Measure-Object -Sum).Sum / ($times.Count - 1)), 1)
        } else { 0 }
    }
}

$allResults = @()

# ============================================================
# Benchmark 1: Startup Time (gvfs version)
# ============================================================
if (-not $SkipSmallRepo) {
Write-Host "`n--- Benchmark 1: Startup Time (gvfs version) ---" -ForegroundColor Yellow

Install-GVFSBuild -SourceDir $prodDir -Label "Production"
$r1_prod = Measure-Operation -Name "Production" -Iterations $Iterations -Operation {
    & $prodGVFS version 2>&1
}

Install-GVFSBuild -SourceDir $aotDir -Label "AOT"
$r1_net10 = Measure-Operation -Name ".NET 10" -Iterations $Iterations -Operation {
    & $net10GVFS version 2>&1
}

$allResults += [PSCustomObject]@{ Benchmark = "Startup (gvfs version)"; Prod = $r1_prod; Net10 = $r1_net10 }

# ============================================================
# Benchmark 2: Clone (small test repo)
# ============================================================
Write-Host "`n--- Benchmark 2: Clone (small test repo, no mount) ---" -ForegroundColor Yellow

$cloneIter = [math]::Min($Iterations, 3)  # Clones are slow, limit iterations

Install-GVFSBuild -SourceDir $prodDir -Label "Production"
$r2_prod = Measure-Operation -Name "Production" -Iterations $cloneIter `
    -Setup { Remove-Item "$cloneRoot\prod" -Recurse -Force -EA 0 } `
    -Operation { & $prodGVFS clone $TestRepo "$cloneRoot\prod" --branch $TestBranch --no-mount 2>&1 } `
    -Cleanup { & $prodGVFS unmount "$cloneRoot\prod" --skip-wait-for-lock 2>&1; Start-Sleep 2; Remove-Item "$cloneRoot\prod" -Recurse -Force -EA 0 }

Install-GVFSBuild -SourceDir $aotDir -Label "AOT"
$r2_net10 = Measure-Operation -Name ".NET 10" -Iterations $cloneIter `
    -Setup { Remove-Item "$cloneRoot\net10" -Recurse -Force -EA 0 } `
    -Operation { & $net10GVFS clone $TestRepo "$cloneRoot\net10" --branch $TestBranch --no-mount 2>&1 } `
    -Cleanup { & $net10GVFS unmount "$cloneRoot\net10" --skip-wait-for-lock 2>&1; Start-Sleep 2; Remove-Item "$cloneRoot\net10" -Recurse -Force -EA 0 }

$allResults += [PSCustomObject]@{ Benchmark = "Clone (small repo, no mount)"; Prod = $r2_prod; Net10 = $r2_net10 }

# ============================================================
# Benchmark 3: Mount (small test repo)
# ============================================================
Write-Host "`n--- Benchmark 3: Mount + Unmount (small test repo) ---" -ForegroundColor Yellow

# Pre-clone for mount tests
Install-GVFSBuild -SourceDir $prodDir -Label "Production"
Remove-Item "$cloneRoot\mount_prod" -Recurse -Force -EA 0
& $prodGVFS clone $TestRepo "$cloneRoot\mount_prod" --branch $TestBranch --no-mount 2>&1 | Out-Null
Install-GVFSBuild -SourceDir $aotDir -Label "AOT"
Remove-Item "$cloneRoot\mount_net10" -Recurse -Force -EA 0
& $net10GVFS clone $TestRepo "$cloneRoot\mount_net10" --branch $TestBranch --no-mount 2>&1 | Out-Null

Install-GVFSBuild -SourceDir $prodDir -Label "Production"

$r3_prod = Measure-Operation -Name "Production" -Iterations $Iterations `
    -Operation { & $prodGVFS mount "$cloneRoot\mount_prod" 2>&1 } `
    -Cleanup { & $prodGVFS unmount "$cloneRoot\mount_prod" 2>&1; Start-Sleep 2 }

$r3_net10 = Measure-Operation -Name ".NET 10" -Iterations $Iterations `
    -Setup { Install-GVFSBuild -SourceDir $aotDir -Label "AOT" } `
    -Operation { & $net10GVFS mount "$cloneRoot\mount_net10" 2>&1 } `
    -Cleanup { & $net10GVFS unmount "$cloneRoot\mount_net10" 2>&1; Start-Sleep 2 }

$allResults += [PSCustomObject]@{ Benchmark = "Mount (small repo)"; Prod = $r3_prod; Net10 = $r3_net10 }

# ============================================================
# Benchmark 4: Status (pipe roundtrip)
# ============================================================
Write-Host "`n--- Benchmark 4: Status (named pipe roundtrip) ---" -ForegroundColor Yellow

# Mount for status tests
Install-GVFSBuild -SourceDir $prodDir -Label "Production"
& $prodGVFS mount "$cloneRoot\mount_prod" 2>&1 | Out-Null
Install-GVFSBuild -SourceDir $aotDir -Label "AOT"
& $net10GVFS mount "$cloneRoot\mount_net10" 2>&1 | Out-Null

# Wait until both mounts are fully ready (not just process started)
$maxWait = 30
for ($w = 0; $w -lt $maxWait; $w++) {
    $prodStatus = & $prodGVFS status "$cloneRoot\mount_prod" 2>&1 | Out-String
    $net10Status = & $net10GVFS status "$cloneRoot\mount_net10" 2>&1 | Out-String
    if ($prodStatus -match "Mount status: Ready" -and $net10Status -match "Mount status: Ready") {
        Write-Host "  Both mounts ready after $($w+1)s"
        break
    }
    Start-Sleep 1
}
if ($w -eq $maxWait) { Write-Host "  WARNING: Mount readiness timeout after ${maxWait}s" -ForegroundColor Red }

$r4_prod = Measure-Operation -Name "Production" -Iterations ($Iterations * 2) -Operation {
    & $prodGVFS status "$cloneRoot\mount_prod" 2>&1
}

$r4_net10 = Measure-Operation -Name ".NET 10" -Iterations ($Iterations * 2) -Operation {
    & $net10GVFS status "$cloneRoot\mount_net10" 2>&1
}

$allResults += [PSCustomObject]@{ Benchmark = "Status (pipe roundtrip)"; Prod = $r4_prod; Net10 = $r4_net10 }

# ============================================================
# Benchmark 5: Prefetch
# ============================================================
Write-Host "`n--- Benchmark 5: Prefetch (*.md files) ---" -ForegroundColor Yellow

$r5_prod = Measure-Operation -Name "Production" -Iterations $cloneIter -Operation {
    & $prodGVFS prefetch "$cloneRoot\mount_prod" --files "*.md" 2>&1
}

$r5_net10 = Measure-Operation -Name ".NET 10" -Iterations $cloneIter -Operation {
    & $net10GVFS prefetch "$cloneRoot\mount_net10" --files "*.md" 2>&1
}

$allResults += [PSCustomObject]@{ Benchmark = "Prefetch (*.md)"; Prod = $r5_prod; Net10 = $r5_net10 }

# ============================================================
# Benchmark 6: Git Status (inside mounted repo)
# ============================================================
Write-Host "`n--- Benchmark 6: Git Status (inside mounted repo) ---" -ForegroundColor Yellow

$r6_prod = Measure-Operation -Name "Production" -Iterations $Iterations -Operation {
    & $git -C "$cloneRoot\mount_prod\src" status 2>&1
}

$r6_net10 = Measure-Operation -Name ".NET 10" -Iterations $Iterations -Operation {
    & $git -C "$cloneRoot\mount_net10\src" status 2>&1
}

$allResults += [PSCustomObject]@{ Benchmark = "Git Status"; Prod = $r6_prod; Net10 = $r6_net10 }

# ============================================================
# Benchmark 7: Git Log
# ============================================================
Write-Host "`n--- Benchmark 7: Git Log (last 100 commits) ---" -ForegroundColor Yellow

$r7_prod = Measure-Operation -Name "Production" -Iterations $Iterations -Operation {
    & $git -C "$cloneRoot\mount_prod\src" log --oneline -100 2>&1
}

$r7_net10 = Measure-Operation -Name ".NET 10" -Iterations $Iterations -Operation {
    & $git -C "$cloneRoot\mount_net10\src" log --oneline -100 2>&1
}

$allResults += [PSCustomObject]@{ Benchmark = "Git Log (-100)"; Prod = $r7_prod; Net10 = $r7_net10 }

# ============================================================
# Benchmark 8: File Enumeration (ProjFS)
# ============================================================
Write-Host "`n--- Benchmark 8: Directory Enumeration (ProjFS) ---" -ForegroundColor Yellow

$r8_prod = Measure-Operation -Name "Production" -Iterations $Iterations -Operation {
    (Get-ChildItem "$cloneRoot\mount_prod\src" -Recurse -File).Count
}

$r8_net10 = Measure-Operation -Name ".NET 10" -Iterations $Iterations -Operation {
    (Get-ChildItem "$cloneRoot\mount_net10\src" -Recurse -File).Count
}

$allResults += [PSCustomObject]@{ Benchmark = "Dir Enumeration (ProjFS)"; Prod = $r8_prod; Net10 = $r8_net10 }

# ============================================================
# Benchmark 9: File Read (hydration through ProjFS)
# ============================================================
Write-Host "`n--- Benchmark 9: File Read / Hydration ---" -ForegroundColor Yellow

$r9_prod = Measure-Operation -Name "Production" -Iterations $cloneIter -Operation {
    Get-Content "$cloneRoot\mount_prod\src\Readme.md" -Raw 2>&1
}

$r9_net10 = Measure-Operation -Name ".NET 10" -Iterations $cloneIter -Operation {
    Get-Content "$cloneRoot\mount_net10\src\Readme.md" -Raw 2>&1
}

$allResults += [PSCustomObject]@{ Benchmark = "File Read (hydration)"; Prod = $r9_prod; Net10 = $r9_net10 }

# ============================================================
# Benchmark 10: Unmount
# ============================================================
Write-Host "`n--- Benchmark 10: Unmount ---" -ForegroundColor Yellow

$r10_prod = Measure-Operation -Name "Production" -Iterations $cloneIter `
    -Setup { & $prodGVFS mount "$cloneRoot\mount_prod" 2>&1; Start-Sleep 2 } `
    -Operation { & $prodGVFS unmount "$cloneRoot\mount_prod" 2>&1 }

$r10_net10 = Measure-Operation -Name ".NET 10" -Iterations $cloneIter `
    -Setup { Install-GVFSBuild -SourceDir $aotDir -Label "AOT"; & $net10GVFS mount "$cloneRoot\mount_net10" 2>&1; Start-Sleep 2 } `
    -Operation { & $net10GVFS unmount "$cloneRoot\mount_net10" 2>&1 }

$allResults += [PSCustomObject]@{ Benchmark = "Unmount"; Prod = $r10_prod; Net10 = $r10_net10 }

} # end SkipSmallRepo

# ============================================================
# Large Repo Benchmarks (OS repo — exercises HTTP stack heavily)
# ============================================================
if (-not $SkipLargeRepo) {
    Write-Host "`n--- Benchmark 11: Large Repo Clone + Prefetch ---" -ForegroundColor Yellow
    Write-Host "  This exercises the HTTP download stack (WinHttpHandler vs SocketsHttpHandler)"

    # --- Production clone ---
    Install-GVFSBuild -SourceDir $prodDir -Label "Production"
    $prodLargePath = "$largeCloneRoot\os_bench_prod"
    $prodLargeCache = "$largeCloneRoot\os_bench_prod_cache"

    Remove-Item $prodLargePath -Recurse -Force -EA 0
    Remove-Item $prodLargeCache -Recurse -Force -EA 0

    $r11_prod = Measure-Operation -Name "Production" -Iterations 1 -Operation {
        & $prodGVFS clone $LargeRepoUrl $prodLargePath --branch $LargeRepoBranch --local-cache-path $prodLargeCache @cacheArg 2>&1
    }

    # --- AOT clone ---
    & $prodGVFS unmount $prodLargePath 2>&1 | Out-Null
    Install-GVFSBuild -SourceDir $aotDir -Label "AOT"
    $aotLargePath = "$largeCloneRoot\os_bench_aot"
    $aotLargeCache = "$largeCloneRoot\os_bench_aot_cache"

    Remove-Item $aotLargePath -Recurse -Force -EA 0
    Remove-Item $aotLargeCache -Recurse -Force -EA 0

    $r11_net10 = Measure-Operation -Name ".NET 10" -Iterations 1 -Operation {
        & $net10GVFS clone $LargeRepoUrl $aotLargePath --branch $LargeRepoBranch --local-cache-path $aotLargeCache @cacheArg 2>&1
    }

    $allResults += [PSCustomObject]@{ Benchmark = "Large Repo Clone+Prefetch"; Prod = $r11_prod; Net10 = $r11_net10 }

    # ============================================================
    # Benchmark 12: Hydration / File Access (HTTP object downloads)
    # ============================================================
    Write-Host "`n--- Benchmark 12: Hydration — Directory Listing (HTTP object downloads) ---" -ForegroundColor Yellow
    Write-Host "  This is the critical benchmark: per-request HTTP latency under load"

    # Mount both
    & $prodGVFS unmount $aotLargePath 2>&1 | Out-Null
    Install-GVFSBuild -SourceDir $prodDir -Label "Production"
    & $prodGVFS mount $prodLargePath 2>&1 | Out-Null
    Start-Sleep 3

    $hydrationPath = "src\OneCore\Base\wil\Containment"

    $r12_prod = Measure-Operation -Name "Production" -Iterations 1 -Operation {
        (Get-ChildItem "$prodLargePath\$hydrationPath" -Recurse -File -EA 0).Count
    }

    & $prodGVFS unmount $prodLargePath 2>&1 | Out-Null
    Install-GVFSBuild -SourceDir $aotDir -Label "AOT"
    & $net10GVFS mount $aotLargePath 2>&1 | Out-Null
    Start-Sleep 3

    $r12_net10 = Measure-Operation -Name ".NET 10" -Iterations 1 -Operation {
        (Get-ChildItem "$aotLargePath\$hydrationPath" -Recurse -File -EA 0).Count
    }

    $allResults += [PSCustomObject]@{ Benchmark = "Hydration (dir listing)"; Prod = $r12_prod; Net10 = $r12_net10 }

    # ============================================================
    # Benchmark 13: HTTP Response Time Analysis
    # ============================================================
    Write-Host "`n--- Benchmark 13: HTTP Response Time Analysis (from mount logs) ---" -ForegroundColor Yellow

    # Analyze the mount logs from the hydration test
    $prodMountLog = Get-ChildItem "$prodLargePath\.gvfs\logs\gvfs_mount_process_*.log" -EA 0 | Sort-Object Name -Descending | Select-Object -First 1
    $aotMountLog  = Get-ChildItem "$aotLargePath\.gvfs\logs\gvfs_mount_process_*.log" -EA 0 | Sort-Object Name -Descending | Select-Object -First 1

    if ($prodMountLog -and $aotMountLog) {
        $prodTimes = @(); Select-String "responseWaitTimeMS" $prodMountLog.FullName | ForEach-Object { if ($_.Line -match '"responseWaitTimeMS":"([^"]+)"') { $prodTimes += [double]$Matches[1] } }
        $aotTimes  = @(); Select-String "responseWaitTimeMS" $aotMountLog.FullName  | ForEach-Object { if ($_.Line -match '"responseWaitTimeMS":"([^"]+)"') { $aotTimes  += [double]$Matches[1] } }

        Write-Host "  Production HTTP: n=$($prodTimes.Count) sum=$([math]::Round(($prodTimes | Measure-Object -Sum).Sum))ms max=$([math]::Round(($prodTimes | Measure-Object -Maximum).Maximum, 1))ms"
        Write-Host "  AOT HTTP:        n=$($aotTimes.Count)  sum=$([math]::Round(($aotTimes  | Measure-Object -Sum).Sum))ms max=$([math]::Round(($aotTimes  | Measure-Object -Maximum).Maximum, 1))ms"

        # Store as pseudo-benchmark results for the report
        $r13_prod  = [PSCustomObject]@{ Name="Production"; Min=0; Max=0; Avg=[math]::Round(($prodTimes | Measure-Object -Sum).Sum); Median=0; StdDev=0 }
        $r13_net10 = [PSCustomObject]@{ Name=".NET 10";    Min=0; Max=0; Avg=[math]::Round(($aotTimes  | Measure-Object -Sum).Sum); Median=0; StdDev=0 }
        $allResults += [PSCustomObject]@{ Benchmark = "HTTP Total Wait (ms)"; Prod = $r13_prod; Net10 = $r13_net10 }
    }

    # Cleanup large repos
    Write-Host "`n--- Cleanup large repos ---"
    & $net10GVFS unmount $aotLargePath 2>&1 | Out-Null
    Stop-Process -Name "GVFS*" -Force -EA 0
    Start-Sleep 3
    Remove-Item $prodLargePath -Recurse -Force -EA 0
    Remove-Item $prodLargeCache -Recurse -Force -EA 0
    Remove-Item $aotLargePath -Recurse -Force -EA 0
    Remove-Item $aotLargeCache -Recurse -Force -EA 0
}

# ============================================================
# Cleanup
# ============================================================
Write-Host "`n--- Cleanup ---" -ForegroundColor Yellow
Stop-Process -Name "GVFS*" -Force -EA 0
Start-Sleep 3
Remove-Item "$cloneRoot" -Recurse -Force -EA 0

# Restore production as the installed version
Install-GVFSBuild -SourceDir $prodDir -Label "Production (restored)"

# ============================================================
# Generate Report
# ============================================================
Write-Host "`n--- Generating Report ---" -ForegroundColor Yellow

$prodVersion = & "$prodDir\GVFS.exe" version 2>$null | Select-Object -First 1
$net10Version = & "$aotDir\GVFS.exe" version 2>$null | Select-Object -First 1

$report = @"
# VFSForGit .NET 10 Migration — Performance Comparison

**Date**: $(Get-Date -Format "yyyy-MM-dd HH:mm")
**Machine**: $env:COMPUTERNAME ($env:PROCESSOR_IDENTIFIER)
**OS**: $(Get-CimInstance Win32_OperatingSystem | Select-Object -ExpandProperty Caption) Build $([Environment]::OSVersion.Version.Build)
**Iterations**: $Iterations per benchmark (clones: $cloneIter)

| Build | Version | Framework | Config |
|-------|---------|-----------|--------|
| Production | $prodVersion | .NET Framework 4.7.1 | Installed (C:\Program Files\GVFS) |
| .NET 10 | $net10Version | .NET 10.0 (self-contained) | Build output (Release) |

## Results

| Benchmark | Production (ms) | .NET 10 (ms) | Delta | Change |
|-----------|---------------:|-------------:|------:|--------|
"@

foreach ($r in $allResults) {
    if ($r.Net10) {
        $delta = $r.Net10.Avg - $r.Prod.Avg
        $pct = if ($r.Prod.Avg -gt 0) { [math]::Round($delta / $r.Prod.Avg * 100, 1) } else { 0 }
        $change = if ($pct -lt -5) { "**faster**" } elseif ($pct -gt 5) { "slower" } else { "~same" }
        $sign = if ($delta -gt 0) { "+" } else { "" }
        $report += "| $($r.Benchmark) | $($r.Prod.Avg) ± $($r.Prod.StdDev) | $($r.Net10.Avg) ± $($r.Net10.StdDev) | $sign$([math]::Round($delta, 1)) ($sign$pct%) | $change |" + "`r`n"
    } else {
        $report += "| $($r.Benchmark) | $($r.Prod.Avg) ± $($r.Prod.StdDev) | N/A | N/A | baseline |" + "`r`n"
    }
}

$report += @"

## Details

### Methodology
- Each benchmark runs the specified number of iterations
- Times measured with `[Stopwatch]` (wall clock, includes process creation overhead)
- Clone benchmarks limited to $cloneIter iterations due to network I/O
- Status benchmarks run $($Iterations * 2) iterations for better precision
- Both builds use the same git version: $(& $git version 2>&1)
- Test repo: ``$TestRepo`` (branch: ``$TestBranch``)

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

``````
"@

foreach ($r in $allResults) {
    $report += "$($r.Benchmark):`n"
    $report += "  Production: min=$($r.Prod.Min) avg=$($r.Prod.Avg) max=$($r.Prod.Max) stddev=$($r.Prod.StdDev)`n"
    if ($r.Net10) {
        $report += "  .NET 10:    min=$($r.Net10.Min) avg=$($r.Net10.Avg) max=$($r.Net10.Max) stddev=$($r.Net10.StdDev)`n"
    }
    $report += "`n"
}

$report += "```````n"

$report | Out-File $OutputFile -Encoding utf8
Write-Host "Report written to $OutputFile" -ForegroundColor Green
Write-Host "`nDone!" -ForegroundColor Cyan
