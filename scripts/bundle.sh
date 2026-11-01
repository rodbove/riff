#!/bin/sh
set -e
cd "$(dirname "$0")/.."
cargo build --release -p riff-app
APP=target/riff.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp target/release/riff "$APP/Contents/MacOS/riff"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>riff</string>
  <key>CFBundleDisplayName</key><string>riff</string>
  <key>CFBundleIdentifier</key><string>com.rodbove.riff</string>
  <key>CFBundleExecutable</key><string>riff</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>riff plays your guitar from the audio interface input through amps and effects.</string>
</dict>
</plist>
PLIST
codesign --force --deep --sign - "$APP"
echo "built $APP"
