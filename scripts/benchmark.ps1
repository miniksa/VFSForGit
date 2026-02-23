<#
.SYNOPSIS
    VFSForGit .NET Framework 4.7.1 vs .NET 10 Performance Comparison

.DESCRIPTION
    Benchmarks common GVFS operations against two builds:
    - Production: C:\Program Files\GVFS\ (.NET Framework 4.7.1)
    - .NET 10:    Build output from the migration branch

    Must be run as Administrator (service install requires elevation).

.PARAMETER Iterations
    Number of iterations per benchmark (default: 5)

.PARAMETER TestRepo
    URL of the test repo to clone (default: ForTests repo)

.PARAMETER LargeRepo
    Path to an existing large GVFS repo for mount/status tests (default: D:\os)

.PARAMETER OutputFile
    Path to write the markdown report (default: benchmark-results.md)
#>
param(
    [int]$Iterations = 5,
    [string]$TestRepo = "https://gvfs.visualstudio.com/ci/_git/ForTests",
    [string]$TestBranch = "FunctionalTests/20201014",
    [string]$LargeRepo = "D:\os",
    [string]$OutputFile = "D:\src\VFSForGit\benchmark-results-$(Get-Date -Format 'yyyyMMdd-HHmmss').md",
    [switch]$UsePublished,
    [switch]$IncludeRust
)

$ErrorActionPreference = "Continue"

# --- Configuration ---
$prodGVFS = "C:\Program Files\GVFS\GVFS.exe"
if ($UsePublished) {
    $net10GVFS = "D:\src\out\gvfs-aot-layout\GVFS.exe"
    $net10Label = ".NET 10 (NativeAOT)"
} else {
    $net10GVFS = "D:\src\out\GVFS\bin\Release\net10.0-windows10.0.17763.0\win-x64\GVFS.exe"
    $net10Label = ".NET 10 (JIT only)"
}
$rustGVFS = "D:\src\VFSForGit\rust-gvfs\target\release\gvfs.exe"
$git = "C:\Program Files\Git\cmd\git.exe"
$cloneRoot = "C:\Repos\GVFSBenchmark"

# Verify builds exist
if (-not (Test-Path $prodGVFS)) { throw "Production GVFS not found at $prodGVFS" }
if (-not (Test-Path $net10GVFS)) { Write-Warning ".NET 10 GVFS not found at $net10GVFS — will skip .NET 10 benchmarks" }
if ($IncludeRust -and -not (Test-Path $rustGVFS)) { throw "Rust GVFS not found at $rustGVFS — run: cd rust-gvfs && cargo build --release" }

Write-Host "=== VFSForGit Performance Benchmark ===" -ForegroundColor Cyan
Write-Host "Production: $prodGVFS"
Write-Host "  Version:  $(& $prodGVFS version 2>&1)"
if (Test-Path $net10GVFS) {
    Write-Host ".NET 10:    $net10GVFS"
    Write-Host "  Version:  $(& $net10GVFS version 2>&1)"
}
if ($IncludeRust) {
    Write-Host "Rust:       $rustGVFS"
    Write-Host "  Version:  $(& $rustGVFS version 2>&1)"
}
Write-Host "Iterations: $Iterations"
Write-Host "Test Repo:  $TestRepo"
Write-Host "Large Repo: $LargeRepo"
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
Write-Host "`n--- Benchmark 1: Startup Time (gvfs version) ---" -ForegroundColor Yellow

$r1_prod = Measure-Operation -Name "Production" -Iterations $Iterations -Operation {
    & $prodGVFS version 2>&1
}

$r1_net10 = Measure-Operation -Name ".NET 10" -Iterations $Iterations -Operation {
    & $net10GVFS version 2>&1
}

$rustResult1 = $null
if ($IncludeRust) {
    $rustResult1 = Measure-Operation -Name "Rust" -Iterations $Iterations -Operation {
        & $rustGVFS version 2>&1
    }
}

$allResults += [PSCustomObject]@{ Benchmark = "Startup (gvfs version)"; Prod = $r1_prod; Net10 = $r1_net10; Rust = $rustResult1 }

# ============================================================
# Benchmark 2: Clone (small test repo)
# ============================================================
Write-Host "`n--- Benchmark 2: Clone (small test repo, no mount) ---" -ForegroundColor Yellow

$cloneIter = [math]::Min($Iterations, 3)  # Clones are slow, limit iterations

$r2_prod = Measure-Operation -Name "Production" -Iterations $cloneIter `
    -Setup { Remove-Item "$cloneRoot\prod" -Recurse -Force -EA 0 } `
    -Operation { & $prodGVFS clone $TestRepo "$cloneRoot\prod" --branch $TestBranch --no-mount 2>&1 } `
    -Cleanup { & $prodGVFS unmount "$cloneRoot\prod" --skip-wait-for-lock 2>&1; Start-Sleep 2; Remove-Item "$cloneRoot\prod" -Recurse -Force -EA 0 }

$r2_net10 = Measure-Operation -Name ".NET 10" -Iterations $cloneIter `
    -Setup { Remove-Item "$cloneRoot\net10" -Recurse -Force -EA 0 } `
    -Operation { & $net10GVFS clone $TestRepo "$cloneRoot\net10" --branch $TestBranch --no-mount 2>&1 } `
    -Cleanup { & $net10GVFS unmount "$cloneRoot\net10" --skip-wait-for-lock 2>&1; Start-Sleep 2; Remove-Item "$cloneRoot\net10" -Recurse -Force -EA 0 }

$rustResult2 = $null
if ($IncludeRust) {
    $rustResult2 = Measure-Operation -Name "Rust" -Iterations $cloneIter `
        -Setup { Remove-Item "$cloneRoot\rust" -Recurse -Force -EA 0 } `
        -Operation { & $rustGVFS clone $TestRepo "$cloneRoot\rust" --branch $TestBranch --no-mount 2>&1 } `
        -Cleanup { Remove-Item "$cloneRoot\rust" -Recurse -Force -EA 0 }
}

$allResults += [PSCustomObject]@{ Benchmark = "Clone (small repo, no mount)"; Prod = $r2_prod; Net10 = $r2_net10; Rust = $rustResult2 }

# ============================================================
# Benchmark 3: Mount (small test repo)
# ============================================================
Write-Host "`n--- Benchmark 3: Mount + Unmount (small test repo) ---" -ForegroundColor Yellow

# Pre-clone for mount tests
Remove-Item "$cloneRoot\mount_prod" -Recurse -Force -EA 0
& $prodGVFS clone $TestRepo "$cloneRoot\mount_prod" --branch $TestBranch --no-mount 2>&1 | Out-Null
Remove-Item "$cloneRoot\mount_net10" -Recurse -Force -EA 0
& $net10GVFS clone $TestRepo "$cloneRoot\mount_net10" --branch $TestBranch --no-mount 2>&1 | Out-Null
if ($IncludeRust) {
    Remove-Item "$cloneRoot\mount_rust" -Recurse -Force -EA 0
    & $rustGVFS clone $TestRepo "$cloneRoot\mount_rust" --branch $TestBranch --no-mount 2>&1 | Out-Null
}

$r3_prod = Measure-Operation -Name "Production" -Iterations $Iterations `
    -Operation { & $prodGVFS mount "$cloneRoot\mount_prod" 2>&1 } `
    -Cleanup { & $prodGVFS unmount "$cloneRoot\mount_prod" 2>&1; Start-Sleep 2 }

$r3_net10 = Measure-Operation -Name ".NET 10" -Iterations $Iterations `
    -Operation { & $net10GVFS mount "$cloneRoot\mount_net10" 2>&1 } `
    -Cleanup { & $net10GVFS unmount "$cloneRoot\mount_net10" 2>&1; Start-Sleep 2 }

$rustResult3 = $null
if ($IncludeRust) {
    $rustResult3 = Measure-Operation -Name "Rust" -Iterations $Iterations `
        -Operation { & $rustGVFS mount "$cloneRoot\mount_rust" 2>&1 } `
        -Cleanup { & $rustGVFS unmount "$cloneRoot\mount_rust" 2>&1; Start-Sleep 2 }
}

$allResults += [PSCustomObject]@{ Benchmark = "Mount (small repo)"; Prod = $r3_prod; Net10 = $r3_net10; Rust = $rustResult3 }

# ============================================================
# Benchmark 4: Status (pipe roundtrip)
# ============================================================
Write-Host "`n--- Benchmark 4: Status (named pipe roundtrip) ---" -ForegroundColor Yellow

# Mount for status tests
& $prodGVFS mount "$cloneRoot\mount_prod" 2>&1 | Out-Null
& $net10GVFS mount "$cloneRoot\mount_net10" 2>&1 | Out-Null
if ($IncludeRust) { & $rustGVFS mount "$cloneRoot\mount_rust" 2>&1 | Out-Null }

# Wait until mounts are fully ready
$maxWait = 60
for ($w = 0; $w -lt $maxWait; $w++) {
    $prodStatus = & $prodGVFS status "$cloneRoot\mount_prod" 2>&1 | Out-String
    $net10Status = & $net10GVFS status "$cloneRoot\mount_net10" 2>&1 | Out-String
    $rustReady = if ($IncludeRust) { (& $rustGVFS status "$cloneRoot\mount_rust" 2>&1 | Out-String) -match "Ready" } else { $true }
    if ($prodStatus -match "Mount status: Ready" -and $net10Status -match "Mount status: Ready" -and $rustReady) {
        Write-Host "  All mounts ready after $($w+1)s"
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

$rustResult4 = $null
if ($IncludeRust) {
    $rustResult4 = Measure-Operation -Name "Rust" -Iterations ($Iterations * 2) -Operation {
        & $rustGVFS status "$cloneRoot\mount_rust" 2>&1
    }
}

$allResults += [PSCustomObject]@{ Benchmark = "Status (pipe roundtrip)"; Prod = $r4_prod; Net10 = $r4_net10; Rust = $rustResult4 }

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

$rustResult5 = $null
if ($IncludeRust) {
    $rustResult5 = Measure-Operation -Name "Rust" -Iterations $cloneIter -Operation {
        & $rustGVFS prefetch "$cloneRoot\mount_rust" --files "*.md" 2>&1
    }
}

$allResults += [PSCustomObject]@{ Benchmark = "Prefetch (*.md)"; Prod = $r5_prod; Net10 = $r5_net10; Rust = $rustResult5 }

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

$rustResult6 = $null
if ($IncludeRust) {
    $rustResult6 = Measure-Operation -Name "Rust" -Iterations $Iterations -Operation {
        & $git -C "$cloneRoot\mount_rust\src" status 2>&1
    }
}

$allResults += [PSCustomObject]@{ Benchmark = "Git Status"; Prod = $r6_prod; Net10 = $r6_net10; Rust = $rustResult6 }

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

$rustResult7 = $null
if ($IncludeRust) {
    $rustResult7 = Measure-Operation -Name "Rust" -Iterations $Iterations -Operation {
        & $git -C "$cloneRoot\mount_rust\src" log --oneline -100 2>&1
    }
}

$allResults += [PSCustomObject]@{ Benchmark = "Git Log (-100)"; Prod = $r7_prod; Net10 = $r7_net10; Rust = $rustResult7 }

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

$rustResult8 = $null
if ($IncludeRust) {
    $rustResult8 = Measure-Operation -Name "Rust" -Iterations $Iterations -Operation {
        (Get-ChildItem "$cloneRoot\mount_rust\src" -Recurse -File).Count
    }
}

$allResults += [PSCustomObject]@{ Benchmark = "Dir Enumeration (ProjFS)"; Prod = $r8_prod; Net10 = $r8_net10; Rust = $rustResult8 }

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

$rustResult9 = $null
if ($IncludeRust) {
    $rustResult9 = Measure-Operation -Name "Rust" -Iterations $cloneIter -Operation {
        Get-Content "$cloneRoot\mount_rust\src\Readme.md" -Raw 2>&1
    }
}

$allResults += [PSCustomObject]@{ Benchmark = "File Read (hydration)"; Prod = $r9_prod; Net10 = $r9_net10; Rust = $rustResult9 }

# ============================================================
# Benchmark 10: Unmount
# ============================================================
Write-Host "`n--- Benchmark 10: Unmount ---" -ForegroundColor Yellow

$r10_prod = Measure-Operation -Name "Production" -Iterations $cloneIter `
    -Setup { & $prodGVFS mount "$cloneRoot\mount_prod" 2>&1; Start-Sleep 2 } `
    -Operation { & $prodGVFS unmount "$cloneRoot\mount_prod" 2>&1 }

$r10_net10 = Measure-Operation -Name ".NET 10" -Iterations $cloneIter `
    -Setup { & $net10GVFS mount "$cloneRoot\mount_net10" 2>&1; Start-Sleep 2 } `
    -Operation { & $net10GVFS unmount "$cloneRoot\mount_net10" 2>&1 }

$rustResult10 = $null
if ($IncludeRust) {
    $rustResult10 = Measure-Operation -Name "Rust" -Iterations $cloneIter `
        -Setup { & $rustGVFS mount "$cloneRoot\mount_rust" 2>&1; Start-Sleep 2 } `
        -Operation { & $rustGVFS unmount "$cloneRoot\mount_rust" 2>&1 }
}

$allResults += [PSCustomObject]@{ Benchmark = "Unmount"; Prod = $r10_prod; Net10 = $r10_net10; Rust = $rustResult10 }

# ============================================================
# Large Repo Benchmarks (if available)
# ============================================================
if (Test-Path $LargeRepo) {
    Write-Host "`n--- Benchmark 11: Large Repo Status ($LargeRepo) ---" -ForegroundColor Yellow
    Write-Host "  (Using existing mount - same GVFS.Mount process)"

    $r11 = Measure-Operation -Name "Large Repo Status" -Iterations $Iterations -Operation {
        & $git -C "$LargeRepo" status --short 2>&1
    }

    $allResults += [PSCustomObject]@{ Benchmark = "Large Repo Git Status"; Prod = $r11; Net10 = $null }
}

# ============================================================
# Cleanup
# ============================================================
Write-Host "`n--- Cleanup ---" -ForegroundColor Yellow
& $prodGVFS unmount "$cloneRoot\mount_prod" --skip-wait-for-lock 2>&1 | Out-Null
& $net10GVFS unmount "$cloneRoot\mount_net10" --skip-wait-for-lock 2>&1 | Out-Null
if ($IncludeRust) { & $rustGVFS unmount "$cloneRoot\mount_rust" 2>&1 | Out-Null }
Get-Process GVFS.Mount -EA 0 | Where-Object { $_.StartTime -gt (Get-Date).AddHours(-1) } | Stop-Process -Force -EA 0
Get-Process gvfs-mount -EA 0 | Where-Object { $_.StartTime -gt (Get-Date).AddHours(-1) } | Stop-Process -Force -EA 0
Start-Sleep 3
Remove-Item "$cloneRoot" -Recurse -Force -EA 0

# ============================================================
# Generate Report
# ============================================================
Write-Host "`n--- Generating Report ---" -ForegroundColor Yellow

$prodVersion = ((& $prodGVFS version) 2>$null | Select-Object -First 1)
$net10Version = ((& $net10GVFS version) 2>$null | Select-Object -First 1)
$rustVersion = if ($IncludeRust) { ((& $rustGVFS version) 2>$null | Select-Object -First 1) } else { "N/A" }

$report = @"
# VFSForGit Performance Comparison

**Date**: $(Get-Date -Format "yyyy-MM-dd HH:mm")
**Machine**: $env:COMPUTERNAME ($env:PROCESSOR_IDENTIFIER)
**OS**: $(Get-CimInstance Win32_OperatingSystem | Select-Object -ExpandProperty Caption) Build $([Environment]::OSVersion.Version.Build)
**Iterations**: $Iterations per benchmark (clones: $cloneIter)

| Build | Version | Framework | Config |
|-------|---------|-----------|--------|
| Production | $prodVersion | .NET Framework 4.7.1 | Installed (C:\Program Files\GVFS) |
| .NET 10 | $net10Version | .NET 10.0 (self-contained) | Build output (Release) |
| Rust | $rustVersion | Native (no runtime) | cargo build --release |

## Results

| Benchmark | Production (ms) | .NET 10 (ms) | Rust (ms) | Delta (Rust vs Prod) |
|-----------|---------------:|-------------:|----------:|--------------------:|
"@

foreach ($r in $allResults) {
    $net10Col = if ($r.Net10) { "$($r.Net10.Avg) ± $($r.Net10.StdDev)" } else { "N/A" }
    $rustCol = if ($r.Rust) { "$($r.Rust.Avg) ± $($r.Rust.StdDev)" } else { "N/A" }
    $deltaCol = if ($r.Rust -and $r.Prod.Avg -gt 0) {
        $delta = $r.Rust.Avg - $r.Prod.Avg
        $pct = [math]::Round($delta / $r.Prod.Avg * 100, 1)
        $sign = if ($delta -gt 0) { "+" } else { "" }
        "$sign$([math]::Round($delta, 1)) ($sign$pct%)"
    } else { "N/A" }
    $report += "| $($r.Benchmark) | $($r.Prod.Avg) ± $($r.Prod.StdDev) | $net10Col | $rustCol | $deltaCol |" + "`r`n"
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
