param([string]$In, [string]$Out, [int]$W = 560)
Add-Type -AssemblyName System.Drawing
$i = [System.Drawing.Image]::FromFile($In)
$h = [int]($i.Height * $W / $i.Width)
$b = New-Object System.Drawing.Bitmap $W, $h
$g = [System.Drawing.Graphics]::FromImage($b)
$g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$g.DrawImage($i, 0, 0, $W, $h)
$b.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
