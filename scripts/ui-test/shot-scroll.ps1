param([string]$Exe, [string]$Out, [string]$ArgList = "", [int]$Wheel = -30, [int]$Wait = 3)
# Launch the app, scroll the mouse wheel inside the window, then capture it (to check parts below the fold).
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class W2 {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, int d, int e);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
"@
[W2]::SetProcessDPIAware() | Out-Null
if ($ArgList -ne "") { $p = Start-Process -FilePath $Exe -ArgumentList $ArgList -PassThru } else { $p = Start-Process -FilePath $Exe -PassThru }
Start-Sleep -Seconds $Wait
$p.Refresh()
$h = $p.MainWindowHandle
[W2]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Milliseconds 600
$r = New-Object W2+RECT
[W2]::GetWindowRect($h, [ref]$r) | Out-Null
[W2]::SetCursorPos([int](($r.L + $r.R) / 2), [int](($r.T + $r.B) / 2)) | Out-Null
for ($i = 0; $i -lt [Math]::Abs($Wheel); $i++) {
  [W2]::mouse_event(0x0800, 0, 0, $(if ($Wheel -lt 0) { -120 } else { 120 }), 0)
  Start-Sleep -Milliseconds 30
}
Start-Sleep -Milliseconds 500
$w = $r.R - $r.L; $hh = $r.B - $r.T
$bmp = New-Object System.Drawing.Bitmap $w, $hh
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.L, $r.T, 0, 0, (New-Object System.Drawing.Size $w, $hh))
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Stop-Process -Id $p.Id -Force
Write-Output "saved $Out ($w x $hh)"
