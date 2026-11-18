; Cimori's installer (Inno Setup 6): per user, no administrator, a fixed folder the AI hooks can count on.
; Build: iscc /DVersion=1.0.0 installer\cimori.iss   (scripts\release.ps1 and the release workflow do it)
#ifndef Version
  #define Version "0.0.0"
#endif

[Setup]
AppId={{FEED6FC7-3219-4F85-B40B-AC2E4B8AA025}
AppName=Cimori
AppVersion={#Version}
AppVerName=Cimori {#Version}
AppPublisher=Marcus Paixão
AppPublisherURL=https://github.com/Vini-Paixao/Cimori
AppSupportURL=https://github.com/Vini-Paixao/Cimori/issues
AppUpdatesURL=https://github.com/Vini-Paixao/Cimori/releases
DefaultDirName={localappdata}\Programs\Cimori
DisableDirPage=yes
DisableProgramGroupPage=yes
DefaultGroupName=Cimori
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.19041
LicenseFile=..\LICENSE
SetupIconFile=..\assets\brand\cimori\windows\cimori.ico
UninstallDisplayIcon={app}\Cimori.exe
UninstallDisplayName=Cimori
OutputDir=..\dist
OutputBaseFilename=Cimori-{#Version}-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=no

[Languages]
Name: "ptbr"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\release\Cimori.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\windhawk\cimori-tray-bridge.wh.cpp"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Cimori"; Filename: "{app}\Cimori.exe"
Name: "{userdesktop}\Cimori"; Filename: "{app}\Cimori.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\Cimori.exe"; Description: "{cm:LaunchProgram,Cimori}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; The AI hooks would run a Cimori that is gone: taken out of Claude Code and Codex, the user's own kept.
Filename: "{app}\Cimori.exe"; Parameters: "--uninstall-hooks"; Flags: runhidden waituntilterminated; RunOnceId: "UninstallHooks"

[Registry]
; "Iniciar com o Windows" is the app's own setting (it writes this value); only removed with the app.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "Cimori"; Flags: uninsdeletevalue dontcreatekey

[Code]
// A running Cimori holds its exe: closed before files are replaced or removed. Settings in
// %APPDATA%\Cimori stay, so an update or a reinstall keeps them.
procedure CloseCimori();
var
  Code: Integer;
begin
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM Cimori.exe', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Sleep(400);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  CloseCimori();
  Result := '';
end;

function InitializeUninstall(): Boolean;
begin
  CloseCimori();
  Result := True;
end;
