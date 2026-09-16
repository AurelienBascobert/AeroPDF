# ⚡ AeroPDF

Un lecteur de PDF minimaliste, ultra-rapide et épuré en **Rust**, conçu spécifiquement pour la consultation instantanée, la recherche et la sélection de texte.

---

## ✨ Fonctionnalités

- **Démarrage instantané (< 20 ms)** : Propulsé par `eframe` (egui) et le moteur de rendu C++ de Chromium (`PDFium`).
- **Interface Pop-up Borderless** : Fenêtre sans bordure encombrante, avec coins arrondis et ombre douce.
- **Rendu Multi-thread haute performance** : Pool de workers exploitant tous les cœurs CPU pour un défilement ultra-fluide.
- **Recherche textuelle asynchrone (`Ctrl + F`)** : Recherche instantanée en arrière-plan sans bloquer l'interface.
- **Table des matières & Signets (`Ctrl + T`)** : Navigation rapide dans les chapitres avec filtre de recherche.
- **Saut à la page (`Ctrl + G`)** : Modal rapide pour atteindre directement une page.
- **Mode Nuit intelligent (`Ctrl + I` ou `N`)** : Inversion soignée des couleurs pour le confort nocturne.
- **Mode Double Page (`Ctrl + D`)** : Affichage côte à côte optimisé pour grands écrans.
- **Rotation (`Ctrl + R` ou `R`)** : Rotation à 90°, 180° et 270°.
- **Impression directe (`Ctrl + P`)** : Envoi direct à l'imprimante Windows par défaut.
- **Surlignage de texte élégant** : Ruban de sélection continu moderne (style Chrome / macOS).
- **Copie Presse-papier instantanée** : `Ctrl + C` copie le texte sélectionné avec notification discrète.
- **Persistance des préférences & reprise de lecture** : Mémorise automatiquement la dernière page lue et vos réglages (zoom, mode nuit, mode double-page).
- **Ouverture rapide** :
  - En ligne de commande : `aeropdf fichier.pdf`
  - Glisser-déposer (Drag & Drop) direct dans la fenêtre
  - Sélecteur de fichier avec `Ctrl + O` ou via l'écran d'accueil

---

## ⌨️ Raccourcis Clavier

| Raccourci | Action |
| :--- | :--- |
| **`Espace`** / **`Maj + Espace`** | Défilement rapide d'une page |
| **`J` / `K`** ou **Flèches Bas / Haut** | Défilement ligne par ligne |
| **`Page Down` / `Page Up`** | Page suivante / précédente |
| **`Ctrl + F`** | Barre de recherche textuelle flottante |
| **`Ctrl + G`** | Aller directement à une page |
| **`Ctrl + T`** ou **`Ctrl + B`** | Sommaire / Table des matières |
| **`Ctrl + D`** | Basculer Mode Double Page / Simple Page |
| **`Ctrl + I`** ou **`N`** | Basculer Mode Sombre / Mode Clair |
| **`Ctrl + R`** ou **`R`** | Tourner la page (rotation 90°) |
| **`Ctrl + P`** | Imprimer le document |
| **`Ctrl + C`** | Copier le texte sélectionné |
| **`Ctrl + O`** | Ouvrir un nouveau fichier PDF |
| **`Ctrl + Molette`** | Zoomer / Dézoomer |
| **`Ctrl + 0`** | Réinitialiser le zoom (100%) |
| **`F11`** | Activer / désactiver le plein écran |
| **`Echap`** ou **`Q`** | Fermer l'application immédiatement |

---

## 📦 Installation & Déploiement

### 1. Installateur Windows officiel (Recommandé)
Téléchargez et lancez **`AeroPDF-Setup-v0.1.0.exe`** (disponible dans les [Releases](https://github.com/AurelienBascobert/AeroPDF/releases)) :
- Assistant d'installation graphique en français/anglais.
- Crée les raccourcis sur le **Bureau** et dans le **Menu Démarrer**.
- Permet d'associer automatiquement les fichiers `.pdf` à AeroPDF.
- Enregistré proprement dans les **Paramètres Windows > Applications installées** avec désinstalleur officiel.

### 2. Version Portable (.ZIP)
Téléchargez **`AeroPDF-v0.1.0-Portable-x64.zip`**, extrayez-le n'importe où et lancez directement `aeropdf.exe`.

### 3. Compilation depuis les sources
Nécessite [Rust](https://www.rust-lang.org/) installé :
```powershell
# Compiler en mode Release
cargo build --release

# Créer le package complet d'installation (ZIP)
powershell .\build_installer.ps1
```
L'archive prête à l'emploi sera générée dans `dist/AeroPDF-v0.1.0-Windows-x64.zip`.
