#!/bin/sh
# universalバイナリ -> .app -> .dmg(未署名だが ad-hoc 署名は付ける)
set -e
cd "$(dirname "$0")/.."
VER=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
APP="dist/PDF R2L RS.app"
rm -rf dist && mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/pdf-r2l-rs" \
  target/aarch64-apple-darwin/release/pdf-r2l-rs target/x86_64-apple-darwin/release/pdf-r2l-rs
sed "s/@VERSION@/$VER/g" packaging/Info.plist > "$APP/Contents/Info.plist"
cp LICENSES.txt "$APP/Contents/Resources/"
codesign --force --deep -s - "$APP"
STAGE=dist/stage && mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
cp LICENSES.txt "$STAGE/LICENSES.txt"
hdiutil create -volname "PDF R2L RS" -srcfolder "$STAGE" -ov -format UDZO "dist/pdf-r2l-rs-$VER.dmg"
rm -rf "$STAGE"
ls -l dist
