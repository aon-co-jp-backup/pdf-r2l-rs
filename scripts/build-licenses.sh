#!/bin/sh
# LICENSES.txt (英日併記, UTF-8 BOM付き) を組み立てる。依存を変えたら先に
#   powershell -File scripts/gen-third-party.ps1 > licenses/third-party.txt
# を実行する。
cd "$(dirname "$0")/.." || exit 1
{ printf '\357\273\277'; cat licenses/header.txt licenses/apache-2.0.txt licenses/third-party-header.txt licenses/third-party.txt; } > LICENSES.txt
mkdir -p android/app/src/main/assets && cp LICENSES.txt android/app/src/main/assets/LICENSES.txt
