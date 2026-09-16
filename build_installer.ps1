# Script de packaging pour AeroPDF
$ErrorActionPreference = "Stop"

Write-Host "=== Compilation et Packaging d'AeroPDF ===" -ForegroundColor Cyan

# 1. Compilation Release
Write-Host "`n[1/4] Compilation en mode Release..." -ForegroundColor Yellow
$env:Path = [System.Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [System.Environment]::GetEnvironmentVariable('Path','User')
cargo build --release

$targetExe = "target\release\aeropdf.exe"
$pdfiumDll = "pdfium.dll"

if (-not (Test-Path $targetExe)) {
    throw "Exécutable introuvable : $targetExe"
}
if (-not (Test-Path $pdfiumDll)) {
    throw "DLL introuvable : $pdfiumDll"
}

# 2. Préparation du dossier de distribution
Write-Host "`n[2/4] Préparation des fichiers..." -ForegroundColor Yellow
$distDir = "dist\AeroPDF"
if (Test-Path $distDir) {
    Remove-Item -Recurse -Force $distDir
}
New-Item -ItemType Directory -Path $distDir | Out-Null

Copy-Item $targetExe $distDir
Copy-Item $pdfiumDll $distDir
Copy-Item "installer\install.bat" $distDir
Copy-Item "installer\uninstall.bat" $distDir

# Fichier d'information pour l'utilisateur final
@"
======================================================
                  AeroPDF v0.1.0
      Lecteur PDF ultra-léger et instantané
======================================================

INSTALLATION :
  Double-cliquez sur 'install.bat' pour installer AeroPDF sur
  votre machine (créera un raccourci Bureau et Menu Démarrer).

UTILISATION PORTABLE :
  Vous pouvez aussi simplement lancer 'aeropdf.exe' directement
  depuis ce dossier (aucune installation requise).

RACCOURCIS CLAVIER :
  - Espace / Maj+Espace : Défilement fluide
  - Molette souris      : Défilement inertiel
  - Ctrl + Molette      : Zoom centré sur la souris
  - Ctrl + 0            : Réinitialiser le zoom (100%)
  - Ctrl + F            : Recherche textuelle instantanée
  - Ctrl + G            : Aller à une page
  - Ctrl + T / B        : Sommaire / Signets du document
  - Ctrl + D            : Mode double-page
  - Ctrl + I ou N       : Mode Nuit (inversion intelligente)
  - Ctrl + R ou R       : Rotation (90°, 180°, 270°)
  - Ctrl + O            : Ouvrir un autre PDF
  - Ctrl + P            : Imprimer le document
  - Ctrl + C            : Copier le texte sélectionné
  - F11                 : Plein écran
  - Echap ou Q          : Quitter
"@ | Set-Content -Path "$distDir\LISEZ-MOI.txt" -Encoding UTF8

# 3. Création de l'archive ZIP
Write-Host "`n[3/4] Création du fichier ZIP d'installation..." -ForegroundColor Yellow
$zipOutput = "dist\AeroPDF-v0.1.0-Windows-x64.zip"
if (Test-Path $zipOutput) {
    Remove-Item -Force $zipOutput
}
Compress-Archive -Path "$distDir\*" -DestinationPath $zipOutput

# 4. Résumé
$sizeMb = [math]::Round((Get-Item $zipOutput).Length / 1MB, 2)
Write-Host "`n[4/4] Paquet créé avec succès !" -ForegroundColor Green
Write-Host "Archive : $zipOutput ($sizeMb Mo)" -ForegroundColor Green
Write-Host "Vous pouvez partager ce fichier ZIP pour installer AeroPDF sur n'importe quel PC Windows." -ForegroundColor Cyan
