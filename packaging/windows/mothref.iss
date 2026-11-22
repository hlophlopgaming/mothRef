; SPDX-License-Identifier: GPL-3.0-or-later
; Compile from the repository root after staging package/.
[Setup]
AppId=org.mothref.MothRef
AppName=MothRef
AppVersion={#AppVersion}
AppPublisher=MothRef contributors
AppPublisherURL=https://github.com/hlophlopgaming/mothRef
DefaultDirName={localappdata}\Programs\MothRef
DefaultGroupName=MothRef
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\mothref.exe
SetupIconFile=..\..\assets\icons\moth.ico
LicenseFile=..\..\LICENSE
OutputDir=..\..\dist
OutputBaseFilename=mothref-windows-x86_64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ChangesAssociations=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; Flags: unchecked
Name: "associate"; Description: "{cm:AssocFileExtension,MothRef,.moth}"; Flags: unchecked

[Files]
Source: "..\..\package\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{group}\MothRef"; Filename: "{app}\mothref.exe"
Name: "{autodesktop}\MothRef"; Filename: "{app}\mothref.exe"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Classes\.moth\OpenWithProgids"; ValueType: string; ValueName: "MothRef.Board"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate
Root: HKCU; Subkey: "Software\Classes\MothRef.Board"; ValueType: string; ValueData: "MothRef board"; Flags: uninsdeletekey; Tasks: associate
Root: HKCU; Subkey: "Software\Classes\MothRef.Board\DefaultIcon"; ValueType: string; ValueData: "{app}\mothref.exe,0"; Tasks: associate
Root: HKCU; Subkey: "Software\Classes\MothRef.Board\shell\open\command"; ValueType: string; ValueData: """{app}\mothref.exe"" ""%1"""; Tasks: associate

[Run]
Filename: "{app}\mothref.exe"; Description: "{cm:LaunchProgram,MothRef}"; Flags: nowait postinstall skipifsilent
