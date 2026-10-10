# Windows インストーラーの無人試験: インストール → アプリ起動(使用中の状態) → アンインストール → 残りが無いこと
# 使い方: powershell -File install-uninstall-test.ps1 -Setup <setup.exe> -ExeName <アプリ.exe> [-Dir F:\_gen\instdir]
param([string]$Setup, [string]$ExeName, [string]$Dir = "F:\_gen\instdir")
if (Test-Path $Dir) { Remove-Item -Recurse -Force $Dir }
Start-Process -Wait -FilePath $Setup -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/DIR=$Dir"
if (-not (Test-Path (Join-Path $Dir $ExeName))) { Write-Output "FAIL: installed file not found"; exit 1 }
Start-Process (Join-Path $Dir $ExeName)      # 使用中の状態でアンインストールできるか(ファイルのロック)
Start-Sleep -Seconds 3
Start-Process -Wait -FilePath (Join-Path $Dir "unins000.exe") -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"
Start-Sleep -Seconds 5
$left = if (Test-Path $Dir) { (Get-ChildItem $Dir -Recurse -Force | Measure-Object).Count } else { 0 }
$proc = (Get-Process -Name ([IO.Path]::GetFileNameWithoutExtension($ExeName)) -ErrorAction SilentlyContinue | Measure-Object).Count
Write-Output "left files: $left, running processes: $proc"
if ($left -eq 0 -and $proc -eq 0) { Write-Output "OK" } else { Write-Output "FAIL"; exit 1 }
