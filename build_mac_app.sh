#!/bin/bash
set -e

APP_NAME="Thock"
APP_DIR="${APP_NAME}.app"
CONTENTS_DIR="${APP_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"

echo "Terminating any running instances of Thock..."
killall Thock 2>/dev/null || true
sleep 1

echo "Building Rust binary..."
source $HOME/.cargo/env
cargo build --release

echo "Creating .app directory structure..."
rm -rf "${APP_DIR}"
mkdir -p "${MACOS_DIR}"
mkdir -p "${RESOURCES_DIR}"

echo "Copying binary and sound packs..."
cp target/release/thock "${MACOS_DIR}/Thock"
cp -r packs "${RESOURCES_DIR}/packs"
cp AppIcon.icns "${RESOURCES_DIR}/AppIcon.icns"

echo "Generating Info.plist..."
cat <<PLIST > "${CONTENTS_DIR}/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>Thock</string>
    <key>CFBundleIdentifier</key>
    <string>com.dharmitparmar.thock</string>
    <key>CFBundleName</key>
    <string>Thock</string>
    <key>CFBundleVersion</key>
    <string>1.0.0</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.0</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST

echo "Done! The macOS app bundle is ready."
