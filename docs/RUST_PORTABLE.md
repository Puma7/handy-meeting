# Rust portable packages

The Rust/Tauri desktop now has target-specific capture adapters and package builders for
Linux x86-64 and macOS Apple Silicon/Intel. Version 1.0.0 packaging is being prepared. A source
implementation, cross-target compilation, a successful CI package build, and an actual
device recording are different evidence. See `TEST_REPORT.md` for completed checks.

## Supported package targets

| Target | Build baseline | Intended runtime | Package |
| --- | --- | --- | --- |
| Linux x86-64 | Ubuntu22.04, Rust1.91.1, WebKitGTK4.1 | Ubuntu22.04/24.04; Debian12/13 | `HandyMeeting-1.0.0-linux-amd64.deb` |
| Apple Silicon | macOS runner, Rust1.91.1, deployment target13.0 | macOS13 or newer, M-series | `HandyMeeting-1.0.0-osx-arm64.dmg` |
| Intel Mac | macOS runner, Rust1.91.1, deployment target13.0 | macOS13 or newer, Intel x86-64 | `HandyMeeting-1.0.0-osx-x64.dmg` |

These are intended targets, subject to each build and hardware test. Linux ARM, ALSA-only
systems without a PulseAudio-compatible service, macOS12 and universal Mac binaries are
not provided. A Wayland/X11 desktop and the listed GTK/WebKit dependencies
are required for the Linux GUI. The release package is named `handymeeting`, with program files in
`/opt/handymeeting/<version>`; recordings remain outside the package. Earlier Rust beta
packages retain their separate `handymeeting-rust-beta` name and directories. Installing
the release does not delete their data. The application bundle ID and existing RustBeta
data location remain stable to preserve user settings and permission continuity.

## Capture and timing

Linux reuses `pactl` and `parec` from `pulseaudio-utils`, with PulseAudio or PipeWire-Pulse.
The system track uses the server's actual output monitor source, rather than guessing
a `.monitor` suffix. Microphones are separate selectable sources. No audio server is
started, stopped, or reconfigured. A missing utility or unavailable service produces an
actionable error. Device query output and process deadlines are bounded.

macOS reuses `platform/macos/HandyMeetingCapture.swift`: ScreenCaptureKit captures system
audio and AVAudioEngine captures the microphone. Only audio stream outputs are registered;
screen frames are not retained. The helper acknowledges startup after permission and
capture startup. This beta offers the default microphone and system output only. It
needs the system's Screen & System Audio Recording/Screen Recording permission, and the
microphone permission when that track is requested. Exact settings names vary by macOS.
An ad-hoc CI signature is not a Developer ID signature or notarization; unsigned candidate
distribution still needs an explicit trust/distribution plan before a public release.

Each source has its own child process and disk writer. A bounded two-packet queue carries
16kHz mono16-bit PCM. Stop/cleanup targets only the child started for that track. Received
PCM is drained and the writer is finalized after a helper exits or fails. macOS helpers
also exit on parent-pipe EOF. Linux `parec` stops when the parent's pipe closes; it has no
independent file or network destination. ASR workers remain separate from capture.

These helpers return raw PCM without common device timestamps. Track alignment uses
approximate receipt timing. Leading gaps and pauses without packets can be represented
by estimated silence; late real packets remain retained and timing can drift. A warning
states this limitation. Such silence does not prove the source was silent. This beta
does not claim sample-accurate alignment, complete capture across sleep/device changes,
or reliable two-hour operation on untested target hardware. Actual device gap detection,
stop tails, drift and permission handling require target-specific tests.

## Native runtime and packages

`scripts/prepare-rust-portable.py` consumes `scripts/native-assets.json`. The three official
transcribe.cpp0.4.1 archives have fixed SHA-256 values. Only expected regular native
libraries, the ABI contract and licenses are materialized. Existing different SDK files
are rejected. The separate `.build/rust-native-beta2-0.4.1-<rid>` namespace preserves the
Windows SDK and earlier distributions. No recognition model is downloaded or bundled.

The prebuilt SDK uses the maintained `transcribe-cpp` safe Rust bindings. No local FFI or
recognition algorithm is introduced. Native modules sit beside the executable in `native/`.
Linux uses `$ORIGIN/native`, macOS `@executable_path/native` as the executable's loader path;
upstream dylibs already refer to their siblings via `@loader_path`.

The SDK's actual Linux requirements are checked with `readelf --version-info` on the Linux
build host. All18 official libraries were also inspected from their verified archive on
Windows: the highest observed tags were GLIBC2.27, GLIBCXX3.4.22 and CXXABI1.3.9;
all version-requirement records were confirmed with LLVM. All nine Mac dylibs declare
minimum macOS11.0 in their actual build commands; capture raises the app's floor to13.0.
Ubuntu22 builds
must still pass real Debian12/13 package installation and headless release checks. The
Mac SDK's minimum build versions are checked with `otool -l` against macOS13.0.

`scripts/rust-portable-artifacts.py` performs filesystem/hash bookkeeping only. It checks
source receipts, current lockfile license manifests, all native library hashes and the
exact executable's model-free release-check result. Staging uses a fresh private directory
and explicit payload lists. `package-rust-portable.sh` builds the native Mac helper, seals
the complete app with an ad-hoc signature, verifies it, and re-runs the relocated binary's
checks without SDK loader environment variables. Final hashes are recorded after signing;
the manifest stays outside the sealed app to avoid circular signature/hash changes.
DMGs/DEBs and manifests have accompanying package hashes and existing candidates are never
overwritten. Models, session histories, settings, API keys and private recordings are not
package inputs. No installer hooks or automatic cloud calls are included.

After final DEB/DMG creation and verification, `bind-package` writes the exact archive
filename, SHA-256 and byte length into its adjacent portable manifest. The earlier sealed
payload proof remains unchanged. Release assembly requires that binding as well as the
matching source and native/relocated checks. Replacing an archive and recomputing only its
checksum sidecar cannot reuse a different package's manifest.

## CI and required target tests

`.github/workflows/rust-portable.yml` builds the actual Tauri GUI for all three targets,
runs locked workspace tests and warning-rejecting Clippy, executes synthetic bounded-pipe
stop/crash/reopen checks, verifies native runtime linkage, and creates candidate packages.
The Linux job also installs the DEB in Debian Bookworm and Trixie containers and runs its
headless release check. This tests installation and dynamic linkage, not desktop rendering
or physical device access. The workflow uploads artifacts; it does not publish a release.

Before calling a target ready, record an actual simultaneous system/microphone session on
that target; verify speech separation, permission denial, default device changes, pause,
sleep/resume, stop tails, helper/ASR/parent crashes, confirmed hashes after reopening, and
a full two-hour capture with separate host/ASR memory measurements. Local model inference
must use a compatible streaming model already installed by Handy. No such target hardware
or inference result follows merely from successful package construction.
