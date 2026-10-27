#!/usr/bin/env bash
# Target-host packaging only. Never updates an installed or previously staged app.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
rid="${1:?Use linux-x64, osx-arm64 or osx-x64}"
source_receipt="${2:?Pass the current source fingerprint JSON}"
release_checks="${3:?Pass the exact release executable check directory}"
case "$rid" in
  linux-x64) test "$(uname -s)" = Linux; target=x86_64-unknown-linux-gnu ;;
  osx-arm64) test "$(uname -s)" = Darwin; target=aarch64-apple-darwin; swift_target=arm64-apple-macos13.0 ;;
  osx-x64) test "$(uname -s)" = Darwin; target=x86_64-apple-darwin; swift_target=x86_64-apple-macos13.0 ;;
  *) echo 'Unsupported package target.' >&2; exit 2 ;;
esac
if [[ "$rid" = linux-x64 ]]; then
  work="$(python3 scripts/rust-portable-artifacts.py stage "$rid" "$source_receipt" "$release_checks")"
else
  tools="$(mktemp -d "$root/.build/rust-mac-tools.XXXXXX")"
  xcrun swiftc -parse-as-library -swift-version 5 -warnings-as-errors -O -target "$swift_target" platform/macos/HandyMeetingCapture.swift -o "$tools/HandyMeetingCapture" -framework AVFoundation -framework ScreenCaptureKit -framework CoreMedia -framework CoreAudio -framework AudioToolbox
  iconset="$tools/HandyMeeting.iconset"
  mkdir "$iconset"
  for size in 16 32 128 256 512; do
    sips -z "$size" "$size" assets/handymeeting.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
    doubled=$((size * 2))
    sips -z "$doubled" "$doubled" assets/handymeeting.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
  done
  iconutil -c icns "$iconset" -o "$tools/HandyMeeting.icns"
  work="$(python3 scripts/rust-portable-artifacts.py stage "$rid" "$source_receipt" "$release_checks" --helper "$tools/HandyMeetingCapture" --icon "$tools/HandyMeeting.icns")"
fi
app="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["app"])' "$work/stage-receipt.json")"
program="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["program"])' "$work/stage-receipt.json")"
version="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["applicationVersion"])' "$work/stage-receipt.json")"
if [[ "$rid" != linux-x64 ]]; then
  plutil -lint "$app/Contents/Info.plist"
  # CI candidates are deliberately ad-hoc signed; no Apple account or notarization claim.
  for library in "$program"/native/*.dylib; do codesign --force --sign - "$library"; done
  codesign --force --sign - "$program/HandyMeetingCapture"
  codesign --force --sign - "$program/HandyMeeting"
  codesign --force --sign - "$app"
  codesign --verify --deep --strict "$app"
fi
# Prove relocation resolves bundled native libraries without a build-host loader path.
env -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH -u TRANSCRIBE_DIR "$program/HandyMeeting" --release-checks "$work/relocated-checks"
if [[ "${GITHUB_ACTIONS:-}" = true ]]; then
  # Only the isolated GitHub-runner wrapper may change a disposable native store.
  # Check the relocated (on macOS already signed) payload, never a user's keychain.
  env -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH -u TRANSCRIBE_DIR bash scripts/check-native-credentials.sh "$program/HandyMeeting" .build/portable-credential-checks
fi
node scripts/rust-artifacts.mjs source-fingerprint > "$work/source-after-package.json"
python3 scripts/rust-portable-artifacts.py finalize "$work" "$work/source-after-package.json" "$work/relocated-checks"
mkdir -p packages
if [[ "$rid" = linux-x64 ]]; then
  stage="$work/deb"
  install_root="/opt/handymeeting/$version"
  mkdir -p "$stage$install_root" "$stage/usr/share/icons/hicolor/scalable/apps"
  cp -a "$app/." "$stage$install_root/"
  # Control file, desktop entry and the /usr/bin/handymeeting link come from one tested definition.
  python3 scripts/rust-portable-artifacts.py deb-metadata "$work"
  cp assets/handymeeting.svg "$stage/usr/share/icons/hicolor/scalable/apps/handymeeting.svg"
  find "$stage" -type d -exec chmod 755 {} +
  find "$stage" -type f -exec chmod 644 {} +
  chmod 755 "$stage$install_root/HandyMeeting"
  package="packages/HandyMeeting-$version-linux-amd64.deb"
  test ! -e "$package" || { echo 'Candidate already exists; refusing to overwrite it.' >&2; exit 2; }
  dpkg-deb --root-owner-group --build "$stage" "$package"
  dpkg-deb --info "$package"
  (cd packages && sha256sum "$(basename "$package")") > "$package.sha256"
else
  package="packages/HandyMeeting-$version-$rid.dmg"
  test ! -e "$package" || { echo 'Candidate already exists; refusing to overwrite it.' >&2; exit 2; }
  # Only the app is materialized in the image; no SDK, check-session or source directory.
  image_root="$work/image"
  mkdir "$image_root"
  cp -a "$app" "$image_root/"
  ln -s /Applications "$image_root/Applications"
  hdiutil create -volname 'HandyMeeting' -srcfolder "$image_root" -format UDZO "$package"
  hdiutil verify "$package"
  (cd packages && shasum -a 256 "$(basename "$package")") > "$package.sha256"
fi
python3 scripts/rust-portable-artifacts.py bind-package "$work" "$package"
# Only explicit regular JSON files enter the flat upload directory. The image's
# Applications link must never be searched by a recursive artifact glob.
python3 scripts/rust-portable-artifacts.py collect-proofs "$rid" "$work"
echo 'Candidate built. No real-device, long-session or notarization result is implied.'
