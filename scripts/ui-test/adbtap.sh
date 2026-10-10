#!/bin/sh
# usage: adbtap.sh "text-substring"   画面の文字(部分一致)を探してタップする
export MSYS_NO_PATHCONV=1
A=/c/Users/noruk/AppData/Local/Android/Sdk/platform-tools/adb.exe
$A shell uiautomator dump /sdcard/ui.xml >/dev/null 2>&1
B=$($A shell cat /sdcard/ui.xml | sed 's/<node/\n<node/g' | grep -F "$1" | head -1 | sed -E 's/.*bounds="\[([0-9]+),([0-9]+)\]\[([0-9]+),([0-9]+)\]".*/\1 \2 \3 \4/')
[ -z "$B" ] && { echo "not found: $1"; exit 1; }
set -- $B
$A shell input tap $(( ($1+$3)/2 )) $(( ($2+$4)/2 ))
echo "tapped $1"
