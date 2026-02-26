<#
.SYNOPSIS
    Compare GVFS branch-switch performance between production and NativeAOT builds.

.DESCRIPTION
    Runs Measure-BranchSwitch.ps1 for both the production (.NET Framework) and
    .NET 10 NativeAOT GVFS builds, then produces a side-by-side comparison report.

    Validates that the working tree is clean before and after every checkout.

    For warm-cache (existing enlistment) tests:
      - Uses the same mounted enlistment for both builds
      - Swaps GVFS binaries between runs (unmount, replace, remount)
      - Restores production GVFS after the benchmark

    For cold-cache tests:
      - Each build gets a fresh clone with a dedicated object cache
      - Requires Administrator privileges (GVFS service management)

    Must be run as Administrator when swapping GVFS binaries.

.PARAMETER TargetBranch
    The target branch to checkout.

.PARAMETER BaseBranch
    Starting branch. Default: official/ge_current_directes_corebuild

.PARAMETER RepoUrl
    GVFS-enabled repo URL. Default: os.2020

.PARAMETER CloneRoot
    Root directory for benchmark clones. Default: D:\gvfs-benchmark

.PARAMETER UseExistingEnlistment
    Use an existing mounted enlistment for both tests (warm cache).
    Specify the enlistment path (e.g., D:\os).

.PARAMETER ProdDir
    Production GVFS backup directory. Default: D:\src\out\gvfs-production

.PARAMETER AotDir
    NativeAOT GVFS build layout. Default: D:\src\out\gvfs-aot-layout

.PARAMETER Iterations
    Number of checkout iterations per build. Default: 3 for warm cache, 1 for cold.

.PARAMETER OutputFile
    Markdown report output path. Auto-generated if not specified.

.PARAMETER SkipProd
    Skip the production benchmark (use a previously-saved JSON result).

.PARAMETER SkipAot
    Skip the AOT benchmark (use a previously-saved JSON result).

.PARAMETER ProdResultFile
    Path to a previously-saved production benchmark JSON (used with -SkipProd).

.PARAMETER AotResultFile
    Path to a previously-saved AOT benchmark JSON (used with -SkipAot).

.PARAMETER Cleanup
    Remove benchmark clones after completion.

.EXAMPLE
    # Warm-cache comparison on existing enlistment (simplest)
    .\Compare-BranchSwitchPerformance.ps1 -TargetBranch "official/ge_current_directes_corebuild_dev4" `
        -UseExistingEnlistment "D:\os"

    # Full cold-cache comparison with fresh clones (most rigorous)
    .\Compare-BranchSwitchPerformance.ps1 -TargetBranch "official/ge_current_directes_corebuild_dev4"
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$TargetBranch,

    [string]$BaseBranch = "official/ge_current_directes_corebuild",

    [string]$RepoUrl = "https://dev.azure.com/microsoft/os/_git/os.2020",

    [string]$CloneRoot = "D:\gvfs-benchmark",

    [string]$UseExistingEnlistment,

    [string]$ProdDir = "D:\src\out\gvfs-production",
    [string]$AotDir  = "D:\src\out\gvfs-aot-layout",

    [int]$Iterations = 0,

    [string]$OutputFile,

    [switch]$SkipProd,
    [switch]$SkipAot,

    [string]$ProdResultFile,
    [string]$AotResultFile,

    [switch]$Cleanup
)

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$measureScript = Join-Path $scriptDir "Measure-BranchSwitch.ps1"

$gvfsInstallDir = "C:\Program Files\GVFS"
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"

if (-not $OutputFile) {
    $OutputFile = "D:\src\VFSForGit\benchmark-comparison-$timestamp.md"
}

# Default iterations: 3 for warm cache, 1 for cold
if ($Iterations -eq 0) {
    $Iterations = if ($UseExistingEnlistment) { 3 } else { 1 }
}

# --- Helpers ---
function Test-Admin {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]$identity
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Install-GVFSBuild {
    param([string]$SourceDir, [string]$Label)

    Write-Host "  Installing $Label build from $SourceDir..." -ForegroundColor Yellow

    # Stop GVFS service
    sc.exe stop GVFS.Service 2>&1 | Out-Null
    for ($i = 0; $i -lt 30; $i++) {
        $svc = Get-Service GVFS.Service -EA 0
        if (-not $svc -or $svc.Status -eq 'Stopped') { break }
        Start-Sleep 1
    }

    # Kill any remaining GVFS processes
    Get-Process GVFS.Mount, GVFS.Service, GVFS.Service.UI -EA 0 | Stop-Process -Force -EA 0
    Start-Sleep 2

    # Copy files, retry if locked
    $retries = 5
    for ($r = 0; $r -lt $retries; $r++) {
        try {
            Copy-Item "$SourceDir\*" $gvfsInstallDir -Recurse -Force -ErrorAction Stop
            break
        } catch {
            Write-Host "    Copy attempt $($r+1) failed: $($_.Exception.Message)" -ForegroundColor Yellow
            # Kill anything that might hold locks
            Get-Process GVFS*, e_sqlite3 -EA 0 | Stop-Process -Force -EA 0
            Start-Sleep 3
            if ($r -eq $retries - 1) { throw }
        }
    }

    # Restart service
    sc.exe start GVFS.Service 2>&1 | Out-Null
    Start-Sleep 3

    $version = & "$gvfsInstallDir\GVFS.exe" version 2>&1 | Select-Object -First 1
    Write-Host "  Installed: $version" -ForegroundColor Green
}

# --- Validation ---
if (-not (Test-Admin)) {
    throw "This script requires Administrator privileges for swapping GVFS binaries. Run as admin."
}

if (-not $SkipProd -and -not (Test-Path "$ProdDir\GVFS.exe")) {
    throw "Production GVFS backup not found at $ProdDir. Copy your production GVFS install there first."
}
if (-not $SkipAot -and -not (Test-Path "$AotDir\GVFS.exe")) {
    throw "NativeAOT GVFS build not found at $AotDir. Build it first with publish-aot.ps1."
}

# --- Banner ---
Write-Host ""
Write-Host "╔═══════════════════════════════════════════════════════════════╗" -ForegroundColor Magenta
Write-Host "║  GVFS Branch Switch Performance Comparison                   ║" -ForegroundColor Magenta
Write-Host "╠═══════════════════════════════════════════════════════════════╣" -ForegroundColor Magenta
Write-Host "║  Base Branch:    $($BaseBranch.Substring(0, [math]::Min(44, $BaseBranch.Length)).PadRight(44))║" -ForegroundColor Magenta
Write-Host "║  Target Branch:  $($TargetBranch.Substring(0, [math]::Min(44, $TargetBranch.Length)).PadRight(44))║" -ForegroundColor Magenta
Write-Host "║  Iterations:     $($Iterations.ToString().PadRight(44))║" -ForegroundColor Magenta
if ($UseExistingEnlistment) {
    Write-Host "║  Mode:           Existing enlistment (warm cache)            ║" -ForegroundColor Magenta
    Write-Host "║  Enlistment:     $($UseExistingEnlistment.PadRight(44))║" -ForegroundColor Magenta
} else {
    Write-Host "║  Mode:           Fresh clone (cold cache)                    ║" -ForegroundColor Magenta
}
Write-Host "╚═══════════════════════════════════════════════════════════════╝" -ForegroundColor Magenta
Write-Host ""

# --- Phase 1: Production Benchmark ---
$prodResult = $null
if ($SkipProd -and $ProdResultFile) {
    Write-Host "  Loading previous production result from $ProdResultFile" -ForegroundColor Yellow
    $prodResult = Get-Content $ProdResultFile -Raw | ConvertFrom-Json
} elseif (-not $SkipProd) {
    Write-Host "═══ Phase 1: Production (.NET Framework) Benchmark ═══" -ForegroundColor Yellow
    Write-Host ""

    if ($UseExistingEnlistment) {
        # Unmount, swap to production, remount
        Write-Host "  Ensuring production GVFS is installed..."
        & "$gvfsInstallDir\GVFS.exe" unmount $UseExistingEnlistment 2>&1 | Out-Null
        Start-Sleep 3
        Install-GVFSBuild -SourceDir $ProdDir -Label "Production"
        & "$gvfsInstallDir\GVFS.exe" mount $UseExistingEnlistment 2>&1 | Out-Null
        Start-Sleep 5
    } else {
        Install-GVFSBuild -SourceDir $ProdDir -Label "Production"
    }

    $prodResultFile = "D:\src\VFSForGit\benchmark-branchswitch-Production-$timestamp.json"
    $prodArgs = @{
        TargetBranch = $TargetBranch
        BaseBranch   = $BaseBranch
        Label        = "Production"
        OutputFile   = $prodResultFile
        Iterations   = $Iterations
    }

    if ($UseExistingEnlistment) {
        $prodArgs.ExistingEnlistment = $UseExistingEnlistment
        $prodArgs.GvfsExe = "$gvfsInstallDir\GVFS.exe"
    } else {
        $prodArgs.RepoUrl = $RepoUrl
        $prodArgs.CloneRoot = $CloneRoot
        $prodArgs.GvfsExe = "$gvfsInstallDir\GVFS.exe"
    }

    & $measureScript @prodArgs
    $prodResult = Get-Content $prodResultFile -Raw | ConvertFrom-Json
}

# --- Phase 2: NativeAOT Benchmark ---
$aotResult = $null
if ($SkipAot -and $AotResultFile) {
    Write-Host "  Loading previous AOT result from $AotResultFile" -ForegroundColor Yellow
    $aotResult = Get-Content $AotResultFile -Raw | ConvertFrom-Json
} elseif (-not $SkipAot) {
    Write-Host ""
    Write-Host "═══ Phase 2: NativeAOT (.NET 10) Benchmark ═══" -ForegroundColor Yellow
    Write-Host ""

    if ($UseExistingEnlistment) {
        # Unmount, swap to AOT, remount
        Write-Host "  Swapping to NativeAOT GVFS build..."
        & "$gvfsInstallDir\GVFS.exe" unmount $UseExistingEnlistment 2>&1 | Out-Null
        Start-Sleep 5
        Install-GVFSBuild -SourceDir $AotDir -Label "NativeAOT"
        & "$gvfsInstallDir\GVFS.exe" mount $UseExistingEnlistment 2>&1 | Out-Null
        Start-Sleep 5
    } else {
        Install-GVFSBuild -SourceDir $AotDir -Label "NativeAOT"
    }

    $aotResultFile = "D:\src\VFSForGit\benchmark-branchswitch-NativeAOT-$timestamp.json"
    $aotArgs = @{
        TargetBranch = $TargetBranch
        BaseBranch   = $BaseBranch
        Label        = "NativeAOT"
        OutputFile   = $aotResultFile
        Iterations   = $Iterations
    }

    if ($UseExistingEnlistment) {
        $aotArgs.ExistingEnlistment = $UseExistingEnlistment
        $aotArgs.GvfsExe = "$gvfsInstallDir\GVFS.exe"
    } else {
        $aotArgs.RepoUrl = $RepoUrl
        $aotArgs.CloneRoot = $CloneRoot
        $aotArgs.GvfsExe = "$gvfsInstallDir\GVFS.exe"
    }

    & $measureScript @aotArgs
    $aotResult = Get-Content $aotResultFile -Raw | ConvertFrom-Json
}

# --- Restore production GVFS ---
if (-not $SkipAot) {
    Write-Host ""
    Write-Host "  Restoring production GVFS..." -ForegroundColor Yellow

    if ($UseExistingEnlistment) {
        & "$gvfsInstallDir\GVFS.exe" unmount $UseExistingEnlistment 2>&1 | Out-Null
        Start-Sleep 5
    }

    Install-GVFSBuild -SourceDir $ProdDir -Label "Production (restored)"

    if ($UseExistingEnlistment) {
        & "$gvfsInstallDir\GVFS.exe" mount $UseExistingEnlistment 2>&1 | Out-Null
        Start-Sleep 3
    } elseif (Test-Path "D:\os\.gvfs") {
        Write-Host "  Remounting D:\os..."
        & "$gvfsInstallDir\GVFS.exe" mount "D:\os" 2>&1 | Out-Null
    }
}

# --- Generate Comparison Report ---
Write-Host ""
Write-Host "═══ Generating Comparison Report ═══" -ForegroundColor Yellow

$report = @"
# GVFS Branch Switch Performance Comparison

**Date**: $(Get-Date -Format "yyyy-MM-dd HH:mm")
**Machine**: $env:COMPUTERNAME ($env:PROCESSOR_IDENTIFIER)
**OS**: $(Get-CimInstance Win32_OperatingSystem | Select-Object -ExpandProperty Caption) Build $([Environment]::OSVersion.Version.Build)

## Test Configuration

| Parameter | Value |
|-----------|-------|
| Base Branch | ``$BaseBranch`` |
| Target Branch | ``$TargetBranch`` |
| Iterations | $Iterations |
| Mode | $(if ($UseExistingEnlistment) { "Existing enlistment (warm cache)" } else { "Fresh clone (cold cache)" }) |
| Working Tree | Verified clean before and after every checkout |

"@

if ($prodResult -and $aotResult) {
    # Compute average across iterations for multi-iteration runs
    function Get-Average {
        param($Iterations, [string]$Property)
        $values = @($Iterations | ForEach-Object { $_.$Property })
        if ($values.Count -eq 0) { return 0 }
        return ($values | Measure-Object -Average).Average
    }

    $prodAvgMs = Get-Average $prodResult.Iterations "CheckoutMs"
    $aotAvgMs  = Get-Average $aotResult.Iterations  "CheckoutMs"
    $prodAvgDL = Get-Average $prodResult.Iterations  "ObjectsDownloaded"
    $aotAvgDL  = Get-Average $aotResult.Iterations   "ObjectsDownloaded"
    $prodAvgHttp = Get-Average $prodResult.Iterations "HttpAvgMs"
    $aotAvgHttp  = Get-Average $aotResult.Iterations  "HttpAvgMs"
    $prodAvgMed  = Get-Average $prodResult.Iterations "HttpMedianMs"
    $aotAvgMed   = Get-Average $aotResult.Iterations  "HttpMedianMs"
    $prodAvgP95  = Get-Average $prodResult.Iterations "HttpP95Ms"
    $aotAvgP95   = Get-Average $aotResult.Iterations  "HttpP95Ms"
    $prodAvgP99  = Get-Average $prodResult.Iterations "HttpP99Ms"
    $aotAvgP99   = Get-Average $aotResult.Iterations  "HttpP99Ms"
    $prodAvgMax  = Get-Average $prodResult.Iterations "HttpMaxMs"
    $aotAvgMax   = Get-Average $aotResult.Iterations  "HttpMaxMs"
    $prodAvgTput = Get-Average $prodResult.Iterations "DownloadsPerSecond"
    $aotAvgTput  = Get-Average $aotResult.Iterations  "DownloadsPerSecond"

    function Format-Delta {
        param([double]$Prod, [double]$Aot, [string]$Unit = "ms", [bool]$LowerIsBetter = $true)
        if ($Prod -eq 0 -and $Aot -eq 0) { return "n/a" }
        $delta = $Aot - $Prod
        $pct = if ($Prod -ne 0) { [math]::Round($delta / $Prod * 100, 1) } else { 0 }
        $sign = if ($delta -gt 0) { "+" } else { "" }
        $verdict = if ($LowerIsBetter) {
            if ($pct -lt -5) { "**faster**" } elseif ($pct -gt 5) { "**slower**" } else { "~same" }
        } else {
            if ($pct -gt 5) { "**better**" } elseif ($pct -lt -5) { "**worse**" } else { "~same" }
        }
        return "$sign$([math]::Round($delta, 1)) $Unit ($sign$pct%) $verdict"
    }

    $report += @"
## Builds

| Build | Version |
|-------|---------|
| Production | $($prodResult.GvfsVersion) |
| NativeAOT | $($aotResult.GvfsVersion) |

## Summary (averaged over $Iterations iterations)

| Metric | Production | NativeAOT | Delta |
|--------|----------:|----------:|-------|
| **Checkout Wall Clock** | $([math]::Round($prodAvgMs / 1000, 2)) sec | $([math]::Round($aotAvgMs / 1000, 2)) sec | $(Format-Delta $prodAvgMs $aotAvgMs "ms") |
| Objects Downloaded | $([math]::Round($prodAvgDL, 0)) | $([math]::Round($aotAvgDL, 0)) | $(Format-Delta $prodAvgDL $aotAvgDL "objects") |
| Working Tree Clean | YES | YES | - |

"@

    if ($prodAvgDL -gt 0 -or $aotAvgDL -gt 0) {
        $report += @"

### HTTP Performance (per-request averages across iterations)

| Metric | Production | NativeAOT | Delta |
|--------|----------:|----------:|-------|
| Response Avg (ms) | $([math]::Round($prodAvgHttp, 1)) | $([math]::Round($aotAvgHttp, 1)) | $(Format-Delta $prodAvgHttp $aotAvgHttp) |
| Response Median (ms) | $([math]::Round($prodAvgMed, 1)) | $([math]::Round($aotAvgMed, 1)) | $(Format-Delta $prodAvgMed $aotAvgMed) |
| Response P95 (ms) | $([math]::Round($prodAvgP95, 1)) | $([math]::Round($aotAvgP95, 1)) | $(Format-Delta $prodAvgP95 $aotAvgP95) |
| Response P99 (ms) | $([math]::Round($prodAvgP99, 1)) | $([math]::Round($aotAvgP99, 1)) | $(Format-Delta $prodAvgP99 $aotAvgP99) |
| Response Max (ms) | $([math]::Round($prodAvgMax, 1)) | $([math]::Round($aotAvgMax, 1)) | $(Format-Delta $prodAvgMax $aotAvgMax) |
| Throughput (obj/sec) | $([math]::Round($prodAvgTput, 2)) | $([math]::Round($aotAvgTput, 2)) | $(Format-Delta $prodAvgTput $aotAvgTput "obj/s" $false) |

"@
    } else {
        $report += @"

> **Note**: Both runs used warm cache — no HTTP downloads were required.
> This measures pure git + GVFS projection overhead (no network component).

"@
    }

    # Per-iteration detail
    $report += @"

## Per-Iteration Detail

### Production
| Iter | Checkout (sec) | Downloads | HTTP Avg (ms) | HTTP P95 (ms) | Clean |
|-----:|--------------:|----------:|--------------:|--------------:|-------|
"@
    foreach ($r in $prodResult.Iterations) {
        $report += "| $($r.Iteration) | $([math]::Round($r.CheckoutMs / 1000, 2)) | $($r.ObjectsDownloaded) | $($r.HttpAvgMs) | $($r.HttpP95Ms) | YES |`n"
    }

    $report += @"

### NativeAOT
| Iter | Checkout (sec) | Downloads | HTTP Avg (ms) | HTTP P95 (ms) | Clean |
|-----:|--------------:|----------:|--------------:|--------------:|-------|
"@
    foreach ($r in $aotResult.Iterations) {
        $report += "| $($r.Iteration) | $([math]::Round($r.CheckoutMs / 1000, 2)) | $($r.ObjectsDownloaded) | $($r.HttpAvgMs) | $($r.HttpP95Ms) | YES |`n"
    }
} elseif ($prodResult) {
    $report += @"
## Production Baseline Only

| Metric | Value |
|--------|------:|
| GVFS Version | $($prodResult.GvfsVersion) |

### Per-Iteration
| Iter | Checkout (sec) | Downloads | HTTP Avg (ms) | HTTP P95 (ms) | Clean |
|-----:|--------------:|----------:|--------------:|--------------:|-------|
"@
    foreach ($r in $prodResult.Iterations) {
        $report += "| $($r.Iteration) | $([math]::Round($r.CheckoutMs / 1000, 2)) | $($r.ObjectsDownloaded) | $($r.HttpAvgMs) | $($r.HttpP95Ms) | $($r.WorkingTreeClean) |`n"
    }
} elseif ($aotResult) {
    $report += @"
## NativeAOT Result Only

| Metric | Value |
|--------|------:|
| GVFS Version | $($aotResult.GvfsVersion) |

### Per-Iteration
| Iter | Checkout (sec) | Downloads | HTTP Avg (ms) | HTTP P95 (ms) | Clean |
|-----:|--------------:|----------:|--------------:|--------------:|-------|
"@
    foreach ($r in $aotResult.Iterations) {
        $report += "| $($r.Iteration) | $([math]::Round($r.CheckoutMs / 1000, 2)) | $($r.ObjectsDownloaded) | $($r.HttpAvgMs) | $($r.HttpP95Ms) | $($r.WorkingTreeClean) |`n"
    }
}

$report += @"

## Methodology

- **Working tree enforcement**: Script verifies `git status --porcelain` is empty before and after every checkout. Fails immediately if any modified/untracked files are detected.
- **Git root auto-detection**: For OS enlistments, the git repo is at `<enlistment>\src`, not `<enlistment>` root.
- **Wall Clock**: `[Stopwatch]` around `git checkout` command (includes git + GVFS overhead)
- **Objects Downloaded**: Count of `DownloadLooseObject` events in the GVFS mount process log
- **HTTP metrics**: Parsed from GVFS log `responseWaitTimeMS` and `connectionWaitTimeMS` fields

### Key Metrics for Regression Detection

| Metric | What It Tells Us |
|--------|------------------|
| **Checkout Wall Clock** | Total user-visible time — the metric users care about |
| **HTTP Avg/Median** | Per-request overhead from the HTTP stack |
| **HTTP P95/P99** | Tail latency — shows connection pool or retry issues |
| **Working Tree Clean** | Confirms no state corruption from branch switching |

### Caveats

- Network conditions vary between runs; compare HTTP percentiles across multiple iterations
- Warm-cache mode measures git + ProjFS projection overhead (no download component)
- Cold-cache mode (fresh clone) gives the most comparable download metrics
"@

$report | Out-File $OutputFile -Encoding utf8
Write-Host "  Comparison report: $OutputFile" -ForegroundColor Green
Write-Host ""

# --- Cleanup ---
if ($Cleanup -and -not $UseExistingEnlistment) {
    Write-Host "  Cleaning up benchmark clones..."
    if (Test-Path "$CloneRoot\Production") {
        & "$gvfsInstallDir\GVFS.exe" unmount "$CloneRoot\Production" 2>&1 | Out-Null
        Remove-Item "$CloneRoot\Production" -Recurse -Force -EA 0
        Remove-Item "$CloneRoot\Production-cache" -Recurse -Force -EA 0
    }
    if (Test-Path "$CloneRoot\NativeAOT") {
        & "$gvfsInstallDir\GVFS.exe" unmount "$CloneRoot\NativeAOT" 2>&1 | Out-Null
        Remove-Item "$CloneRoot\NativeAOT" -Recurse -Force -EA 0
        Remove-Item "$CloneRoot\NativeAOT-cache" -Recurse -Force -EA 0
    }
}

Write-Host "Done!" -ForegroundColor Cyan
