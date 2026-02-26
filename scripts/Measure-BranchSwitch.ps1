<#
.SYNOPSIS
    Benchmark GVFS branch switching and object download performance for A/B comparison.

.DESCRIPTION
    Measures git checkout performance on a GVFS-mounted repository by:
    1. Verifying the working tree is clean (no modified/untracked files)
    2. Starting from a known base branch
    3. Performing a git checkout to a target branch
    4. Verifying the working tree is still clean afterward
    5. Parsing the GVFS mount process log for download metrics
    6. Reporting structured timing data

    The script will FAIL if the working tree is not clean before or after checkout.
    This ensures branch switching is being measured fairly — not masking state problems.

    For the OS enlistment at D:\os, the git repo root is at D:\os\src (not D:\os).
    The script auto-detects this by looking for a .git directory.

.PARAMETER TargetBranch
    The branch to checkout (the operation being measured).

.PARAMETER BaseBranch
    The starting branch before checkout. Default: official/ge_current_directes_corebuild

.PARAMETER RepoUrl
    GVFS-enabled repo URL. Default: os.2020

.PARAMETER CloneRoot
    Directory where the benchmark clone will be created. Each run gets a unique subdirectory.

.PARAMETER Label
    Label for this benchmark run (e.g., "Production", "NativeAOT"). Used in the report.

.PARAMETER GvfsExe
    Path to the GVFS executable to use. Default: C:\Program Files\GVFS\GVFS.exe

.PARAMETER ExistingEnlistment
    If specified, uses an already-mounted enlistment instead of creating a fresh clone.
    The git root is auto-detected (may be a subdirectory like \src).

.PARAMETER SkipClone
    Skip the clone step and assume the enlistment already exists at CloneRoot\<Label>.

.PARAMETER OutputFile
    Path to write the benchmark results (JSON). Auto-generated if not specified.

.PARAMETER Iterations
    Number of checkout iterations. Default: 1 (cold cache = single meaningful iteration).

.EXAMPLE
    # Cached comparison on existing enlistment
    .\Measure-BranchSwitch.ps1 -ExistingEnlistment "D:\os" -Label "Production" `
        -TargetBranch "official/ge_current_directes_corebuild_dev4"

    # Fresh clone for cold-cache comparison
    .\Measure-BranchSwitch.ps1 -Label "Production" -TargetBranch "official/ge_current_directes_corebuild_dev4"
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$TargetBranch,

    [string]$BaseBranch = "official/ge_current_directes_corebuild",

    [string]$RepoUrl = "https://dev.azure.com/microsoft/os/_git/os.2020",

    [string]$CloneRoot = "D:\gvfs-benchmark",

    [Parameter(Mandatory)]
    [string]$Label,

    [string]$GvfsExe = "C:\Program Files\GVFS\GVFS.exe",

    [string]$ExistingEnlistment,

    [switch]$SkipClone,

    [string]$OutputFile,

    [int]$Iterations = 1
)

$ErrorActionPreference = "Stop"
$git = "C:\Program Files\Git\cmd\git.exe"

if (-not $OutputFile) {
    $timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $OutputFile = "D:\src\VFSForGit\benchmark-branchswitch-$Label-$timestamp.json"
}

# --- Helpers ---

function Find-GitRoot {
    <#
    .SYNOPSIS
        Find the git repo root within a GVFS enlistment.
        For OS enlistments, .git is at <enlistment>\src\.git, not <enlistment>\.git.
    #>
    param([string]$EnlistmentPath)

    # Check if git repo is directly at the enlistment root
    if (Test-Path (Join-Path $EnlistmentPath ".git")) {
        return $EnlistmentPath
    }
    # Check src subdirectory (OS enlistment pattern)
    $srcGit = Join-Path $EnlistmentPath "src\.git"
    if (Test-Path $srcGit) {
        return (Join-Path $EnlistmentPath "src")
    }
    throw "Cannot find .git directory in $EnlistmentPath or $EnlistmentPath\src"
}

function Assert-CleanWorkingTree {
    <#
    .SYNOPSIS
        Verify git working tree is clean. Fails the benchmark if dirty.
        Uses --porcelain output (stdout only) — ignores stderr messages
        like GVFS lock-wait notifications.
    #>
    param([string]$GitRoot, [string]$Phase)

    Push-Location $GitRoot
    try {
        # Capture only stdout; stderr (GVFS lock messages, warnings) goes to $null
        $statusOutput = & $git status --porcelain 2>$null
        $dirtyFiles = @($statusOutput | Where-Object { $_ -and $_.Trim().Length -gt 0 })
        $dirtyCount = $dirtyFiles.Count
        if ($dirtyCount -gt 0) {
            Write-Host ""
            Write-Host "  FAIL: Working tree is DIRTY during '$Phase'" -ForegroundColor Red
            Write-Host "  $dirtyCount dirty files (modified + untracked):" -ForegroundColor Red
            $dirtyFiles | Select-Object -First 20 | ForEach-Object { Write-Host "    $_" -ForegroundColor Red }
            if ($dirtyCount -gt 20) {
                Write-Host "    ... and $($dirtyCount - 20) more" -ForegroundColor Red
            }
            throw "Working tree must be clean for benchmarking. $dirtyCount dirty files found during '$Phase'."
        }
        Write-Host "  Working tree clean ($Phase)" -ForegroundColor Green
    } finally {
        Pop-Location
    }
}

function Get-CurrentBranch {
    param([string]$GitRoot)
    Push-Location $GitRoot
    try {
        $branch = & $git branch --show-current 2>&1
        return $branch.Trim()
    } finally {
        Pop-Location
    }
}

# --- Banner ---
Write-Host "╔═══════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  GVFS Branch Switch Benchmark                        ║" -ForegroundColor Cyan
Write-Host "╠═══════════════════════════════════════════════════════╣" -ForegroundColor Cyan
Write-Host "║  Label:      $($Label.PadRight(40))║" -ForegroundColor Cyan
Write-Host "║  GVFS:       $($GvfsExe.PadRight(40))║" -ForegroundColor Cyan
Write-Host "║  Base:       $($BaseBranch.Substring(0, [math]::Min(40, $BaseBranch.Length)).PadRight(40))║" -ForegroundColor Cyan
Write-Host "║  Target:     $($TargetBranch.Substring(0, [math]::Min(40, $TargetBranch.Length)).PadRight(40))║" -ForegroundColor Cyan
Write-Host "║  Iterations: $($Iterations.ToString().PadRight(40))║" -ForegroundColor Cyan
Write-Host "╚═══════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# --- Verify prerequisites ---
if (-not (Test-Path $GvfsExe)) { throw "GVFS executable not found: $GvfsExe" }
$gvfsVersion = & $GvfsExe version 2>&1 | Select-Object -First 1
Write-Host "  GVFS Version: $gvfsVersion" -ForegroundColor Gray
$gitVersion = & $git version 2>&1
Write-Host "  Git Version:  $gitVersion" -ForegroundColor Gray
Write-Host ""

# --- Setup enlistment ---
$enlistmentPath = $null
$gitRoot = $null
$logDir = $null
$isFreshClone = $false

if ($ExistingEnlistment) {
    $enlistmentPath = $ExistingEnlistment
    $logDir = Join-Path $enlistmentPath ".gvfs\logs"
    $gitRoot = Find-GitRoot $enlistmentPath

    Write-Host "  Using existing enlistment: $enlistmentPath" -ForegroundColor Yellow
    Write-Host "  Git root: $gitRoot" -ForegroundColor Yellow
    Write-Host ""

    # Verify mounted
    $gvfsStatus = & $GvfsExe status $enlistmentPath 2>&1 | Out-String
    if ($gvfsStatus -match "not mounted|Unable to connect") {
        Write-Host "  Mounting $enlistmentPath..."
        & $GvfsExe mount $enlistmentPath 2>&1 | Out-Null
        Start-Sleep 5
    }

    # Verify clean state BEFORE any checkout
    Assert-CleanWorkingTree -GitRoot $gitRoot -Phase "pre-benchmark verification"

    # Ensure we're on the base branch
    $currentBranch = Get-CurrentBranch $gitRoot
    if ($currentBranch -ne $BaseBranch) {
        Write-Host "  Switching to base branch: $BaseBranch (from $currentBranch)"
        Push-Location $gitRoot
        & $git checkout $BaseBranch 2>&1 | Write-Host
        $exitCode = $LASTEXITCODE
        Pop-Location

        if ($exitCode -ne 0) {
            throw "Failed to checkout base branch $BaseBranch (exit code: $exitCode)"
        }

        # Let GVFS settle after checkout (projection updates, lock release)
        Start-Sleep 5

        # Verify still clean after switching to base
        Assert-CleanWorkingTree -GitRoot $gitRoot -Phase "after switching to base branch"
    }
    Write-Host "  On base branch: $BaseBranch" -ForegroundColor Green
} else {
    $enlistmentPath = Join-Path $CloneRoot $Label
    $cacheDir = Join-Path $CloneRoot "$Label-cache"
    $logDir = Join-Path $enlistmentPath ".gvfs\logs"

    if (-not $SkipClone) {
        Write-Host "  Creating fresh clone for cold-cache comparison..." -ForegroundColor Yellow

        # Clean up any previous run
        if (Test-Path $enlistmentPath) {
            Write-Host "  Cleaning up previous benchmark enlistment..."
            & $GvfsExe unmount $enlistmentPath 2>&1 | Out-Null
            Start-Sleep 3
            Remove-Item $enlistmentPath -Recurse -Force -EA 0
        }
        if (Test-Path $cacheDir) {
            Remove-Item $cacheDir -Recurse -Force -EA 0
        }

        # Clone
        Write-Host "  Cloning $RepoUrl (branch: $BaseBranch)..."
        $cloneSw = [System.Diagnostics.Stopwatch]::StartNew()
        & $GvfsExe clone $RepoUrl $enlistmentPath --branch $BaseBranch --local-cache-path $cacheDir 2>&1 | Write-Host
        $cloneSw.Stop()
        Write-Host "  Clone completed in $([math]::Round($cloneSw.Elapsed.TotalMinutes, 1)) minutes" -ForegroundColor Green
        $isFreshClone = $true
    } else {
        Write-Host "  Using existing benchmark clone (SkipClone): $enlistmentPath"
        & $GvfsExe mount $enlistmentPath 2>&1 | Out-Null
        Start-Sleep 3
    }

    $gitRoot = Find-GitRoot $enlistmentPath

    # Verify clean state after clone/mount
    Assert-CleanWorkingTree -GitRoot $gitRoot -Phase "post-clone verification"
}

# --- Record log position before benchmark ---
$preLogFiles = Get-ChildItem "$logDir\gvfs_mount_process_*.log" -EA 0 | Sort-Object Name -Descending
$preLogPath = $preLogFiles | Select-Object -First 1 -ExpandProperty FullName
$preLogLines = if ($preLogPath) { (Get-Content $preLogPath).Count } else { 0 }

Write-Host ""
Write-Host "  Pre-benchmark log: $preLogPath (lines: $preLogLines)"

# --- Run the benchmark ---
$allResults = @()

for ($iter = 0; $iter -lt $Iterations; $iter++) {
    Write-Host ""
    Write-Host "══ Iteration $($iter + 1) of $Iterations ══" -ForegroundColor Yellow

    # If not the first iteration, go back to base branch first
    if ($iter -gt 0) {
        Write-Host "  Returning to base branch: $BaseBranch"
        Push-Location $gitRoot
        & $git checkout $BaseBranch 2>&1 | Write-Host
        $exitCode = $LASTEXITCODE
        Pop-Location

        if ($exitCode -ne 0) {
            throw "Failed to checkout base branch (iteration $($iter + 1), exit code: $exitCode)"
        }

        # Let GVFS settle after checkout
        Start-Sleep 5

        Assert-CleanWorkingTree -GitRoot $gitRoot -Phase "return to base branch (iteration $($iter + 1))"

        Start-Sleep 2

        # Note new log position
        $preLogPath = Get-ChildItem "$logDir\gvfs_mount_process_*.log" -EA 0 |
            Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName
        $preLogLines = if ($preLogPath) { (Get-Content $preLogPath).Count } else { 0 }
    }

    # Perform the checkout
    Write-Host "  Checking out: $TargetBranch"
    $checkoutSw = [System.Diagnostics.Stopwatch]::StartNew()

    Push-Location $gitRoot
    $checkoutOutput = & $git checkout $TargetBranch 2>&1 | Out-String
    $checkoutExitCode = $LASTEXITCODE
    Pop-Location

    $checkoutSw.Stop()
    $checkoutMs = $checkoutSw.Elapsed.TotalMilliseconds

    Write-Host "  Checkout completed in $([math]::Round($checkoutMs / 1000, 1)) seconds (exit code: $checkoutExitCode)"

    if ($checkoutExitCode -ne 0) {
        Write-Host "  Checkout output: $($checkoutOutput.Trim())" -ForegroundColor Red
        throw "git checkout failed with exit code $checkoutExitCode. Working tree may be dirty."
    }

    # Let GVFS settle after checkout (projection updates, lock release)
    Start-Sleep 5

    # Verify clean state AFTER checkout
    Assert-CleanWorkingTree -GitRoot $gitRoot -Phase "after checkout to $TargetBranch (iteration $($iter + 1))"

    # Wait a moment for any async log flushing
    Start-Sleep 2

    # --- Parse the new log entries ---
    $postLogPath = Get-ChildItem "$logDir\gvfs_mount_process_*.log" -EA 0 |
        Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName

    $postLogLines = if ($postLogPath) { (Get-Content $postLogPath).Count } else { 0 }

    # If the log file rotated during the test, read the whole new file
    $logLinesToParse = @()
    if ($postLogPath -ne $preLogPath) {
        $logLinesToParse = Get-Content $postLogPath
    } else {
        if ($postLogLines -gt $preLogLines) {
            $logLinesToParse = Get-Content $postLogPath | Select-Object -Skip $preLogLines
        }
    }

    Write-Host "  New log lines to parse: $($logLinesToParse.Count)"

    # Parse download events from new log lines
    $downloads = @()
    $httpResponses = @()

    foreach ($line in $logLinesToParse) {
        if ($line -match 'DownloadLooseObject' -and $line -match '^\[(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d+)') {
            $ts = [datetime]::ParseExact($Matches[1], "yyyy-MM-dd HH:mm:ss.ffff", $null)
            $objId = if ($line -match '"objectId":"([a-f0-9]+)"') { $Matches[1] } else { "" }
            $src = if ($line -match '"requestSource":"([^"]+)"') { $Matches[1] } else { "" }
            $downloads += [PSCustomObject]@{ Timestamp = $ts; ObjectId = $objId; Source = $src }
        }

        if ($line -match 'NetworkResponse' -and $line -match 'gvfs/objects/' -and $line -match '^\[(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d+)') {
            $ts = [datetime]::ParseExact($Matches[1], "yyyy-MM-dd HH:mm:ss.ffff", $null)
            $respMs = if ($line -match '"responseWaitTimeMS":"([^"]+)"') { [double]$Matches[1] } else { 0 }
            $connMs = if ($line -match '"connectionWaitTimeMS":"([^"]+)"') { [double]$Matches[1] } else { 0 }
            $status = if ($line -match '"StatusCode":(\d+)') { [int]$Matches[1] } else { 0 }
            $httpResponses += [PSCustomObject]@{
                Timestamp = $ts; ResponseWaitMs = $respMs; ConnectionWaitMs = $connMs; StatusCode = $status
            }
        }
    }

    # Compute statistics
    $respWaits = @($httpResponses | ForEach-Object { $_.ResponseWaitMs })
    $connWaits = @($httpResponses | ForEach-Object { $_.ConnectionWaitMs })

    $respSorted = $respWaits | Sort-Object
    $httpTotalMs = if ($respWaits.Count -gt 0) { ($respWaits | Measure-Object -Sum).Sum } else { 0 }
    $httpAvgMs = if ($respWaits.Count -gt 0) { ($respWaits | Measure-Object -Average).Average } else { 0 }
    $httpMedianMs = if ($respSorted.Count -gt 0) { $respSorted[[math]::Floor($respSorted.Count / 2)] } else { 0 }
    $httpP95Ms = if ($respSorted.Count -gt 0) { $respSorted[[math]::Floor($respSorted.Count * 0.95)] } else { 0 }
    $httpP99Ms = if ($respSorted.Count -gt 0) { $respSorted[[math]::Floor($respSorted.Count * 0.99)] } else { 0 }
    $httpMaxMs = if ($respSorted.Count -gt 0) { $respSorted[-1] } else { 0 }
    $httpMinMs = if ($respSorted.Count -gt 0) { $respSorted[0] } else { 0 }

    $connSorted = $connWaits | Sort-Object
    $connAvgMs = if ($connWaits.Count -gt 0) { ($connWaits | Measure-Object -Average).Average } else { 0 }
    $connMedianMs = if ($connSorted.Count -gt 0) { $connSorted[[math]::Floor($connSorted.Count / 2)] } else { 0 }
    $connP95Ms = if ($connSorted.Count -gt 0) { $connSorted[[math]::Floor($connSorted.Count * 0.95)] } else { 0 }
    $connMaxMs = if ($connSorted.Count -gt 0) { $connSorted[-1] } else { 0 }

    $dlSpanSec = if ($downloads.Count -gt 1) {
        ($downloads[-1].Timestamp - $downloads[0].Timestamp).TotalSeconds
    } else { 0 }

    $throughput = if ($dlSpanSec -gt 0) { [math]::Round($downloads.Count / $dlSpanSec, 2) } else { 0 }

    $iterResult = [PSCustomObject]@{
        Iteration            = $iter + 1
        CheckoutMs           = [math]::Round($checkoutMs, 1)
        CheckoutExitCode     = $checkoutExitCode
        WorkingTreeClean     = $true
        ObjectsDownloaded    = $downloads.Count
        HttpRequests         = $httpResponses.Count
        DownloadSpanSeconds  = [math]::Round($dlSpanSec, 1)
        DownloadsPerSecond   = $throughput
        HttpTotalMs          = [math]::Round($httpTotalMs, 1)
        HttpAvgMs            = [math]::Round($httpAvgMs, 1)
        HttpMedianMs         = [math]::Round($httpMedianMs, 1)
        HttpP95Ms            = [math]::Round($httpP95Ms, 1)
        HttpP99Ms            = [math]::Round($httpP99Ms, 1)
        HttpMinMs            = [math]::Round($httpMinMs, 1)
        HttpMaxMs            = [math]::Round($httpMaxMs, 1)
        ConnAvgMs            = [math]::Round($connAvgMs, 1)
        ConnMedianMs         = [math]::Round($connMedianMs, 1)
        ConnP95Ms            = [math]::Round($connP95Ms, 1)
        ConnMaxMs            = [math]::Round($connMaxMs, 1)
    }

    $allResults += $iterResult

    Write-Host ""
    Write-Host "  ─── Iteration $($iter + 1) Results ───" -ForegroundColor Green
    Write-Host "  Wall Clock:         $([math]::Round($checkoutMs / 1000, 1)) sec"
    Write-Host "  Working Tree:       CLEAN" -ForegroundColor Green
    Write-Host "  Objects Downloaded: $($downloads.Count)"
    Write-Host "  HTTP Requests:      $($httpResponses.Count)"
    if ($downloads.Count -gt 0) {
        Write-Host "  Download Span:      $([math]::Round($dlSpanSec, 1)) sec"
        Write-Host "  Throughput:         $throughput objects/sec"
        Write-Host "  HTTP Response Time: avg=$([math]::Round($httpAvgMs, 1))ms median=$([math]::Round($httpMedianMs, 1))ms p95=$([math]::Round($httpP95Ms, 1))ms max=$([math]::Round($httpMaxMs, 1))ms"
        Write-Host "  HTTP Connect Time:  avg=$([math]::Round($connAvgMs, 1))ms median=$([math]::Round($connMedianMs, 1))ms p95=$([math]::Round($connP95Ms, 1))ms max=$([math]::Round($connMaxMs, 1))ms"
    } else {
        Write-Host "  (Warm cache — no HTTP downloads needed)" -ForegroundColor DarkGray
    }
}

# --- Return to base branch ---
Write-Host ""
Write-Host "  Returning to base branch: $BaseBranch"
Push-Location $gitRoot
& $git checkout $BaseBranch 2>&1 | Write-Host
Pop-Location
Start-Sleep 5
Assert-CleanWorkingTree -GitRoot $gitRoot -Phase "final return to base branch"

# --- Build final report ---
$report = [PSCustomObject]@{
    Label           = $Label
    GvfsVersion     = $gvfsVersion
    GitVersion      = $gitVersion
    Machine         = $env:COMPUTERNAME
    Timestamp       = (Get-Date -Format "yyyy-MM-dd HH:mm:ss")
    BaseBranch      = $BaseBranch
    TargetBranch    = $TargetBranch
    RepoUrl         = $RepoUrl
    FreshClone      = $isFreshClone
    EnlistmentPath  = $enlistmentPath
    GitRoot         = $gitRoot
    Iterations      = $allResults
}

# Save JSON report
$report | ConvertTo-Json -Depth 5 | Out-File $OutputFile -Encoding utf8
Write-Host ""
Write-Host "  Report saved to: $OutputFile" -ForegroundColor Green

# --- Print summary table ---
Write-Host ""
Write-Host "╔═══════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  Summary: $($Label.PadRight(43))║" -ForegroundColor Cyan
Write-Host "╠═══════════════════════════════════════════════════════╣" -ForegroundColor Cyan

foreach ($r in $allResults) {
    Write-Host "║  Iteration $($r.Iteration):                                        ║" -ForegroundColor Cyan
    Write-Host "║    Checkout:     $("$([math]::Round($r.CheckoutMs / 1000, 1)) sec".PadRight(36))║" -ForegroundColor White
    Write-Host "║    Clean Tree:   $("YES".PadRight(36))║" -ForegroundColor Green
    Write-Host "║    Downloads:    $("$($r.ObjectsDownloaded) objects in $($r.DownloadSpanSeconds) sec".PadRight(36))║" -ForegroundColor White
    if ($r.ObjectsDownloaded -gt 0) {
        Write-Host "║    HTTP Avg:     $("$($r.HttpAvgMs) ms".PadRight(36))║" -ForegroundColor White
        Write-Host "║    HTTP Median:  $("$($r.HttpMedianMs) ms".PadRight(36))║" -ForegroundColor White
        Write-Host "║    HTTP P95:     $("$($r.HttpP95Ms) ms".PadRight(36))║" -ForegroundColor White
        Write-Host "║    HTTP Max:     $("$($r.HttpMaxMs) ms".PadRight(36))║" -ForegroundColor White
        Write-Host "║    Throughput:   $("$($r.DownloadsPerSecond) obj/sec".PadRight(36))║" -ForegroundColor White
    } else {
        Write-Host "║    Cache:        $("warm (no downloads)".PadRight(36))║" -ForegroundColor DarkGray
    }
}

Write-Host "╚═══════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# Output the report object for piping
$report
