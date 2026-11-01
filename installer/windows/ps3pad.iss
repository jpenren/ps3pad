; PS3Pad Windows installer (Inno Setup 6)
;
; Build locally:
;   ISCC.exe /DMyAppVersion=0.1.0 installer\windows\ps3pad.iss
;
; The CI release workflow builds this automatically and publishes
; ps3pad-windows-x86_64-setup.exe.

#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif

#define MyAppName "PS3Pad"
#define MyAppPublisher "ps3pad contributors"
#define MyAppURL "https://github.com/jpenren/ps3pad"
#define MyAppExeName "ps3pad.exe"
#define MyAppId "{{FEAF2AC6-B977-4155-A725-55EEFAB2A430}"

[Setup]
AppId={#MyAppId}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}/issues
AppUpdatesURL={#MyAppURL}/releases
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\..\dist
OutputBaseFilename=ps3pad-windows-x86_64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\{#MyAppExeName}
SetupLogging=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\..\target\x86_64-pc-windows-msvc\release\ps3pad.exe"; DestDir: "{app}"; Flags: ignoreversion

; Ship the KMDF/VHF driver files when they are present in the repository.
; They are not installed automatically; see the Windows driver documentation.
#if FileExists(AddBackslash(SourcePath) + "..\..\drivers\windows\ps3padvhf.inf")
Source: "..\..\drivers\windows\*"; DestDir: "{app}\drivers\windows"; Flags: ignoreversion recursesubdirs createallsubdirs
#endif

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Parameters: "gui"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Parameters: "gui"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Parameters: "gui"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent
