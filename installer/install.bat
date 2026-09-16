@echo off
setlocal EnableDelayedExpansion
chcp 65001 >nul
title Installation d'AeroPDF

echo ======================================================
echo           Installation de AeroPDF v0.1.0
echo ======================================================
echo.

set "INSTALL_DIR=%LOCALAPPDATA%\Programs\AeroPDF"
set "BIN_DIR=%~dp0"

echo [*] Dossier d'installation : %INSTALL_DIR%
if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"

echo [*] Copie des fichiers...
copy /Y "%BIN_DIR%aeropdf.exe" "%INSTALL_DIR%\aeropdf.exe" >nul
if %errorlevel% neq 0 (
    echo [!] Erreur: impossible de copier aeropdf.exe.
    echo     Fermez AeroPDF s'il est en cours d'execution et relancez.
    pause
    exit /b 1
)

copy /Y "%BIN_DIR%pdfium.dll" "%INSTALL_DIR%\pdfium.dll" >nul
if %errorlevel% neq 0 (
    echo [!] Erreur: impossible de copier pdfium.dll.
    pause
    exit /b 1
)

echo [*] Creation des raccourcis...
powershell -NoProfile -Command ^
  "$ws = New-Object -ComObject WScript.Shell; " ^
  "$sDesktop = $ws.CreateShortcut([Environment]::GetFolderPath('Desktop') + '\AeroPDF.lnk'); " ^
  "$sDesktop.TargetPath = '%INSTALL_DIR%\aeropdf.exe'; " ^
  "$sDesktop.WorkingDirectory = '%INSTALL_DIR%'; " ^
  "$sDesktop.Description = 'Lecteur PDF ultra-léger et instantané'; " ^
  "$sDesktop.Save(); " ^
  "$startDir = [Environment]::GetFolderPath('Programs'); " ^
  "$sStart = $ws.CreateShortcut($startDir + '\AeroPDF.lnk'); " ^
  "$sStart.TargetPath = '%INSTALL_DIR%\aeropdf.exe'; " ^
  "$sStart.WorkingDirectory = '%INSTALL_DIR%'; " ^
  "$sStart.Description = 'Lecteur PDF ultra-léger et instantané'; " ^
  "$sStart.Save();"

echo [*] Ajout de AeroPDF au PATH utilisateur...
powershell -NoProfile -Command ^
  "$p = [Environment]::GetEnvironmentVariable('Path', 'User'); " ^
  "if ($p -notlike '*%INSTALL_DIR%*') { [Environment]::SetEnvironmentVariable('Path', $p + ';%INSTALL_DIR%', 'User') }"

echo.
echo ======================================================
echo         Installation terminée avec succès !
echo ======================================================
echo.
echo  - Raccourci créé sur votre Bureau
echo  - Raccourci ajouté dans le Menu Démarrer
echo  - Vous pouvez aussi taper 'aeropdf <fichier.pdf>' dans un terminal
echo.
echo Voulez-vous lancer AeroPDF maintenant ? (O/N)
set /p LAUNCH="> "
if /i "%LAUNCH%"=="O" (
    start "" "%INSTALL_DIR%\aeropdf.exe"
)
