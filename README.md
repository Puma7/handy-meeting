<p align="center"><img src="assets/handymeeting.svg" width="76" alt="Meeting for Handy logo"></p>
<h1 align="center">Meeting for Handy</h1>
<p align="center"><strong>Your meeting. Your computer. Your transcript.</strong></p>
<p align="center">Local recording, streaming transcripts and meeting notes with original quotes.</p>

<p align="center">
<a href="https://github.com/Puma7/meeting-for-handy/releases">Downloads</a> ·
<a href="README.de.md">Deutsch</a> ·
<a href="docs/RELEASING.md">Releasing</a> ·
<a href="https://github.com/Puma7/meeting-for-handy/issues">Feedback</a>
</p>

[![Rust checks](https://github.com/Puma7/meeting-for-handy/actions/workflows/rust.yml/badge.svg)](https://github.com/Puma7/meeting-for-handy/actions/workflows/rust.yml)
[![Linux and Mac packages](https://github.com/Puma7/meeting-for-handy/actions/workflows/rust-portable.yml/badge.svg)](https://github.com/Puma7/meeting-for-handy/actions/workflows/rust-portable.yml)
[![Release packages](https://github.com/Puma7/meeting-for-handy/actions/workflows/rust-release.yml/badge.svg)](https://github.com/Puma7/meeting-for-handy/actions/workflows/rust-release.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

An independent **Rust + Tauri companion for [Handy](https://github.com/cjpais/Handy)**. Record the audio from Zoom, Teams or another desktop meeting, optionally record your microphone as a separate track, and transcribe it locally while the meeting runs. Turn the transcript into summaries, decisions and tasks without sending the audio to a cloud service.

## Download

**[Download the latest complete release](https://github.com/Puma7/meeting-for-handy/releases/latest)**. Open Assets and choose the package for your computer. The versioned downloads are listed together with their checksums. [Older releases](https://github.com/Puma7/meeting-for-handy/releases) remain available.

| Your computer | Package type | Requirements |
| --- | --- | --- |
| Windows 10/11 · x64 | Setup EXE · portable ZIP | WebView2 and Microsoft Visual C++ x64 runtime; no .NET needed |
| Ubuntu 22.04+ / Debian 12–13 · x64 | DEB | PulseAudio or PipeWire-Pulse; package manager resolves declared libraries |
| Mac · Apple Silicon / M-series | Apple Silicon DMG | macOS 15+; recording permissions |
| Mac · Intel | Intel DMG | macOS 15+; recording permissions |

**Release status (28 October 2026):** the current public release is **[1.0.4](https://github.com/Puma7/meeting-for-handy/releases/tag/v1.0.4)**, with all 17 assets: five packages and twelve checksum/source proofs, including `RELEASE_MANIFEST.json`. Recordings are always preserved; from 1.0.6 Windows uses one program folder (see below).

**Future releases:** a matching version tag automatically starts the full Windows/Linux/Mac build matrix. Successful checks produce all five packages and their proofs in a release draft. Publish the populated draft to make the downloads public; publishing verifies the populated draft without starting another build. Draft PRs and ordinary main updates run source checks; full candidates run for ready PRs, explicit manual requests and release tags. [Release procedure](docs/RELEASING.md).

**Validation limits:** Mac/Linux device capture and a new full-app Rust two-hour run still require hardware validation. Mac builds are ad-hoc signed and not Apple-notarized; Windows setup is unsigned. Operating-system trust checks may block them. Do not disable system protection to install a build. Use independently backed-up test meetings first. [Measured evidence and open gates](TEST_REPORT.md).

The current release includes visible analysis progress, names and VIP flags, local history search, separately confirmed archives and audio copies, optional speaker experiments, and explicit update checks. See the [user guide](docs/1.0.2-USER-GUIDE.md) for the separate controls and their limits; its filename is retained from the 1.0.2 development stage.

## A meeting workflow, kept together

| Capture | Understand | Keep control |
| --- | --- | --- |
| Separate system and optional microphone tracks | Local streaming speech recognition from compatible Handy models | Audio capture continues independently of recognition failures |
| Live transcript, saved-audio counters and recording warnings | Summaries, decisions and tasks with verbatim source quotes | Local session history, original snapshots and retained analysis versions |
| Complete TXT/Markdown/JSON transcript export and one-click session folder | Editable named templates and ordered analysis tabs | Visible progress, elapsed time, targeted cancellation and resumable work |

![Meeting for Handy transcript and local file controls](assets/screenshots/transcript-light.png)
*Actual app components, rendered with synthetic demo data. The displayed two-hour timeline is a UI fixture, not a hardware endurance result. English and German interfaces are included.*

### Notes you can check

Choose a local text model, select a template and run the analysis. Every result links to an existing source section and a verbatim quote. Unknown owners and deadlines remain unknown. Each saved analysis keeps the transcript and instructions that produced it.

![Meeting for Handy source-linked analysis in light mode](assets/screenshots/analysis-light.png)

Add processing tabs, edit their instructions and save templates for different kinds of meeting. Original saved results remain unchanged when a reusable template changes. Copy all tabs together as Markdown, including evidence and the original transcript.

![Meeting for Handy analysis in dark mode](assets/screenshots/analysis-dark.png)
*Light, dark and system themes. Demo results are illustrative; source verification does not prove the model interpreted a statement correctly.*

## First meeting

1. Install **Handy** and download a compatible **streaming speech model** there. Meeting for Handy detects the installation and reads its current model list; model weights are not included in our downloads.
2. Install the package for your computer. With the Windows ZIP, extract the complete archive and use its start file. Keep the program folder and its libraries together. On Linux, install the DEB with your package manager; on Mac, copy the app from the matching DMG into Applications.
3. Open **Meeting → Recording setup**. Name the meeting and select a model and the audio output used by the meeting app. Enable your microphone if you want that separate track.
4. Start a short test and check both sources. The selected output can contain other apps' sounds, not just the meeting. On Mac, system capture uses the default output; choose the default microphone or another microphone listed by the bundled helper. Mac device selection still needs physical-device validation.
5. End the meeting, inspect its transcript, then open **Analyse this meeting**. **Open session folder** gives you the audio, journals, exports and saved analyses directly.

For a local summary, run a local text-model service such as LM Studio, load an appropriate text model and start its local server. The default address is `http://127.0.0.1:1234/v1/`. Meeting for Handy discovers the model IDs offered by that service and asks you to choose one. Speech models and text models serve different purposes; a speech model alone does not generate meeting notes.

## Privacy, recovery and honest limits

- **Audio stays local.** Speech recognition runs in the companion's own local worker processes. Handy's existing model files are reused; a running Handy inference process is not shared.
- **Local notes are the default.** Cloud text processing is optional and requires explicit approval for every run. It receives text, source IDs and instructions, without audio or local file paths. There is no automatic cloud fallback. A separately configured local server must itself execute models locally. Cloud requests and update checks connect directly without a system proxy; if your network requires one, use a local text model and download updates in a browser.
- **Keys are session-only by default.** Optional storage uses Windows Credential Manager, macOS Keychain or Linux Secret Service. Unreadable entries are preserved; there is no plaintext fallback.
- **Saved checkpoints are recoverable after tested process crashes.** Unconfirmed audio at the end can still be missing. This is not a power-loss or hardware-failure guarantee. Existing recordings, transcript journals and analysis versions are not automatically deleted.
- **Two tracks are not speaker identification.** Speaker names are only assigned when you confirm them. The optional post-meeting separation (beta) and live voice sections (experimental) give neutral per-section voice IDs, depend on a compatible model and do not identify people. Exact word timestamps and reliable words-per-person statistics are not implemented. Times identify recording sections. Notes remain reviewable AI drafts.
- **No hidden recording authority.** Local agents use private stdio with visible approval for each owned recording and separate microphone consent. Disconnect, revocation and limits stop only that agent's recording. This is not an MCP server.

Files live in the existing local `HandyMeeting/RustBeta` application-data folder for compatibility with earlier Rust builds. The name is retained to avoid moving or losing user data. The Windows uninstaller removes its listed program files; it does not recursively delete session storage. Files themselves are not encrypted; a folder-sync service can independently upload them.

## Built alongside Handy

The [original discussion](https://github.com/cjpais/Handy/discussions/1923) places meeting features outside Handy's own scope. Meeting for Handy explores that companion direction with its own lifecycle. It reads installed model metadata and uses the maintained transcribe.cpp bindings and a pinned native runtime. It does not modify Handy's settings, credentials or model weights. New model formats can require a compatible runtime update.

This is **an independent project**, not an official Handy product or an endorsed integration. A supported upstream companion contract is still being discussed. [Integration boundary](docs/HANDY_UPSTREAM_PROPOSAL.md).

## Developers and contributors

[Rust build and verification](docs/RUST_BUILD.md) · [Release workflow](docs/RELEASING.md) · [Native device checklist](docs/NATIVE_VALIDATION.md) · [Agent protocol](docs/RUST_AGENT_PROTOCOL.md) · [Local files](docs/MEETING_FILES.md) · [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md)

Dependencies and native downloads are pinned. CI checks Rust, frontend behavior, original-data recovery, source validation, native credential stores, relocated packages and Windows installer/uninstaller preservation. Build results and real device evidence are reported separately in [TEST_REPORT.md](TEST_REPORT.md). Some raw local evidence is intentionally excluded from the repository.

The previous C# prototypes and their recordings remain preserved. Their published packages and historical checks are documented in [BUILD_STATUS.md](BUILD_STATUS.md); they are not the Rust 1.0 packages above.

MIT licensed. Thanks to Handy, transcribe.cpp, Tauri and the maintained libraries this project builds on. Original dependency notices are included in [THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt) and `licenses/`. No model weights, recordings, settings or API keys are included in the source repository or download packages.


## Included in 1.0.4

Recording pause/resume and independent app microphone mute keep the meeting timeline with explicit silence and retained control events. Full transcript snapshots can be saved as TXT, safe Markdown or structured JSON with source IDs and section times. Explicit program/import choices now require a native picker and confirmation for the current app start. Physical interruption, endurance, Mac/Linux-device and signing gates remain separate from automated tests.

The visible name is **Meeting for Handy**. Existing data/key/bundle identifiers, executable filenames and the `HandyMeeting-<version>-...` download prefix stay compatible with earlier installations and updater clients. From 1.0.6 the Windows setup installs into one folder, `%LOCALAPPDATA%\Programs\Meeting for Handy`, and adds **Meeting for Handy** to the Start menu, so Windows search finds it. You can also add a desktop shortcut and choose English or German in the installer. Running a newer or the same setup again updates or repairs that folder once the app is closed. Version folders from 1.0.5 and earlier (`Programs\HandyMeeting\<version>`) are removed through their own uninstallers once the old app is closed; a folder whose app is still running stays and can be removed later in Windows Settings > Apps. Recordings and settings are kept; they live separately in `%LOCALAPPDATA%\HandyMeeting`. Windows does not let installers pin apps to the taskbar: right-click the running app and choose *Pin to taskbar*.

## Development toward 1.0.6 (unreleased)

A **live summary** panel beside the transcript summarizes a running meeting: summary so far, decisions, tasks, questions raised and important points, each with its source identifier, verbatim quote, time and audio track. It updates at a chosen interval and earlier when a long contribution on one audio track is followed by a reply on the other, works best with a small local text model, and can be copied to read out at any time. A **local model setup** finds Ollama or LM Studio, suggests Gemma 4 E2B/E4B from installed memory and can download the suggestion through Ollama. Design and limits: [1.0.6 plan](docs/1.0.6-LIVE-SUMMARY-PLAN.md).

## Development toward 1.0.5 (unreleased)

The [28 October audit corrections](docs/AUDIT_FIXES_20261028.md) cover capture diagnostics, navigation and layout, legacy file preservation and safe exports, report provenance, source-export containment and background-job handling. These source changes are separate from the public 1.0.4 download. The source is prepared as release candidate 1.0.5; it has not been tagged or released. Current checks and remaining limits belong in [TEST_REPORT.md](TEST_REPORT.md).

The development source also adds separate live meters directly below the system-audio and microphone controls. Green/yellow/red segments and digital peak values show incoming effective audio; clipping, silence, pause/mute and unavailable or old measurements have distinct states. The meters remain visible with recording setup collapsed. See [live input levels](docs/1.0.2-USER-GUIDE.md#live-input-levels-unreleased-development) for interpretation and limits.

General audio/video import, broader re-transcription variants, individual result-tab exports, reliable speaker statistics, mindmaps, generated overview images and outgoing integrations remain [optional roadmap work](docs/OPTIONAL_FEATURE_ROADMAP.md). The roadmap distinguishes existing controls from proposed additions.
