; Script Inno Setup pour AeroPDF
; Produit un installateur exécutable autonome unique AeroPDF-Setup-v0.1.0.exe

#define MyAppName "AeroPDF"
#define MyAppVersion "0.1.0"
#define MyAppPublisher "Aurelien Bascobert"
#define MyAppURL "https://github.com/AurelienBascobert/AeroPDF"
#define MyAppExeName "aeropdf.exe"

[Setup]
; Identifiant unique pour les mises à jour et la désinstallation
AppId={{D3A81D92-7E59-4E6F-A0C3-8B3B9B9E77F1}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}

; Permet l'installation sans droits administrateurs ou pour toute la machine
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog

; Emplacement par défaut : %LOCALAPPDATA%\Programs\AeroPDF en mode utilisateur
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes

; Nom et dossier du fichier exécutable Setup de sortie
OutputDir=..\dist
OutputBaseFilename=AeroPDF-Setup-v0.1.0
SetupIconFile=
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern

; Enregistrement propre dans Paramètres Windows > Applications
UninstallDisplayIcon={app}\{#MyAppExeName}
UninstallDisplayName={#MyAppName}

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "associatepdf"; Description: "Associer AeroPDF comme lecteur pour les fichiers PDF (.pdf)"; GroupDescription: "Associations de fichiers :"; Flags: unchecked

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\pdfium.dll"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
; Association optionnelle des fichiers .pdf
Root: HKA; Subkey: "Software\Classes\.pdf\OpenWithProgids"; ValueType: string; ValueName: "AeroPDF.Document"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associatepdf
Root: HKA; Subkey: "Software\Classes\AeroPDF.Document"; ValueType: string; ValueName: ""; ValueData: "Document PDF"; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKA; Subkey: "Software\Classes\AeroPDF.Document\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey; Tasks: associatepdf

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
