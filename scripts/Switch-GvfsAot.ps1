<#
.SYNOPSIS
    Switch GVFS repos between production (.NET Framework) and .NET 10 NativeAOT builds.

.DESCRIPTION
    Unmounts all GVFS repos using the current GVFS install, swaps the binaries,
    then re-mounts everything. Supports switching in both directions.

    Must be run as Administrator (service management requires elevation).

.PARAMETER Mode
    "switch" — Switch from production GVFS to the .NET 10 AOT build
    "restore" — Switch back from .NET 10 AOT to production GVFS

.PARAMETER AotZipOrDir
    Path to either:
    - The gvfs-aot-net10.zip file
    - An extracted directory containing the AOT binaries
    Default: looks for gvfs-aot-net10.zip next to this script

.PARAMETER GvfsInstallDir
    Production GVFS install directory.
    Default: C:\Program Files\GVFS

.EXAMPLE
    # Switch to .NET 10 AOT (run as admin):
    .\Switch-GvfsAot.ps1 -Mode switch -AotZipOrDir .\gvfs-aot-net10.zip

    # Switch back to production:
    .\Switch-GvfsAot.ps1 -Mode restore
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateSet("switch", "restore")]
    [string]$Mode,

    [string]$AotZipOrDir = "",

    [string]$GvfsInstallDir = "C:\Program Files\GVFS"
)

$ErrorActionPreference = "Stop"

$BackupDir = Join-Path $GvfsInstallDir ".backup-production"

# ─────────────────────────────────────────────────────────────────────────────
# Helpers
# ─────────────────────────────────────────────────────────────────────────────

function Test-Admin {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]$identity
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Get-MountedRepos {
    # Query the GVFS service for active repos
    try {
        $output = & "$GvfsInstallDir\GVFS.exe" service --list-mounted 2>&1 | Out-String
        $repos = $output.Trim().Split("`n") | Where-Object { $_ -and $_.Trim() -ne "" } | ForEach-Object { $_.Trim() }
        return $repos
    } catch {
        # Fallback: look at GVFS service registry
        $regPath = Join-Path $GvfsInstallDir "ProgramData\GVFS.Service\repo-registry"
        if (-not (Test-Path $regPath)) {
            $regPath = "C:\ProgramData\GVFS.Service\repo-registry"
        }
        if (Test-Path $regPath) {
            $lines = Get-Content $regPath | Select-Object -Skip 1  # skip version line
            $repos = @()
            foreach ($line in $lines) {
                if ($line.Trim()) {
                    $obj = $line | ConvertFrom-Json -EA 0
                    if ($obj -and $obj.IsActive) {
                        $repos += $obj.EnlistmentRoot
                    }
                }
            }
            return $repos
        }
        return @()
    }
}

function Stop-GvfsService {
    Write-Host "  Stopping GVFS.Service..." -NoNewline
    sc.exe stop GVFS.Service 2>&1 | Out-Null
    # Wait for it to actually stop
    for ($i = 0; $i -lt 30; $i++) {
        $svc = Get-Service GVFS.Service -EA 0
        if (-not $svc -or $svc.Status -eq 'Stopped') { break }
        Start-Sleep 1
    }
    # Also kill any remaining GVFS processes
    Get-Process GVFS.Mount, GVFS.Service, GVFS.Service.UI -EA 0 | Stop-Process -Force -EA 0
    Write-Host " OK" -ForegroundColor Green
}

function Start-GvfsService {
    Write-Host "  Starting GVFS.Service..." -NoNewline
    sc.exe start GVFS.Service 2>&1 | Out-Null
    Start-Sleep 3
    Write-Host " OK" -ForegroundColor Green
}

# ─────────────────────────────────────────────────────────────────────────────
# Preflight
# ─────────────────────────────────────────────────────────────────────────────

if (-not (Test-Admin)) {
    Write-Host "ERROR: This script must be run as Administrator." -ForegroundColor Red
    Write-Host "Right-click PowerShell -> 'Run as administrator', then run this script again."
    exit 1
}

if (-not (Test-Path $GvfsInstallDir)) {
    Write-Host "ERROR: GVFS install directory not found at $GvfsInstallDir" -ForegroundColor Red
    exit 1
}

Write-Host "=== GVFS AOT Switcher ===" -ForegroundColor Cyan
Write-Host "Mode:    $Mode"
Write-Host "Install: $GvfsInstallDir"
Write-Host ""

# ─────────────────────────────────────────────────────────────────────────────
# SWITCH mode: production → .NET 10 AOT
# ─────────────────────────────────────────────────────────────────────────────
if ($Mode -eq "switch") {

    # Resolve AOT source
    if (-not $AotZipOrDir) {
        $AotZipOrDir = Join-Path $PSScriptRoot "gvfs-aot-net10.zip"
    }
    if (-not (Test-Path $AotZipOrDir)) {
        Write-Host "ERROR: AOT binaries not found at $AotZipOrDir" -ForegroundColor Red
        Write-Host "Provide -AotZipOrDir pointing to gvfs-aot-net10.zip or an extracted directory."
        exit 1
    }

    # Extract zip if needed
    $aotDir = $AotZipOrDir
    if ($AotZipOrDir -like "*.zip") {
        $aotDir = Join-Path $env:TEMP "gvfs-aot-extracted"
        Write-Host "  Extracting $AotZipOrDir..." -NoNewline
        Remove-Item $aotDir -Recurse -Force -EA 0
        Expand-Archive -Path $AotZipOrDir -DestinationPath $aotDir -Force
        Write-Host " OK" -ForegroundColor Green
    }

    # Verify AOT binaries
    $requiredExes = @("GVFS.exe", "GVFS.Mount.exe", "GVFS.Service.exe", "GVFS.Hooks.exe")
    foreach ($exe in $requiredExes) {
        if (-not (Test-Path (Join-Path $aotDir $exe))) {
            Write-Host "ERROR: Missing $exe in AOT directory" -ForegroundColor Red
            exit 1
        }
    }

    $aotVersion = (Get-Item (Join-Path $aotDir "GVFS.exe")).VersionInfo.ProductVersion
    $prodVersion = (Get-Item (Join-Path $GvfsInstallDir "GVFS.exe")).VersionInfo.ProductVersion
    Write-Host "  Production version: $prodVersion"
    Write-Host "  AOT version:        $aotVersion"
    Write-Host ""

    # Check if already switched
    if (Test-Path $BackupDir) {
        Write-Host "WARNING: Backup directory already exists at $BackupDir" -ForegroundColor Yellow
        Write-Host "  It looks like you've already switched. Run with -Mode restore first."
        Write-Host "  Or delete $BackupDir manually if you're sure."
        exit 1
    }

    # Step 1: Discover mounted repos
    Write-Host "--- Step 1: Discovering mounted repos ---" -ForegroundColor Yellow
    $repos = Get-MountedRepos
    if ($repos.Count -gt 0) {
        Write-Host "  Found $($repos.Count) mounted repo(s):"
        $repos | ForEach-Object { Write-Host "    $_" }
    } else {
        Write-Host "  No mounted repos found."
    }

    # Step 2: Unmount all repos
    Write-Host ""
    Write-Host "--- Step 2: Unmounting repos ---" -ForegroundColor Yellow
    foreach ($repo in $repos) {
        Write-Host "  Unmounting $repo..." -NoNewline
        & "$GvfsInstallDir\GVFS.exe" unmount $repo --skip-wait-for-lock 2>&1 | Out-Null
        Write-Host " OK" -ForegroundColor Green
    }

    # Step 3: Stop GVFS service
    Write-Host ""
    Write-Host "--- Step 3: Stopping GVFS service ---" -ForegroundColor Yellow
    Stop-GvfsService

    # Step 4: Backup production binaries
    Write-Host ""
    Write-Host "--- Step 4: Backing up production binaries ---" -ForegroundColor Yellow
    Write-Host "  Backup: $BackupDir"
    New-Item -ItemType Directory -Path $BackupDir -Force | Out-Null

    # Only backup the exe files (not the enormous DLL set — those stay in place)
    $exesToReplace = Get-ChildItem $aotDir -Filter "*.exe" | Select-Object -ExpandProperty Name
    foreach ($exe in $exesToReplace) {
        $src = Join-Path $GvfsInstallDir $exe
        if (Test-Path $src) {
            Copy-Item $src $BackupDir -Force
            Write-Host "  Backed up: $exe"
        }
    }
    # Save the repo list for restore
    $repos | Out-File (Join-Path $BackupDir "mounted-repos.txt") -Encoding utf8

    # Step 5: Copy AOT binaries
    Write-Host ""
    Write-Host "--- Step 5: Installing AOT binaries ---" -ForegroundColor Yellow
    foreach ($exe in $exesToReplace) {
        $src = Join-Path $aotDir $exe
        $dst = Join-Path $GvfsInstallDir $exe
        Copy-Item $src $dst -Force
        $size = [math]::Round((Get-Item $dst).Length / 1MB, 1)
        Write-Host "  Installed: $exe (${size}MB)"
    }

    # Step 6: Start GVFS service
    Write-Host ""
    Write-Host "--- Step 6: Starting GVFS service ---" -ForegroundColor Yellow
    Start-GvfsService

    # Step 7: Re-mount repos
    Write-Host ""
    Write-Host "--- Step 7: Re-mounting repos ---" -ForegroundColor Yellow
    foreach ($repo in $repos) {
        Write-Host "  Mounting $repo..." -NoNewline
        & "$GvfsInstallDir\GVFS.exe" mount $repo 2>&1 | Out-Null
        Write-Host " OK" -ForegroundColor Green
    }

    Write-Host ""
    Write-Host "=== Switch complete ===" -ForegroundColor Cyan
    Write-Host "You are now running .NET 10 NativeAOT GVFS ($aotVersion)."
    Write-Host "To switch back: .\Switch-GvfsAot.ps1 -Mode restore"
    Write-Host ""
    & "$GvfsInstallDir\GVFS.exe" version
}

# ─────────────────────────────────────────────────────────────────────────────
# RESTORE mode: .NET 10 AOT → production
# ─────────────────────────────────────────────────────────────────────────────
elseif ($Mode -eq "restore") {

    if (-not (Test-Path $BackupDir)) {
        Write-Host "ERROR: No backup found at $BackupDir" -ForegroundColor Red
        Write-Host "  Nothing to restore. You may already be on the production build."
        exit 1
    }

    $currentVersion = (Get-Item (Join-Path $GvfsInstallDir "GVFS.exe")).VersionInfo.ProductVersion
    Write-Host "  Current version (AOT): $currentVersion"
    Write-Host ""

    # Step 1: Discover mounted repos
    Write-Host "--- Step 1: Discovering mounted repos ---" -ForegroundColor Yellow
    $repos = Get-MountedRepos
    # Also load the saved repo list from backup
    $savedRepos = @()
    $savedReposFile = Join-Path $BackupDir "mounted-repos.txt"
    if (Test-Path $savedReposFile) {
        $savedRepos = Get-Content $savedReposFile | Where-Object { $_.Trim() }
    }
    # Merge: use current mounted repos + any from saved list
    $allRepos = ($repos + $savedRepos) | Select-Object -Unique
    if ($allRepos.Count -gt 0) {
        Write-Host "  Found $($allRepos.Count) repo(s) to re-mount:"
        $allRepos | ForEach-Object { Write-Host "    $_" }
    }

    # Step 2: Unmount all repos
    Write-Host ""
    Write-Host "--- Step 2: Unmounting repos ---" -ForegroundColor Yellow
    foreach ($repo in $repos) {
        Write-Host "  Unmounting $repo..." -NoNewline
        & "$GvfsInstallDir\GVFS.exe" unmount $repo --skip-wait-for-lock 2>&1 | Out-Null
        Write-Host " OK" -ForegroundColor Green
    }

    # Step 3: Stop GVFS service
    Write-Host ""
    Write-Host "--- Step 3: Stopping GVFS service ---" -ForegroundColor Yellow
    Stop-GvfsService

    # Step 4: Restore production binaries
    Write-Host ""
    Write-Host "--- Step 4: Restoring production binaries ---" -ForegroundColor Yellow
    $backedUpExes = Get-ChildItem $BackupDir -Filter "*.exe" | Select-Object -ExpandProperty Name
    foreach ($exe in $backedUpExes) {
        $src = Join-Path $BackupDir $exe
        $dst = Join-Path $GvfsInstallDir $exe
        Copy-Item $src $dst -Force
        Write-Host "  Restored: $exe"
    }

    # Step 5: Remove backup
    Remove-Item $BackupDir -Recurse -Force
    Write-Host "  Backup cleaned up."

    # Step 6: Start GVFS service
    Write-Host ""
    Write-Host "--- Step 6: Starting GVFS service ---" -ForegroundColor Yellow
    Start-GvfsService

    # Step 7: Re-mount repos
    Write-Host ""
    Write-Host "--- Step 7: Re-mounting repos ---" -ForegroundColor Yellow
    foreach ($repo in $allRepos) {
        if (Test-Path $repo) {
            Write-Host "  Mounting $repo..." -NoNewline
            & "$GvfsInstallDir\GVFS.exe" mount $repo 2>&1 | Out-Null
            Write-Host " OK" -ForegroundColor Green
        } else {
            Write-Host "  Skipping $repo (path not found)" -ForegroundColor DarkGray
        }
    }

    $restoredVersion = (Get-Item (Join-Path $GvfsInstallDir "GVFS.exe")).VersionInfo.ProductVersion
    Write-Host ""
    Write-Host "=== Restore complete ===" -ForegroundColor Cyan
    Write-Host "You are back on production GVFS ($restoredVersion)."
    Write-Host ""
    & "$GvfsInstallDir\GVFS.exe" version
}
