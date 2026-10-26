## HandyMeeting 1.0.1

A patch release of the independent Rust/Tauri meeting companion for Handy.

### Fixed in 1.0.1

- Windows setup now creates a fresh default destination when its parent folders do not yet exist. Existing program directories and user files remain protected. Native tests and actual CI installation cover this first-install layout.
- Unfinished analysis work is discoverable after restarting the app, preserving original instructions and requiring fresh cloud consent. Backup-recoverable sessions remain visible in history.
- Portable package proofs bind the final archive name, bytes and SHA-256.
- Template UI tests use independent cases rather than one long combined workflow; no timeout increase or test retry is used.

### Choose your download

- **Windows 10/11 x64:** `windows-x64-setup.exe`, or extract the complete `windows-x64.zip`. WebView2 and Microsoft Visual C++ x64 are required; .NET is not needed.
- **Ubuntu 22.04+ / Debian 12–13 x64:** install `linux-amd64.deb` with your package manager. PulseAudio or PipeWire-Pulse is required.
- **Mac M-series / Apple Silicon:** `osx-arm64.dmg`, macOS 13+.
- **Mac Intel:** `osx-x64.dmg`, macOS 13+.

Install Handy and a compatible downloaded streaming model first. The companion detects its model metadata and reuses the existing weights in its own local inference processes. For local summaries, additionally run a local text-model service such as LM Studio and choose a text model in the app.

### Included

- Separate system and optional microphone recording; recognition runs independently of audio saving.
- Local session history, original transcript snapshots, complete TXT export and direct session-folder access.
- Named analysis templates with editable processing tabs, original quotes and source navigation.
- Local-first notes, explicit per-run cloud text consent and optional native OS credential storage.
- Confirmed-step progress, elapsed time, cancellation, retained/resumable analysis work, German/English and light/dark themes.
- Private local agent control with visible recording consent and exact session ownership; no MCP claim.

### Read before relying on a recording

Mac/Linux device capture and a new full-app Rust two-hour run still need real-device validation. Earlier synthetic, C# and worker-only tests do not establish that result. Driver interruption warnings remain meaningful; confirmed checkpoint recovery is not a power-loss guarantee and an unconfirmed audio tail can be missing. Diarization and exact word timestamps are not implemented. Meeting notes remain reviewable AI drafts.

Mac packages are **ad-hoc signed, not Apple-notarized**. Windows setup is **unsigned**. System trust checks can block these builds. Do not disable system protection to install them. Use short independently backed-up test sessions first.

The retained dependency audit reports nine maintenance warnings and one existing GLib unsoundness warning. No dependency versions were silently updated for packaging; see the test report for the audit scope and limitations.

SHA-256 sidecars, `SHA256SUMS.txt`, package proofs and `RELEASE_MANIFEST.json` accompany the complete matrix. The manifest records the exact source commit and fingerprint; it explicitly distinguishes automated checks from unperformed real-device/notarization checks. No models, recordings, settings or keys are included. Existing user data and earlier releases are preserved.

[English guide](https://github.com/Puma7/handy-meeting/blob/main/README.md) · [Deutsche Anleitung](https://github.com/Puma7/handy-meeting/blob/main/README.de.md) · [Test evidence](https://github.com/Puma7/handy-meeting/blob/main/TEST_REPORT.md) · [Open issues](https://github.com/Puma7/handy-meeting/issues)

HandyMeeting is independently developed and is not an official Handy product or an endorsed integration. MIT licensed; original dependency and installer notices are included.
