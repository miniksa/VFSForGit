@echo off
REM Copies all required executables into the GVFS.exe output directory
REM to replicate the production co-located layout needed for clone/mount.
REM Run this after 'dotnet build' and 'msbuild' (for C++ hooks).

setlocal
set CONFIG=%1
if "%CONFIG%"=="" set CONFIG=Debug

set TFM=net10.0-windows10.0.17763.0
set RID=win-x64
set OUT=%~dp0..\out
set DEST=%OUT%\GVFS\bin\%CONFIG%\%TFM%\%RID%

echo Copying managed peer executables to %DEST%...
for %%P in (GVFS.Mount GVFS.Service GVFS.Service.UI) do (
    if exist "%OUT%\%%P\bin\%CONFIG%\%TFM%\%RID%\%%P.exe" (
        xcopy /Y /Q "%OUT%\%%P\bin\%CONFIG%\%TFM%\%RID%\%%P.*" "%DEST%\" >nul
        echo   %%P: OK
    ) else (
        echo   %%P: MISSING
    )
)

echo Copying native C++ hooks...
for %%H in (GitHooksLoader GVFS.ReadObjectHook GVFS.PostIndexChangedHook GVFS.VirtualFileSystemHook) do (
    if exist "%OUT%\%%H\bin\x64\%CONFIG%\%%H.exe" (
        xcopy /Y /Q "%OUT%\%%H\bin\x64\%CONFIG%\%%H.exe" "%DEST%\" >nul
        echo   %%H: OK
    ) else (
        echo   %%H: MISSING (build with VS MSBuild first)
    )
)

echo Done.
