<img src="assets/handymeeting.svg" width="80" height="80" alt="HandyMeeting logo">

# HandyMeeting

**Local meeting audio, live transcripts, and notes you can trace back to the original.**

[![Windows checks](https://github.com/Puma7/handy-meeting/actions/workflows/windows.yml/badge.svg)](https://github.com/Puma7/handy-meeting/actions/workflows/windows.yml)
[![Portable package checks](https://github.com/Puma7/handy-meeting/actions/workflows/portable.yml/badge.svg)](https://github.com/Puma7/handy-meeting/actions/workflows/portable.yml)

An independent companion prototype for the [Handy](https://github.com/cjpais/Handy) ecosystem. Capture system audio and an optional microphone separately, transcribe locally while the meeting runs, and create source-linked summaries, decisions and tasks afterward.

[Deutsche Anleitung](README.de.md) · [Build status](BUILD_STATUS.md) · [Platform status](PORTABILITY.md) · [Agent API](AGENT_API.md) · [Test evidence](TEST_REPORT.md) · [Contributing](CONTRIBUTING.md)

**Rust/Tauri migration:** `1.0.0-beta.2` adds local text-model discovery, explicit meeting selection, confirmed-task progress, cancellation and source-linked Markdown. Linux and both Mac architectures now have capture adapters and native candidate package workflows. See [actual evidence and remaining gates](TEST_REPORT.md), [migration scope](docs/RUST_MIGRATION.md) and [portable Rust candidates](docs/RUST_PORTABLE.md). Existing C# releases and recordings are preserved; this remains a migration beta, not a release candidate.

## Why a companion app?

In [Handy discussion #1923](https://github.com/cjpais/Handy/discussions/1923), the maintainer put meeting mode outside Handy's scope and mentioned a separate companion application. This project explores that direction. It is independently developed, uses its own name and logo, and has no official integration agreement or endorsement.

HandyMeeting detects a compatible Handy installation, reads its installed-model metadata, and reuses existing local model weights. The companion includes a pinned transcribe.cpp runtime and runs inference in its **own process**. It does not inject into Handy or share an already-loaded model process. A stable shared model-discovery or service contract is an open discussion topic.

## What works today

This feature list describes the existing prototypes. The Rust beta's migrated scope and remaining differences are listed in [RUST_MIGRATION.md](docs/RUST_MIGRATION.md).

- Separate local system-audio and optional microphone tracks.
- Streaming transcription using a compatible model already installed through Handy; tested with Nemotron Streaming 3.5.
- Audio capture stays independent of inference. Existing audio and transcript checkpoints are retained when recognition fails.
- Local session history, interrupted-session recovery, and full-transcript copying.
- Local-first post-processing with original quotes and source references. Optional cloud processing requires explicit text consent for each run and sends no audio.
- Named templates with editable instructions and ordered analysis tabs; copy all results together.
- A local, documented agent protocol for capability discovery, approved transcript reads and local analysis. Recording control is not exposed by protocol v1.

## Platform status

| Platform | Status | Important limits |
| --- | --- | --- |
| Windows x64 | Working WPF prototype, version 0.4.1 | Requires .NET Desktop Runtime 10 and compatible Handy/model installation |
| Ubuntu / Debian x64 | Portable Avalonia preview, version 0.5.0-preview.1 | Synthetic process/recovery tests; real desktop, audio-device and long-session tests still required |
| macOS Apple Silicon | Experimental DMG built on an actual arm64 Mac runner | macOS 15+; ad-hoc signed; real-device capture is not validated |
| macOS Intel | Experimental DMG built on an actual Intel Mac runner | macOS 15+; same limitations; separate architecture-specific package |

This is a prototype. Speaker diarization, exact word timestamps and reliable words-per-person statistics are not implemented. Two audio tracks do not establish speaker identity. AI notes remain reviewable drafts: checking a verbatim quote does not prove that its interpretation is correct.

## Downloads and first run

Published prototype packages: [Windows 0.4.1](https://github.com/Puma7/handy-meeting/releases/tag/v0.4.1) and [Mac/Linux 0.5.0-preview.1](https://github.com/Puma7/handy-meeting/releases/tag/v0.5.0-preview.1). The portable release contains separate Apple Silicon and Intel DMGs and an Ubuntu/Debian x64 DEB. SHA-256 manifests accompany the downloads; exact build evidence is in [BUILD_STATUS.md](BUILD_STATUS.md).

**Windows:** extract the complete Windows package, install .NET Desktop Runtime 10, then run the included start file. Install Handy and a compatible streaming model first. Select the same audio output used by your meeting app, optionally enable your microphone, and start recording. Keep the extracted directory together.

**Linux preview:** install the `.deb` using your package manager so dependencies are resolved. A local PulseAudio-compatible server, including PipeWire-Pulse, is required. The preview does not start or replace your system audio service. Read [PORTABILITY.md](PORTABILITY.md) before recording an important meeting.

**Mac preview:** use the package matching Apple Silicon or Intel, on macOS 15 or newer. Microphone and system-audio permissions are required. These preview builds are ad-hoc signed and are not Apple-notarized releases. Real hardware testing remains necessary after a successful package build.

For local notes, run a compatible local text-model service such as LM Studio or Ollama. The default endpoint is `http://127.0.0.1:1234/v1/`; select a locally executed instruct model with structured-output support. Speech recognition and text analysis use separate model types. The companion does not download either model's weights.

## Privacy and recovery boundaries

Audio, transcript journals and original snapshots are stored locally. Audio is written in short PCM files, independently of the recognizer. Confirmed transcript steps are persisted before progress is acknowledged. Recovery tests cover confirmed data after process termination; an unconfirmed tail can still be missing. Process termination is not a power-loss test.

The optional cloud text path requires explicit per-run consent. It receives transcript text, processing instructions and source IDs, without audio or local file paths. There is no automatic cloud fallback or provider switch. Local transport permits loopback endpoints only and disables proxies and redirects. A separately configured local service is responsible for its own upstream behavior.

Windows keys use current-user encryption. Portable-preview keys stay in memory for the session; native keychain integration is unfinished. Original recordings and derived versions are never automatically deleted. Sessions are ordinary local files; a folder-sync service can independently transmit them.

## Build and verify

For the Rust/Tauri beta, use the [dedicated build and verification instructions](docs/RUST_BUILD.md). The commands below remain the C# prototype build paths.

Use .NET SDK 10 as pinned in `global.json`. Dependencies and native-runtime downloads are version-locked; native downloads are checked against fixed SHA-256 values.

```powershell
# Windows
./verify.ps1
./build.ps1

# Portable Linux build including its .NET runtime
./scripts/build-portable.ps1 -Runtime linux-x64
```

```sh
# Package the published Linux application on Linux
bash scripts/package-linux.sh

# Build the native helper and preview DMG on macOS with Xcode Command Line Tools
bash scripts/package-macos.sh arm64
bash scripts/package-macos.sh x64
```

See [PORTABILITY.md](PORTABILITY.md) for prerequisites and outstanding platform checks. Successful CI packaging is not evidence of real audio-device reliability. [TEST_REPORT.md](TEST_REPORT.md) distinguishes local measurements, synthetic checks and untested conditions. Some raw evidence links are intentionally local and excluded from source control; new CI reports are retained as workflow artifacts.

## Roadmap and collaboration

Immediate priorities are real-device Mac/Linux capture and recovery tests, longer portable runs, packaging/signing, portable feature parity, and agreement on how a companion should discover and reuse Handy models without depending on private internals.

Feedback on the integration boundary is welcome. Meeting UI, recordings, history and notes should stay in the companion; speech recognition should reuse maintained libraries and models. See [MASTERPLAN.md](MASTERPLAN.md) and [Issues](https://github.com/Puma7/handy-meeting/issues).

MIT licensed. Dependency and runtime notices are in [THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt) and `licenses/`. Model weights, recordings, transcripts, settings and API keys are excluded from the source repository.
