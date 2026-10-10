#!/bin/sh
# Android アプリの画面を、複数の解像度・向き(スマホ/タブレット、縦/横)で撮影する。
# 使い方: sh android-matrix.sh <パッケージ名> <アクティビティ(例 .MainActivity)> [出力フォルダ]
# 前提: エミュレータ(または実機)が adb で見えていて、アプリがインストール済み。実行後は元の解像度・向きに戻す。
# 確認する点は ../../docs/UI-TEST-CHECKLIST.md を参照。撮影画像は縮小して保存する(確認用)。
export MSYS_NO_PATHCONV=1
PKG=$1
ACT=${2:-.MainActivity}
OUT=${3:-F:/_gen/matrix}
ADB=${ADB:-/c/Users/noruk/AppData/Local/Android/Sdk/platform-tools/adb.exe}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT"

shot() { # 名前 幅(縮小後)
  $ADB shell am force-stop "$PKG"
  $ADB shell am start -n "$PKG/$ACT" >/dev/null
  sleep 8   # エミュレータは起動直後の描画が遅い(スプラッシュだけ写るのを避ける)
  $ADB exec-out screencap -p > "$OUT/$1.png"
  powershell -NoProfile -File "$(cygpath -w "$HERE/shrink.ps1")" -In "$(cygpath -w "$OUT/$1.png")" -Out "$(cygpath -w "$OUT/$1-s.png")" -W "$2" >/dev/null
}

$ADB shell settings put system accelerometer_rotation 0
$ADB shell settings put system user_rotation 0
# 小さいスマホ(縦)・標準スマホ(縦)・スマホ(横)・10インチタブレット(縦/横)
$ADB shell wm size 720x1280;  $ADB shell wm density 320; shot phone-small-portrait 380
$ADB shell wm size 1080x2400; $ADB shell wm density 420; shot phone-portrait 380
$ADB shell settings put system user_rotation 1;          shot phone-landscape 700
$ADB shell settings put system user_rotation 0
$ADB shell wm size 1600x2560; $ADB shell wm density 320; shot tablet-portrait 420
$ADB shell wm size 2560x1600;                            shot tablet-landscape 700
# 文字を大きくした設定(アクセシビリティ)でも崩れないか
$ADB shell wm size 1080x2400; $ADB shell wm density 420
$ADB shell settings put system font_scale 1.5;           shot phone-bigfont 380
$ADB shell settings put system font_scale 1.0
# 元に戻す
$ADB shell wm size reset; $ADB shell wm density reset
$ADB shell settings put system user_rotation 0
echo "撮影した画像: $OUT/*-s.png"
