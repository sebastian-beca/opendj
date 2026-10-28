; Inno Setup script for openDJ's Windows setup. scripts/bundle-windows.ps1
; runs it with /DVersion=... /DSource=<folder with openDJ.exe> /DOutput=dist.
; Installs for the current user only, so it needs no administrator.

#ifndef Version
  #define Version "0.0.0"
#endif

[Setup]
; Never change the AppId: it is how Windows knows a new setup updates the
; installed openDJ instead of adding a second one.
AppId={{59D890AB-0739-4D1A-987B-2E441345DC98}
AppName=openDJ
AppVersion={#Version}
AppVerName=openDJ {#Version}
AppPublisher=openDJ
AppPublisherURL=https://opendj.app
AppSupportURL=https://github.com/sebastian-beca/opendj/issues
AppUpdatesURL=https://github.com/sebastian-beca/opendj/releases
DefaultDirName={autopf}\openDJ
DefaultGroupName=openDJ
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
LicenseFile={#Source}\LICENSE-GPL3.txt
SetupIconFile=opendj.ico
UninstallDisplayIcon={app}\openDJ.exe
OutputDir={#Output}
OutputBaseFilename=openDJ-{#Version}-windows-x64-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#Source}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[InstallDelete]
; Mappings removed from a newer version shouldn't linger.
Type: filesandordirs; Name: "{app}\controllers"

[Icons]
Name: "{autoprograms}\openDJ"; Filename: "{app}\openDJ.exe"
Name: "{autodesktop}\openDJ"; Filename: "{app}\openDJ.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\openDJ.exe"; Description: "{cm:LaunchProgram,openDJ}"; Flags: nowait postinstall skipifsilent
; After an in-app update, which runs this setup silently with /RELAUNCH.
Filename: "{app}\openDJ.exe"; Flags: nowait; Check: IsRelaunch

[Code]
function IsRelaunch: Boolean;
begin
  Result := Pos('/RELAUNCH', UpperCase(GetCmdTail)) > 0;
end;
