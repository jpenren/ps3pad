; PS3Pad Windows installer (Inno Setup 6)
;
; Build locally:
;   ISCC.exe /DMyAppVersion=0.1.0 installer\windows\ps3pad.iss
;
; The CI release workflow fetches the DsHidMini driver package with
; fetch-dshidmini.ps1, then builds this installer and publishes
; ps3pad-windows-x86_64-setup.exe.
;
; On Windows the physical DualShock 3 is driven by the signed DsHidMini UMDF
; driver (replacing hidusb for USB\VID_054C&PID_0268). ps3pad talks XInput
; to the device DsHidMini exposes, so no virtual gamepad or ViGEmBus is
; needed: Windows already shows a genuine XInput controller that games
; recognise natively.

#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif

#define MyAppName "PS3Pad"
#define MyAppPublisher "ps3pad contributors"
#define MyAppURL "https://github.com/jpenren/ps3pad"
#define MyAppExeName "ps3pad.exe"
#define MyAppId "{FEAF2AC6-B977-4155-A725-55EEFAB2A430}"

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
PrivilegesRequired=admin
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

; DsHidMini driver package (BSD-3-Clause, nssmkig binary redistribution
; permitted). Installed elevated by install-driver.ps1 on first run.
Source: "drivers\dshidmini\dshidmini.inf"; DestDir: "{app}\drivers\dshidmini"; Flags: ignoreversion
Source: "drivers\dshidmini\dshidmini.cat"; DestDir: "{app}\drivers\dshidmini"; Flags: ignoreversion
Source: "drivers\dshidmini\x64\dshidmini.dll"; DestDir: "{app}\drivers\dshidmini\x64"; Flags: ignoreversion
Source: "drivers\dshidmini\nssmkig_x64\igfilter.inf"; DestDir: "{app}\drivers\dshidmini\nssmkig_x64"; Flags: ignoreversion
Source: "drivers\dshidmini\nssmkig_x64\igfilter.cat"; DestDir: "{app}\drivers\dshidmini\nssmkig_x64"; Flags: ignoreversion
Source: "drivers\dshidmini\nssmkig_x64\nssmkig.sys"; DestDir: "{app}\drivers\dshidmini\nssmkig_x64"; Flags: ignoreversion
Source: "drivers\dshidmini\nssmkig_x64\LICENSE"; DestDir: "{app}\drivers\dshidmini\nssmkig_x64"; Flags: ignoreversion
Source: "drivers\dshidmini\nssmkig_ARM64_LICENSE"; DestDir: "{app}\drivers\dshidmini"; Flags: ignoreversion

; Driver install/uninstall helper scripts.
Source: "install-driver.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "uninstall-driver.ps1"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Parameters: "gui"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Parameters: "gui"; Tasks: desktopicon

[Run]
; Install the DsHidMini driver package elevated so the connected DS3 is
; bound to DsHidMini and exposed as a native XInput gamepad.
Filename: "{app}\install-driver.ps1"; Parameters: "-DriverDir ""{app}\drivers\dshidmini"""; WorkingDir: "{app}"; Flags: runas waituntilidle runhidden; StatusMsg: "Installing DsHidMini driver for DualShock 3..."

[UninstallRun]
; Remove the DsHidMini driver packages from the driver store.
Filename: "{app}\uninstall-driver.ps1"; WorkingDir: "{app}"; Flags: runas waituntilidle runhidden; StatusMsg: "Removing DsHidMini driver packages..."
