@echo off
echo === GVFS Emergency Restore ===
echo.
echo Stopping GVFS services and processes...
sc stop GVFS.Service >nul 2>&1
timeout /t 3 /nobreak >nul
taskkill /F /IM GVFS.exe >nul 2>&1
taskkill /F /IM GVFS.Mount.exe >nul 2>&1
taskkill /F /IM GVFS.Service.exe >nul 2>&1
taskkill /F /IM GVFS.Service.UI.exe >nul 2>&1
timeout /t 2 /nobreak >nul

echo Restoring production binaries...
set BACKUP=C:\Program Files\GVFS\.backup-production
if exist "%BACKUP%" (
    for %%f in ("%BACKUP%\*.exe") do (
        copy /Y "%%f" "C:\Program Files\GVFS\%%~nxf" >nul
        echo   Restored: %%~nxf
    )
    rmdir /S /Q "%BACKUP%"
    echo   Backup removed.
) else (
    echo   ERROR: No backup found at %BACKUP%
    pause
    exit /b 1
)

echo Starting GVFS service...
sc start GVFS.Service >nul 2>&1
timeout /t 3 /nobreak >nul

echo Verifying...
"C:\Program Files\GVFS\GVFS.exe" version

echo.
echo Mounting D:\os...
"C:\Program Files\GVFS\GVFS.exe" mount D:\os

echo.
echo === Done ===
pause
