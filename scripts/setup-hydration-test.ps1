<#
.SYNOPSIS
    Clone D:\os3 (production GVFS) and D:\os4 (AOT GVFS) with separate caches
    for apples-to-apples hydration comparison.
#>
$ErrorActionPreference = "Continue"

$prodGVFS = "C:\Program Files\GVFS\.backup-production\GVFS.exe"
$aotGVFS  = "D:\src\out\gvfs-aot-layout\GVFS.exe"
$repo     = "https://dev.azure.com/microsoft/os/_git/os.2020"
$branch   = "official/ge_current_directes_corebuild"

# Check production backup exists (we're running AOT currently)
if (-not (Test-Path $prodGVFS)) {
    Write-Host "ERROR: Production GVFS backup not found at $prodGVFS" -ForegroundColor Red
    Write-Host "You need to have run Switch-GvfsAot.ps1 at least once to create the backup."
    exit 1
}

Write-Host "=== Hydration Comparison Setup ===" -ForegroundColor Cyan
Write-Host "Production: $prodGVFS"
Write-Host "AOT:        $aotGVFS"
Write-Host "Repo:       $repo"
Write-Host "Branch:     $branch"
Write-Host ""

# ─── Clone D:\os3 with production GVFS ───
Write-Host "--- Cloning D:\os3 (production GVFS, own cache) ---" -ForegroundColor Yellow
if (Test-Path D:\os3) { Write-Host "D:\os3 already exists, skipping clone" } else {
    & $prodGVFS clone $repo D:\os3 --branch $branch --local-cache-path D:\os3cache --no-prefetch 2>&1
}

# ─── Clone D:\os4 with AOT GVFS ───
Write-Host "--- Cloning D:\os4 (AOT GVFS, own cache) ---" -ForegroundColor Yellow
if (Test-Path D:\os4) { Write-Host "D:\os4 already exists, skipping clone" } else {
    & $aotGVFS clone $repo D:\os4 --branch $branch --local-cache-path D:\os4cache --no-prefetch 2>&1
}

Write-Host ""
Write-Host "=== Setup Complete ===" -ForegroundColor Cyan
Write-Host "D:\os3 → production GVFS (cache: D:\os3cache)"
Write-Host "D:\os4 → AOT GVFS (cache: D:\os4cache)"
Write-Host ""
Write-Host "Next steps:"
Write-Host "  1. Mount D:\os3:  & '$prodGVFS' mount D:\os3"
Write-Host "  2. Mount D:\os4:  & '$aotGVFS' mount D:\os4"  
Write-Host "  3. Run a small build in each and compare timing"
