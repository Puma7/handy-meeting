# Changelog

From 1.0.8 on, the changes of every version are listed automatically in its [GitHub release](https://github.com/Puma7/meeting-for-handy/releases): creating the release generates the notes from the merged pull requests. This file keeps the history up to 1.0.7 and the plans that are not tied to a release.

## Planned for 2.0

- Remove the C# prototype (0.4/0.5): `src/`, `verify.ps1`, `build.ps1`, `setup-native.ps1`, `global.json`, `NuGet.config`, `System.Numerics.Tensors.*`, the NuGet/Avalonia/Inter license files and `docs/legacy/`. The repository then contains only the Rust project.

## Unreleased

- **Releases need one step:** creating a GitHub release with a new tag sets the version everywhere (`scripts/set-version.py`), builds and checks all five packages, attaches them and lists the merged changes. 1.0.7 failed because its tag did not match the version in the files; that check is replaced by taking the version from the tag.
- **Tidier repository:** removed stale launchers, status files, the source manifest, the C# prototype download workflows and outdated plans; measurement records moved to `docs/evidence/`, C# prototype documents to `docs/legacy/`. Shorter README, README.de and release guide.

## 1.0.7 — recording backlog and simpler workflows (29 October 2026)

- Windows microphone clock gaps retain their exact gap evidence without creating a separate audio/recognition job for every tiny gap. Capture returns to stop/pause controls between bounded drain turns instead of treating cumulative processed audio as an invalid backlog.
- Live text-model requests wait while speech recognition has more than five seconds of confirmed audio pending. Already running model requests are not cancelled by this scheduler.
- Resumed live summaries ignore completed same-model audio even if an older progress counter remains; unfinished and other-model audio still receive recognition priority. Audio-copy confirmations are cleared when paging hides the selected row, including lookahead records.
- The live transcript retains earlier sections, periodically refreshes catch-up text outside the recent status preview, and highlights literal Unicode searches without shifting matches.
- A compact running view keeps both level meters and pause/mute/stop controls visible. Narrow windows offer a direct transcript/summary switch. Rename controls and export actions align, and compact-audio pagination uses separate page metadata.
- A damaged compact-copy catalogue entry on the next page no longer hides the current valid page. Page metadata is independent of contents; requested invalid records still fail closed and all original files remain unchanged.
- Settings and post-processing share one connection/model form. Detection reads LM Studio's configured server port and a local-only Ollama host; local/cloud drafts are kept separately and transient credentials are cleared on switching. Templates and optional speaker tools have a clearer grouping; local speaker jobs require an explicit capability check.
- Live task deduplication retains different owners and deadlines, and saved live summaries from another meeting are rejected without changing their files. Confirmed speaker names load even while editing controls are closed; model-discovery actions respect a running analysis lock.
- Speaker capability can be checked in Settings before the first recording and is retained across unchanged automatic catalogue refreshes. Explicit loopback bind addresses are preserved. Catch-up refreshes and preview sequencing retain later text, stopped summaries clear their waiting label, and speaker-check evidence has a fixed 128-folder ceiling that preserves existing complete and partial records.
- Ollama detection preserves an explicit HTTP port 80 and follows Ollama's distinct defaults for bare hosts and complete HTTP URLs.
- A recognition track that has ended no longer blocks live summaries of confirmed text from the other track; pending audio remains available for later recognition.
- Completed speaker-capability proofs can be reused after restart only for the same freshly verified model and native runtime identity; incomplete or conflicting records never enable a model.
- Full transcript publications use the same per-section freshness rules as bounded previews, preserving newer recognition independently of audio-journal sequencing.
- Full or conflicting speaker-check archives offer an explicit preserving archive action, guarded across app instances. Existing proof files remain intact, the active 128-record bound stays fixed, and temporary failures retain a direct retry action.

## 1.0.6 — live summary and easy local text models (28 October 2026)

- **Live summary while recording:** a side panel summarizes the running meeting – summary so far, decisions, tasks, questions raised and important points – so the meeting can be recapped at any moment. Updates run every 1–15 minutes once at least 200 new characters are confirmed, earlier when a long contribution of one audio track (90 seconds or more) is followed by a reply on the other track, or after a lot of new speech, on request and once more after catch-up; less new text waits for the next trigger or "Update now".
- Compact summaries: newly confirmed text is summarized in append-only windows, and a rolling overview of at most eight points is rebuilt from the verified statements in batches, so requests stay small for local models. Every verified statement stays in the detailed lists; the overview is a summary and can leave points out.
- Every live statement passes the same literal source check as post-meeting analysis; the audio track ("microphone" or "computer audio") is derived from its evidence, never invented. Live files are created once, re-verified when read and never overwritten or deleted.
- Local models first; a cloud or company endpoint needs a text release for every start and never starts automatically. Optional automatic start with every recording (local only).
- **Windows installer:** one folder `Meeting for Handy` without a version number; running setup again repairs or updates it once the app is closed (1.0.5 refused a second run). Start menu entry created natively, so Windows search finds the app even under the default Restricted PowerShell policy (1.0.5 left no entry there). Optional desktop shortcut (silent: `/DESKTOPSHORTCUT`) and "Start Meeting for Handy" on the finish page; uninstall removes only the links this setup created that still start its program, English/German language choice, a multi-size setup and app icon (1.0.5 had a single 256 px image) and "Meeting for Handy" as copyright holder. Former version folders are removed through their own uninstallers; recordings and settings stay. The updater also accepts a future `MeetingForHandy-` setup name.
- **Calmer desktop after the 1.0.5 walkthrough:**
  - Pages use the full window width, and the transcript scrolls with the page instead of in a second scrollbar.
  - New search field inside the transcript.
  - Markdown is the default export.
  - Pause/mute and FFmpeg explanations, plus new retention-rule explanations, are hover tooltips; the "selected tracks are recorded" line is gone.
  - The section-time hint is a tooltip on each time.
  - Audio-driver notices are collapsed and translated again (the 1.0.5 wording was shown twice in English).
  - Level meters fall smoothly instead of flashing to zero on a breath.
  - A finished search-index update no longer stays in the footer.
  - Settings buttons have spacing, and an empty archive says why it is empty.
  - When no model answers at the configured local endpoint, the analysis page opens the Ollama/LM Studio detection by itself, including a hint to start the LM Studio server.
- **Local model setup:** detects Ollama and LM Studio on this computer, suggests `gemma4:e2b` or `gemma4:e4b` from installed memory and can ask a running Ollama to download the suggested model. The last local endpoint and model are remembered.

## 1.0.5 — input meters, audit fixes and efficient CI (28 October 2026)

- Independent live system-audio/microphone meters with clear silence, clipping, stale and control states.
- Preserve healthy capture through diagnostic failures; correct linked-file writes, Markdown exports, report provenance and private source-export boundaries.
- Consistent borders, aligned retention controls and navigation focus; shared derived-job lifecycle.
- Bounded native SDK download retries without weakening certificate, size or hash checks.
- Source checks on drafts/main; complete candidates on ready PRs/manual/tag requests; publication reuses the verified draft without a second build.
- Existing user data, older programs and all prior evidence remain retained. Physical-device/endurance/GPU/signing/upstream limits remain separate.

## 1.0.4 — Meeting for Handy follow-up (28 October 2026)

- Recording pause/resume and separate app microphone mute with old-buffer privacy gates, explicit silent sections and retained control events.
- Trustworthy Windows microphone discontinuities align to the monotonic/device clock; impossible jumps fail before padding.
- Complete TXT/Markdown/JSON transcript snapshots with original source IDs and section-time semantics.
- Native purpose/path/SHA256-bound executable/import selection; shared operation leases including metadata changes.
- Separate capture, archive, UI, session and command modules; malformed restored audio markers cannot escape their session.
- Meeting for Handy display branding across UI and new packages, with earlier data/update compatibility. Installer helpers preserve execution policy.

## 1.0.3 — recording reliability, recovery and platform follow-ups (28 October 2026)

- Stop a meeting without waiting for the recognition backlog; the session shows "catching up" and completes in the background. A track that stops while the other continues is reported on its own (#17, #20).
- When recognition continues on CPU after a GPU failure and decodes a section differently, keep the result as an explicitly marked new text version instead of leaving the section pending forever; earlier text stays in the journal (#20).
- Close every audio part at its confirmed prefix when a session is opened, record the end of capture for every post-recording status, and write journals and the manifest outside the shared session lock so a slow disk cannot delay audio capture (#15, #16, #18).
- Reject dangling links before opening or creating session, archive, label, speaker and retention files; keep a readable TXT owned across a crash during its refresh; retry briefly when a session lease is still held by a closing process (#21).
- Show an unreadable archive entry as preserved instead of hiding the whole archive list; let an interrupted archive move be repeated while keeping its earlier placement record (#21).
- Find a section when the search combines a meeting-title word with a transcript word; the search index converts in place (#22).
- Drop and count individual analysis statements that fail the source checks instead of failing the whole run; keep analyses from earlier prompt versions readable and mark them; skip damaged saved runs instead of hiding the list (#19).
- Download updates to a `.partial` file that becomes the installer only after its size and SHA-256 match; Linux and macOS show their own package and open the official release page. Status bars poll quickly only while a job runs (#23).
- Keep the Start menu link on the newest side-by-side installation; uninstalling the linked version moves the link to the newest remaining one (#23).
- `localhost` reaches a local text-model server on IPv4 or IPv6 loopback without system DNS or proxies. Linux: versioned Handy AppImages in `~/Applications` are found, and the DEB adds `/usr/bin/handymeeting`. macOS: choose a real microphone. CI gains a virtual PulseAudio capture check and a GUI launch smoke check (#24).
- Start only Handy- and FFmpeg-named programs chosen through the app, and add a locked `cargo audit` CI gate (#25).

## 1.0.2 — library, retention and update controls (not released separately; part of 1.0.3)

- Show analysis stage, confirmed steps and elapsed time while a meeting is processed; failed or rejected steps stay resumable without repeating confirmed work.
- Add meeting names, VIP flags and a rebuildable local search index for history, with search hits that open the highlighted source section.
- Add disabled-by-default retention policies with a reviewed preview, verified archives, restore, FLAC/Opus audio copies and permanent deletion only after typed confirmation. An interrupted deletion is listed and can be resumed; it removes only logged files whose hash still matches.
- Add optional, separately confirmed speaker experiments and evidence-checked name suggestions. Original transcripts are never changed.
- Check official releases on request; Windows downloads the installer with size and SHA-256 checks and installs only after confirmation, while Linux and macOS link to the release page. Installation waits for recordings and background jobs.
- Keep a single stable Start menu link owned by the installer; user-changed links are preserved.

## 1.0.1 — installer and recovery fixes (27 October 2026)

- Fix first-time Windows setup when the default parent folders do not exist; retain exclusive destination reservation and existing user files. Native regression checks cover missing nested parents, occupied directories and parent paths blocked by files. Full CI installation now starts with a fresh nested path.
- Split the long combined template workflow test into independent preservation cases, retaining the existing five-second per-case timeout.
- Keep all workspace crates, desktop configuration and lockfile metadata on one application version.
- Require the fresh-parent installation check before accepting a Windows Setup package.
- Bring the three reviewed recovery/package-proof fixes from PR #9 into the next version; they were not included in public 1.0.0.

## 1.0.0 — initial public release (26 October 2026)

- Present the Rust/Tauri application with bilingual guides and screenshots rendered from synthetic demonstration data.
- Build Windows Setup EXE and ZIP, Ubuntu/Debian DEB, and separate Apple Silicon/Intel DMGs from the same checked source.
- Verify actual isolated Windows installation/uninstallation and native credential stores before assembling a complete release draft. Publishing the verified draft marks a stable version Latest.
- Preserve earlier program versions and user data; refuse mismatched source proofs, changed assets and recursive uninstall deletion.
- Synchronize synthetic pipe tests with actual helper readiness rather than process existence. Hardware recording, full-app two-hour validation and signing remain separately documented release gates.

## 1.0.0-beta.4 — clearer meeting workflow and local text exports

- Collapse recording setup during capture and analysis setup during processing or result review. Keep the recording controls available and long transcripts in a keyboard-scrollable region.
- Show understandable German/English explanations for known audio-driver interruptions, with original technical messages available on demand and preserved in session metadata. Unknown warnings remain literal data.
- Open the exact selected meeting folder in the system file manager; save complete current transcripts as new UTF-8 text snapshots in its exports folder. Prior exports and original journals are retained. Failed writes remain pending, never a finished TXT export.
- Keep analysis stage, confirmed-task percentage, elapsed time and targeted cancellation visible while scrolling or visiting another page. Compact completed feedback and historical-prompt explanations.
- Verify 480 synthetic original sections, same-named meeting selection, export failures, literal untrusted warning text and small-window rendering. No capture or recognition-engine change is included.

## 1.0.0-beta.3 — issue implementation and verification

- Manage named templates explicitly with create, duplicate, rename, reviewed removal and optional browser import. Keep prior revisions and historical analysis prompts immutable; reject conflicting or damaged stores.
- Add optional Windows Credential Manager, macOS Keychain and Linux Secret Service storage. Session-only remains the default; locked/unreadable/ambiguous entries cannot be overwritten or silently replaced by plaintext.
- Add a private local agent protocol with visible consent for each new owned session, separate microphone consent, bounded live reads, precise stop ownership and revocation on disconnect/deadline. A revoked/expired preparation cannot open capture afterward. This is not MCP.
- Expose a consumer-owned versioned Handy integration description without model paths or session data; document the proposed upstream boundary without claiming maintainer agreement.
- Add explicit opt-in long device diagnostics and confirmed PCM-prefix recovery checks; Mac/Linux physical-device validation remains outstanding.
- Add a preregistered dense native-worker memory comparison preserving the original warning, binaries and thresholds. This isolated experiment does not replace a full-app two-hour test.

## 1.0.0-beta.2 — guided meeting analysis and portable candidates

- Discover local text-model IDs from the configured loopback server; guide LM Studio setup and require an explicit model choice.
- Separate meeting history/details from new recording; identify analysis sources with title, date, time and duration.
- Show confirmed-task progress, elapsed time, cancellation and failure feedback; preserve exact resumable analysis requests.
- Copy validated Markdown with literal original quotes and internal source references.
- Read Handy's installed-package version without trying an unsupported CLI flag.
- Drain queued Windows audio packets and stop tails, retain bounded device diagnostics and surface device failures instead of padding over them.
- Add Linux PulseAudio/PipeWire-Pulse and macOS capture adapters, official native SDK preparation, DEB/DMG candidate packaging and actual target-host CI jobs. Device and long-session validation remain release gates.
- Preserve byte identities of locked dependency and license files across Windows checkouts; retain old programs and user sessions.

## 1.0.0-beta.1 — Rust/Tauri migration

- Windows migration target with an independent Rust workspace and a Handy-style Tauri/React interface.
- Installed Handy remains authoritative for fresh model lists; official transcribe.cpp Rust bindings reuse installed weights.
- Immutable model/runtime identity, isolated per-track recognition workers, durable audio/text journals and bounded live previews.
- Read-only legacy session access, original-quote analysis, editable templates, local-first transport and explicit per-run cloud text consent.
- Windows hardware/long-session validation, portable Rust capture adapters and complete legacy feature parity remain release gates. Existing prototypes and recordings are preserved.

## 0.5.0-preview.1 — portable development (C# prototype)

- Shared platform-neutral C# core for durable PCM, transcript journals, streaming coordination and source-linked notes.
- Avalonia desktop preview with Ubuntu/Debian capture adapters and macOS capture-helper source.
- Self-contained Linux package and separate Apple Silicon/Intel Mac build targets.
- Synthetic persistence, process-abort, local inference and headless UI checks. Real-device portable validation remains outstanding.
- Portable API keys are session-only; full keychain integration and UI parity are unfinished.

## 0.4.1 — Windows prototype (C#)

- Clearer analysis instructions and output-style explanations, with a visible edit action.
- Original logo, application icon and favicon assets.
- Local JSON-RPC-over-stdio agent interface with explicit transcript access, capability descriptions and local-only analysis.
- Independent Windows verification and native streaming regression checks.

## 0.4.0 (C# prototype)

- Full-transcript copying beyond the bounded live excerpt.
- Named templates, custom ordered tabs and immutable per-run instruction snapshots.
- Copy all result sections as readable text with references and original quotes.
- German/English interface, light/dark appearance and explanatory tooltips.
