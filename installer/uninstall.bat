@echo off
setlocal EnableDelayedExpansion
chcp 65001 >nul
title Désinstallation d'AeroPDF

echo ======================================================
echo          Désinstallation de AeroPDF
echo ======================================================
echo.

set "INSTALL_DIR=%LOCALAPPDATA%\Programs\AeroPDF"

echo [*] Suppression des raccourcis...
powershell -NoProfile -Command ^
  "$desktop = [Environment]::GetFolderPath('Desktop') + '\AeroPDF.lnk'; if (Test-Path $desktop) { Remove-Item $desktop }; " ^
  "$start = [Environment]::GetFolderPath('Programs') + '\AeroPDF.lnk'; if (Test-Path $start) { Remove-Item $start };"

echo [*] Suppression des fichiers installés...
if exist "%INSTALL_DIR%" (
    rmdir /S /Q "%INSTALL_DIR%"
)

echo.
echo ======================================================
echo         AeroPDF a été désinstallé avec succès.
echo ======================================================
echo.
pause
