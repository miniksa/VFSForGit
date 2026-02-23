<#
.SYNOPSIS
    Publish VFSForGit .NET 10 NativeAOT binaries and create installer layout.

.DESCRIPTION
    Builds all GVFS projects as self-contained NativeAOT executables for win-x64,
    then assembles them into a flat layout directory suitable for the Inno Setup
    installer (Setup.iss).

    The NativeAOT layout is dramatically simpler than the .NET Framework layout:
    just 9 self-contained .exe files instead of ~150 DLLs + runtimes.

.PARAMETER Configuration
    Build configuration: Debug or Release (default: Release)

.PARAMETER OutputDir
    Layout output directory (default: $RepoRoot\..\out\gvfs-aot-layout)

.PARAMETER SkipBuild
    Skip the dotnet publish step (use existing build output)

.PARAMETER BuildInstaller
    Also build the Inno Setup installer after creating the layout
#>
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",
    [string]$OutputDir = "",
    [switch]$SkipBuild,
    [switch]$BuildInstaller
)

$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$GVFSRoot = Join-Path $RepoRoot "GVFS"
$OutRoot = if ($OutputDir) { $OutputDir } else { Join-Path (Split-Path $RepoRoot) "out\gvfs-aot-layout" }

Write-Host "=== VFSForGit NativeAOT Publish ===" -ForegroundColor Cyan
Write-Host "Configuration: $Configuration"
Write-Host "Output:        $OutRoot"
Write-Host ""

# ─────────────────────────────────────────────────────────────────────────────
# Managed projects that get published as NativeAOT
# ─────────────────────────────────────────────────────────────────────────────
$ManagedProjects = @(
    @{ Name = "GVFS";              Dir = "GVFS";              Exe = "GVFS.exe" },
    @{ Name = "GVFS.Mount";        Dir = "GVFS.Mount";        Exe = "GVFS.Mount.exe" },
    @{ Name = "GVFS.Hooks";        Dir = "GVFS.Hooks";        Exe = "GVFS.Hooks.exe" },
    @{ Name = "GVFS.Service";      Dir = "GVFS.Service";      Exe = "GVFS.Service.exe" },
    @{ Name = "GVFS.Service.UI";   Dir = "GVFS.Service.UI";   Exe = "GVFS.Service.UI.exe" }
)

# Native C++ projects (built with MSBuild, not dotnet publish)
$NativeProjects = @(
    @{ Name = "GitHooksLoader";           Dir = "GitHooksLoader";           Exe = "GitHooksLoader.exe" },
    @{ Name = "GVFS.ReadObjectHook";      Dir = "GVFS.ReadObjectHook";     Exe = "GVFS.ReadObjectHook.exe" },
    @{ Name = "GVFS.PostIndexChangedHook"; Dir = "GVFS.PostIndexChangedHook"; Exe = "GVFS.PostIndexChangedHook.exe" },
    @{ Name = "GVFS.VirtualFileSystemHook"; Dir = "GVFS.VirtualFileSystemHook"; Exe = "GVFS.VirtualFileSystemHook.exe" }
)

# ─────────────────────────────────────────────────────────────────────────────
# Step 1: Build / Publish
# ─────────────────────────────────────────────────────────────────────────────
if (-not $SkipBuild) {
    Write-Host "--- Step 1: Publishing NativeAOT binaries ---" -ForegroundColor Yellow

    foreach ($proj in $ManagedProjects) {
        $csproj = Join-Path $GVFSRoot "$($proj.Dir)\$($proj.Name).csproj"
        if (-not (Test-Path $csproj)) {
            Write-Warning "Project not found: $csproj — skipping"
            continue
        }

        Write-Host "  Publishing $($proj.Name)..." -NoNewline
        $sw = [Diagnostics.Stopwatch]::StartNew()

        dotnet publish $csproj `
            -c $Configuration `
            -r win-x64 `
            --self-contained true `
            -o "$OutRoot" `
            2>&1 | Out-Null

        if ($LASTEXITCODE -ne 0) {
            Write-Host " FAILED" -ForegroundColor Red
            throw "dotnet publish failed for $($proj.Name)"
        }

        $sw.Stop()
        $size = if (Test-Path "$OutRoot\$($proj.Exe)") {
            [math]::Round((Get-Item "$OutRoot\$($proj.Exe)").Length / 1MB, 1)
        } else { "?" }
        Write-Host " OK (${size}MB, $([math]::Round($sw.Elapsed.TotalSeconds, 1))s)" -ForegroundColor Green
    }

    # Build native projects with MSBuild
    Write-Host ""
    Write-Host "  Building native hooks..." -NoNewline

    # Find MSBuild
    $msbuildExe = $null
    # Try vswhere first
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $vsPath = & $vswhere -latest -requires Microsoft.Component.MSBuild -property installationPath 2>$null
        if ($vsPath) {
            $msbuildExe = Join-Path $vsPath "MSBuild\Current\Bin\amd64\MSBuild.exe"
            if (-not (Test-Path $msbuildExe)) {
                $msbuildExe = Join-Path $vsPath "MSBuild\Current\Bin\MSBuild.exe"
            }
        }
    }
    # Fallback: check PATH
    if (-not $msbuildExe -or -not (Test-Path $msbuildExe)) {
        $msbuildExe = Get-Command msbuild.exe -EA 0 | Select-Object -ExpandProperty Source
    }

    if ($msbuildExe -and (Test-Path $msbuildExe)) {
        foreach ($proj in $NativeProjects) {
            $vcxproj = Get-ChildItem -Path $GVFSRoot -Recurse -Filter "$($proj.Name).vcxproj" | Select-Object -First 1
            if ($vcxproj) {
                & $msbuildExe $vcxproj.FullName /p:Configuration=$Configuration /p:Platform=x64 /v:minimal /nologo 2>&1 | Out-Null
                # Copy native exe to layout
                $nativeExe = Join-Path (Split-Path $RepoRoot) "out\$($proj.Name)\bin\x64\$Configuration\$($proj.Exe)"
                if (Test-Path $nativeExe) {
                    Copy-Item $nativeExe $OutRoot -Force
                }
            }
        }
        Write-Host " OK" -ForegroundColor Green
    } else {
        Write-Host " SKIPPED (MSBuild not found — native hooks will use existing binaries)" -ForegroundColor Yellow
    }
} else {
    Write-Host "--- Step 1: Skipped (using existing build output) ---" -ForegroundColor DarkGray
}

# ─────────────────────────────────────────────────────────────────────────────
# Step 2: Assemble Layout
# ─────────────────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "--- Step 2: Verifying layout ---" -ForegroundColor Yellow

# Create required subdirectories
New-Item -ItemType Directory -Path "$OutRoot\ProgramData\GVFS.Service" -Force | Out-Null

# Copy icon if present
$icon = Join-Path $GVFSRoot "GVFS\GitVirtualFileSystem.ico"
if (Test-Path $icon) {
    Copy-Item $icon $OutRoot -Force
}

# Write version marker file
$versionStr = if (Test-Path "$OutRoot\GVFS.exe") {
    (Get-Item "$OutRoot\GVFS.exe" | Select-Object -ExpandProperty VersionInfo).ProductVersion
} else { "0.0.0.0" }
"" | Out-File "$OutRoot\OnDiskVersion16CapableInstallation.dat" -Encoding ascii

# Verify all required executables exist
$allExes = ($ManagedProjects + $NativeProjects) | ForEach-Object { $_.Exe }
$missing = @()
foreach ($exe in $allExes) {
    $path = Join-Path $OutRoot $exe
    if (Test-Path $path) {
        $size = [math]::Round((Get-Item $path).Length / 1MB, 1)
        Write-Host "  [OK] $exe (${size}MB)" -ForegroundColor Green
    } else {
        Write-Host "  [MISSING] $exe" -ForegroundColor Red
        $missing += $exe
    }
}

if ($missing.Count -gt 0) {
    Write-Warning "Missing $($missing.Count) executable(s). Layout is incomplete."
} else {
    Write-Host ""
    Write-Host "  Layout complete: $($allExes.Count) executables" -ForegroundColor Cyan
    $totalSize = [math]::Round((Get-ChildItem $OutRoot -File | Measure-Object Length -Sum).Sum / 1MB, 1)
    Write-Host "  Total size: ${totalSize}MB" -ForegroundColor Cyan
}

# ─────────────────────────────────────────────────────────────────────────────
# Step 3: Build Installer (optional)
# ─────────────────────────────────────────────────────────────────────────────
if ($BuildInstaller) {
    Write-Host ""
    Write-Host "--- Step 3: Building Installer ---" -ForegroundColor Yellow

    # Find Inno Setup compiler
    $iscc = $null
    # Check NuGet package first
    $nugetIscc = Get-ChildItem "$env:USERPROFILE\.nuget\packages\tools.innosetup" -Recurse -Filter "ISCC.exe" -EA 0 | Select-Object -First 1
    if ($nugetIscc) {
        $iscc = $nugetIscc.FullName
    }
    # Check Program Files
    if (-not $iscc) {
        $progIscc = "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
        if (Test-Path $progIscc) { $iscc = $progIscc }
    }

    if (-not $iscc) {
        Write-Warning "Inno Setup compiler (ISCC.exe) not found. Install via: winget install JRSoftware.InnoSetup"
        Write-Warning "Or: dotnet restore GVFS\GVFS.Installers\GVFS.Installers.csproj (downloads Tools.InnoSetup NuGet)"
    } else {
        $setupIss = Join-Path $GVFSRoot "GVFS.Installers\Setup.iss"
        $installerOut = Join-Path $OutRoot "..\installer"
        New-Item -ItemType Directory -Path $installerOut -Force | Out-Null

        $gvfsVersion = $versionStr
        Write-Host "  ISCC: $iscc"
        Write-Host "  Version: $gvfsVersion"
        Write-Host "  Building installer..."

        & $iscc /DLayoutDir="$OutRoot" /DGVFSVersion=$gvfsVersion $setupIss /O"$installerOut" 2>&1

        if ($LASTEXITCODE -eq 0) {
            $installer = Get-ChildItem $installerOut -Filter "SetupGVFS*.exe" | Select-Object -First 1
            if ($installer) {
                $instSize = [math]::Round($installer.Length / 1MB, 1)
                Write-Host "  Installer: $($installer.FullName) (${instSize}MB)" -ForegroundColor Green
            }
        } else {
            Write-Warning "Installer build failed (exit code $LASTEXITCODE)"
        }
    }
}

Write-Host ""
Write-Host "=== Done ===" -ForegroundColor Cyan
Write-Host "Layout: $OutRoot"
if ($BuildInstaller) {
    Write-Host "Installer: $(Join-Path $OutRoot '..\installer')"
}
