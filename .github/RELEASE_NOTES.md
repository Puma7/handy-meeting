## Meeting for Handy 1.0.6

A release of the independent Rust/Tauri meeting companion for Handy. The release tag and attached manifests identify the exact application version, source commit and checked package bytes.

### Live summary while recording

- A side panel summarizes the running meeting: summary so far, decisions, tasks, open questions and important points. Read it out when someone asks what was discussed, or use it for the closing recap.
- Updates every 1–15 minutes once at least 200 new characters are confirmed, earlier when a long contribution on one audio track is followed by a reply on the other, or after a lot of new speech, on request, and once more when catch-up has finished. Less new text waits for the next trigger or "Update now". On half a screen the summary sits above the transcript; a focus button shows only the summary.
- Every statement keeps its original quote and audio track (microphone or computer audio). The detailed lists keep every verified statement; the short summary on top can leave points out. Statements without a literal source are discarded and counted. Results stay reviewable AI drafts.
- Local text models are the default and can start automatically with every recording. A cloud or company endpoint needs a text release for every start; no audio is sent.

### Easy local text models

- Finds Ollama and LM Studio on this computer and lists their models. Suggests `gemma4:e2b` (8 GB memory) or `gemma4:e4b` (16 GB or more) and can download the suggestion through a running Ollama. Nothing is downloaded without a click.

### Reliability and desktop clarity

- Separate live system-audio and microphone meters stay visible below their controls. Green/yellow/red segments, digital peak values and clear silence, clipping, pause/mute and unavailable states help check incoming audio independently of the transcript. Meters do not start devices while idle or replace saved-audio progress checks.
- Recording diagnostics preserve healthy audio capture when a warning cannot be saved. Stopping still waits for every owned audio writer and reports failures.
- Consistent borders, aligned retention checkboxes and page focus make navigation clearer without interrupting background work or source jumps.
- Legacy session writes and repairs preserve unrelated linked files and original backups; transcript Markdown treats supplied text literally. Original TXT, JSON and saved source values remain intact.
- Memory and interruption reports retain the tested version, process/phase identity and actual checked scope instead of inventing causes or measurements.
- Portable builds retry temporary SDK-download failures within a fixed limit. Hash, certificate, size, source and installer checks remain required.

### Recording, exports and local controls

- Pause/resume recording and mute only the app microphone. Ignored audio becomes explicit silent sections; other apps are unchanged. Depending on the platform, the operating system may still display a recording indicator.
- Full transcript snapshots as TXT, inert Markdown or structured JSON, with original section/source identifiers and explicit section-time semantics.
- Native file choice and confirmation for local Handy/FFmpeg programs and imported sessions. Approvals bind purpose/path/SHA256 and expire when the app closes.
- Conservative Windows microphone clock-gap alignment and explicit capture-control provenance; impossible clock jumps stop before padding. Missing audio is not recovered speech.
- Shared operation leases and additional archive-marker/path protections while retaining live rename and all original data.
- Visible name **Meeting for Handy**; existing internal data/key/bundle/executable/package identifiers remain compatible. Installer helpers respect execution policy.

### Meeting workflow

- **Meeting library:** names, VIP flags and a rebuildable local search over meeting titles and transcripts. Search hits open the highlighted source section.
- **Retention and archive:** disabled-by-default retention rules with a reviewed preview, verified archives, restore, FLAC/Opus audio copies and permanent deletion only after typed confirmation. Interrupted moves and deletions can be continued.
- **Updates:** check official releases on request. Windows downloads the setup to a temporary file and installs it only after its size and SHA-256 match and you confirm. Linux and macOS open the release page. Installation waits for recordings and background jobs.
- **Optional speaker experiments** with evidence-checked name suggestions. Original transcripts are never changed.
- **macOS:** choose a real microphone (not yet tested on Mac hardware).

### Recording and recovery

- "Stop meeting" returns at once; remaining sections are recognized in the background ("catching up"). A recognition run that ends without a full result leaves a resumable session.
- A track that stops while the other continues is reported on its own. Trustworthy Windows microphone gaps are marked with explicit silence; uncertain clocks are rejected and warnings remain visible.
- When recognition continues on CPU after a GPU failure and decodes a section differently, the new text is kept as a marked revision; earlier text stays in the journal.
- Opening a session closes every audio part at its confirmed prefix. Journals and the manifest are written outside the shared lock, so a slow disk does not delay audio capture.
- The readable TXT copy retries briefly after a short file lock instead of staying out of date.
- Dangling links are refused before session, archive, label, speaker and retention files are opened.

### Analysis

- Statements that fail the source checks are dropped and counted per section instead of failing the whole analysis. Analyses from earlier prompt versions stay readable and are marked; damaged saved runs no longer hide the list.

### Platforms and installer

- The Start menu link stays on the newest side-by-side Windows installation, including beta versions (beta.10 after beta.9).
- Linux: versioned Handy AppImages in `~/Applications` are found (only ones that can run), and the DEB adds `/usr/bin/handymeeting`.
- `localhost` reaches a local text-model server on IPv4 or IPv6 loopback without system DNS or proxies.

### Choose your download

- **Windows 10/11 x64:** `windows-x64-setup.exe`, or extract the complete `windows-x64.zip`. WebView2 and Microsoft Visual C++ x64 are required; .NET is not needed.
- **Ubuntu 22.04+ / Debian 12–13 x64:** install `linux-amd64.deb` with your package manager. PulseAudio or PipeWire-Pulse is required.
- **Mac M-series / Apple Silicon:** `osx-arm64.dmg`, macOS 15+.
- **Mac Intel:** `osx-x64.dmg`, macOS 15+.

Install Handy and a compatible downloaded streaming model first. The companion detects its model metadata and reuses the existing weights in its own local inference processes. For local summaries, additionally run a local text-model service such as LM Studio and choose a text model in the app.

Meeting for Handy can find later releases itself (Settings → updates). Version 1.0.1 has no update check; install the latest complete release manually once. Earlier program versions and all user data are preserved.

### Read before relying on a recording

Hardware, long-session and abort tests for the changed capture and recognition paths, Mac/Linux device capture and a new full-app Rust two-hour run still need real-device validation. The automated checks use synthetic data and virtual devices. Driver interruption warnings remain meaningful; confirmed checkpoint recovery is not a power-loss guarantee and an unconfirmed audio tail can be missing. Diarization and exact word timestamps are not implemented. Meeting notes remain reviewable AI drafts.

Mac packages are **ad-hoc signed, not Apple-notarized**. Windows setup is **unsigned**. System trust checks can block these builds. Do not disable system protection to install them. Use short independently backed-up test sessions first.

A locked `cargo audit` check reports no known vulnerabilities; nine maintenance warnings and one existing GLib unsoundness warning are unchanged from 1.0.1. See the test report for scope and limits.

SHA-256 sidecars, `SHA256SUMS.txt`, package proofs and `RELEASE_MANIFEST.json` accompany the complete matrix. The manifest records the exact source commit and fingerprint; it explicitly distinguishes automated checks from unperformed real-device/notarization checks. No models, recordings, settings or keys are included. Existing user data and earlier releases are preserved.

[English guide](https://github.com/Puma7/meeting-for-handy/blob/main/README.md) · [Deutsche Anleitung](https://github.com/Puma7/meeting-for-handy/blob/main/README.de.md) · [Test evidence](https://github.com/Puma7/meeting-for-handy/blob/main/TEST_REPORT.md) · [Open issues](https://github.com/Puma7/meeting-for-handy/issues)

Meeting for Handy is independently developed and is not an official Handy product or an endorsed integration. MIT licensed; original dependency and installer notices are included.
