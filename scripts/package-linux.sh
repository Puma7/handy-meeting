#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
app="$root/.build/portable/linux-x64"
stage="$(mktemp -d "${TMPDIR:-/tmp}/handymeeting-deb.XXXXXX")"
out="$root/packages"
test -f "$app/HandyMeeting"
test -f "$app/native/libtranscribe.so"
mkdir -p "$stage/DEBIAN" "$stage/opt/handymeeting" "$stage/usr/share/applications" "$stage/usr/share/icons/hicolor/scalable/apps" "$out"
cp -a "$app/." "$stage/opt/handymeeting/"
find "$stage" -type d -exec chmod 755 {} +
find "$stage/opt/handymeeting" -type f -exec chmod 644 {} +
chmod 755 "$stage/opt/handymeeting/HandyMeeting"
cat > "$stage/DEBIAN/control" <<'EOF'
Package: handymeeting
Version: 0.5.0~preview1
Section: sound
Priority: optional
Architecture: amd64
Maintainer: HandyMeeting contributors
Depends: libc6 (>= 2.35), libgcc-s1, libstdc++6, libgomp1, libicu70 | libicu72 | libicu74 | libicu76 | libicu78, libfontconfig1, libx11-6, libxrender1, libxrandr2, libxi6, libxcursor1, libice6, libsm6, libglib2.0-0 | libglib2.0-0t64, libgl1, pulseaudio-utils
Suggests: pipewire-pulse | pulseaudio
Description: Local meeting capture and grounded notes, portable preview
 Captures system audio and optional microphone separately using the local
 PulseAudio-compatible service. Reuses a compatible model installed by Handy.
 Preview: device capture and recovery still require tests on target hardware.
EOF
cat > "$stage/usr/share/applications/handymeeting.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=HandyMeeting Preview
Comment=Local meeting audio and transcript
Exec=/opt/handymeeting/HandyMeeting
Icon=handymeeting
Terminal=false
Categories=Audio;AudioVideo;Utility;
EOF
cp "$root/assets/handymeeting.svg" "$stage/usr/share/icons/hicolor/scalable/apps/handymeeting.svg"
dpkg-deb --root-owner-group --build "$stage" "$out/HandyMeeting-0.5.0-preview.1-linux-amd64.deb"
dpkg-deb --info "$out/HandyMeeting-0.5.0-preview.1-linux-amd64.deb"
(cd "$out" && sha256sum 'HandyMeeting-0.5.0-preview.1-linux-amd64.deb') > "$out/HandyMeeting-0.5.0-preview.1-linux-amd64.deb.sha256"
echo 'Built a preview package. This does not establish real-device compatibility.'
