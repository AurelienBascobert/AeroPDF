# Script de packaging et génération d'installeur pour AeroPDF
$ErrorActionPreference = "Stop"

Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "        Création de l'installateur AeroPDF           " -ForegroundColor Cyan
Write-Host "======================================================" -ForegroundColor Cyan

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

# 2. Création du dossier dist
$distDir = "dist"
if (-not (Test-Path $distDir)) {
    New-Item -ItemType Directory -Path $distDir | Out-Null
}

# 3. Génération du vrai installeur .EXE avec Inno Setup
Write-Host "`n[2/4] Création du VRAI installeur exécutable autonome (.EXE)..." -ForegroundColor Yellow

$isccCandidates = @(
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
    "C:\Program Files\Inno Setup 6\ISCC.exe",
    (Get-Command iscc.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source)
)

$isccPath = $null
foreach ($c in $isccCandidates) {
    if ($c -and (Test-Path $c)) {
        $isccPath = $c
        break
    }
}

$setupExe = "$distDir\AeroPDF-Setup-v0.1.0.exe"

if ($isccPath) {
    Write-Host "Utilisation du compilateur Inno Setup : $isccPath" -ForegroundColor Gray
    & "$isccPath" "installer\aeropdf.iss"
    if (Test-Path $setupExe) {
        $setupMb = [math]::Round((Get-Item $setupExe).Length / 1MB, 2)
        Write-Host "-> Installeur autonome généré : $setupExe ($setupMb Mo)" -ForegroundColor Green
    }
} else {
    Write-Warning "Inno Setup (ISCC.exe) introuvable. Installez-le avec : winget install JRSoftware.InnoSetup"
}

# 4. Création de l'archive ZIP Portable
Write-Host "`n[3/4] Création du package portable ZIP..." -ForegroundColor Yellow
$portableDir = "$distDir\portable_bundle"
if (Test-Path $portableDir) { Remove-Item -Recurse -Force $portableDir }
New-Item -ItemType Directory -Path $portableDir | Out-Null

Copy-Item $targetExe $portableDir
Copy-Item $pdfiumDll $portableDir
Copy-Item "installer\install.bat" $portableDir
Copy-Item "installer\uninstall.bat" $portableDir

@"
======================================================
                  AeroPDF v0.1.0
      Lecteur PDF ultra-léger et instantané
======================================================

INSTALLATEUR OFFICIEL :
  Double-cliquez sur 'AeroPDF-Setup-v0.1.0.exe' pour installer
  AeroPDF comme une vraie application Windows (avec raccourcis,
  désinstalleur dans Paramètres Windows et association .pdf).

VERSION PORTABLE :
  Vous pouvez aussi simplement lancer 'aeropdf.exe' directement
  depuis ce dossier (aucune installation requise).
"@ | Set-Content -Path "$portableDir\LISEZ-MOI.txt" -Encoding UTF8

$zipOutput = "$distDir\AeroPDF-v0.1.0-Portable-x64.zip"
if (Test-Path $zipOutput) { Remove-Item -Force $zipOutput }
Compress-Archive -Path "$portableDir\*" -DestinationPath $zipOutput
Remove-Item -Recurse -Force $portableDir

# 5. Résumé final
Write-Host "`n======================================================" -ForegroundColor Green
Write-Host "                Génération terminée !                 " -ForegroundColor Green
Write-Host "======================================================" -ForegroundColor Green
if (Test-Path $setupExe) {
    Write-Host " 1. VRAI INSTALLATEUR .EXE : $setupExe" -ForegroundColor Cyan
}
Write-Host " 2. ARCHIVE PORTABLE .ZIP : $zipOutput" -ForegroundColor Cyan
Write-Host "`nVous pouvez partager directement '$setupExe' à n'importe qui !" -ForegroundColor White
