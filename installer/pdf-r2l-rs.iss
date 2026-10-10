[Setup]
AppName=PDF R2L RS
AppVersion=1.0.2
AppPublisher=aon-co-jp
DefaultDirName={autopf}\PDF R2L RS
DefaultGroupName=PDF R2L RS
OutputDir=..\dist
OutputBaseFilename=pdf-r2l-rs-setup
Compression=lzma2
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile=..\LICENSES.txt
UninstallDisplayIcon={app}\pdf-r2l-rs.exe

[Languages]
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop icon / デスクトップにアイコンを作成"

[Files]
Source: "..\target\release\pdf-r2l-rs.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\pdf-r2l-cli.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSES.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\PDF R2L RS"; Filename: "{app}\pdf-r2l-rs.exe"
Name: "{autodesktop}\PDF R2L RS"; Filename: "{app}\pdf-r2l-rs.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\pdf-r2l-rs.exe"; Description: "Launch PDF R2L RS / 起動"; Flags: nowait postinstall skipifsilent
