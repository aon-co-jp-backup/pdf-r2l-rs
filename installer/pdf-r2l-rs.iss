[Setup]
AppName=PDF R2L RS
AppVersion=1.0.3
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
; 上書きインストール時に、起動中のアプリを閉じてから置き換える。
CloseApplications=yes
RestartApplications=no

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

[UninstallRun]
; 起動中のアプリがあるとファイルを削除できないので、先に終了させる。
Filename: "{sys}\taskkill.exe"; Parameters: "/F /T /IM pdf-r2l-rs.exe"; Flags: runhidden skipifdoesntexist; RunOnceId: "KillGui"
Filename: "{sys}\taskkill.exe"; Parameters: "/F /T /IM pdf-r2l-cli.exe"; Flags: runhidden skipifdoesntexist; RunOnceId: "KillCli"

[UninstallDelete]
; 実行中にできたファイルなど、インストーラーが入れていないものも残さない。
Type: filesandordirs; Name: "{app}"

[Code]
// 通常のアンインストール(画面あり)では、設定とキャッシュ(取得したフォント)も消すか尋ねる。
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if (CurUninstallStep = usPostUninstall) and (not UninstallSilent) then
  begin
    if MsgBox('設定と、取得したフォントなどのキャッシュも削除しますか?' + #13#10 +
              'Also delete the settings and the cache (downloaded fonts)?',
              mbConfirmation, MB_YESNO) = IDYES then
    begin
      DelTree(ExpandConstant('{userappdata}\pdf-r2l-rs'), True, True, True);
      DelTree(ExpandConstant('{localappdata}\pdf-r2l-rs'), True, True, True);
    end;
  end;
end;
