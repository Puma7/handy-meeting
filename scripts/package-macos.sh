#!/usr/bin/env bash
set -euo pipefail
test "$(uname -s)" = Darwin || { echo 'A real macOS build host with Xcode command-line tools is required.' >&2; exit 2; }
root="$(cd "$(dirname "$0")/.." && pwd)"
arch="${1:-arm64}"
case "$arch" in arm64) rid=osx-arm64; target=arm64-apple-macos13.0;; x64) rid=osx-x64; target=x86_64-apple-macos13.0;; *) exit 2;; esac
export DOTNET_CLI_HOME="$root/.build/dotnet-home"
app="$root/.build/mac-$rid/HandyMeeting.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$root/packages"
dotnet restore "$root/src/HandyMeeting.Portable/HandyMeeting.Portable.csproj" --locked-mode
dotnet publish "$root/src/HandyMeeting.Portable/HandyMeeting.Portable.csproj" -c Release -r "$rid" --self-contained true --no-restore -o "$app/Contents/MacOS"
python3 "$root/scripts/native-runtime.py" "$rid" "$app/Contents/MacOS/native"
xcrun swiftc -parse-as-library -swift-version 5 -warnings-as-errors -O -target "$target" "$root/platform/macos/HandyMeetingCapture.swift" -o "$app/Contents/MacOS/HandyMeetingCapture" -framework AVFoundation -framework ScreenCaptureKit -framework CoreMedia
cp "$root/THIRD_PARTY_NOTICES.txt" "$app/Contents/Resources/"
cp "$root/LICENSE" "$app/Contents/Resources/HandyMeeting-LICENSE.txt"
cp -R "$root/licenses" "$app/Contents/Resources/third-party-licenses"
cat > "$app/Contents/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>org.handymeeting.preview</string><key>CFBundleName</key><string>HandyMeeting Preview</string><key>CFBundleExecutable</key><string>HandyMeeting</string><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleShortVersionString</key><string>0.5.0</string><key>CFBundleVersion</key><string>1</string><key>LSMinimumSystemVersion</key><string>13.0</string><key>NSHighResolutionCapable</key><true/>
<key>NSMicrophoneUsageDescription</key><string>Record your microphone as a separate local meeting track.</string>
<key>NSAudioCaptureUsageDescription</key><string>Record system meeting audio locally. Audio is not sent to a cloud service.</string>
</dict></plist>
EOF
plutil -lint "$app/Contents/Info.plist"
# Test artifacts use ad-hoc signing. Public distribution additionally needs a Developer ID and notarization.
identity="${HANDYMEETING_SIGNING_IDENTITY:--}"
# This .NET bundle layout places managed assemblies and runtime data in MacOS too.
# Seal every regular payload file before the helpers/apphost and the bundle itself.
# Signing only dylibs left managed DLLs unsigned and caused bundle verification to fail.
while IFS= read -r -d '' binary; do
  codesign --force --sign "$identity" "$binary"
done < <(find "$app/Contents/MacOS" -type f ! -name 'HandyMeetingCapture' ! -name 'HandyMeeting' -print0)
codesign --force --sign "$identity" "$app/Contents/MacOS/HandyMeetingCapture"
codesign --force --sign "$identity" "$app/Contents/MacOS/HandyMeeting"
codesign --force --sign "$identity" "$app"
codesign --verify --deep --strict "$app"
# Explicit regression check for the managed assembly that failed in the first CI run.
codesign --verify --strict "$app/Contents/MacOS/System.Diagnostics.Contracts.dll"
image="$root/packages/HandyMeeting-0.5.0-preview.1-$rid.dmg"
hdiutil create -volname 'HandyMeeting Preview' -srcfolder "$(dirname "$app")" -ov -format UDZO "$image"
hdiutil verify "$image"
shasum -a 256 "$image" > "$image.sha256"
echo 'Preview DMG built. No real-device test or notarization is implied.'
