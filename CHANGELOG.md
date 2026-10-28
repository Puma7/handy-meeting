# Changelog

## 1.0.6 — live summary and easy local text models (in development)

- **Live summary while recording:** a side panel summarizes the running meeting – summary so far, decisions, tasks, open questions and important points – so the meeting can be recapped at any moment. Updates run every 1–15 minutes once at least 200 new characters are confirmed, earlier when a long contribution of one audio track (90 seconds or more) is followed by a reply on the other track, or after a lot of new speech, on request and once more after catch-up; less new text waits for the next trigger or "Update now".
- Compact summaries: newly confirmed text is summarized in append-only windows, and a rolling overview of at most eight points is rebuilt from the verified statements in batches, so requests stay small for local models. Every verified statement stays in the detailed lists; the overview is a summary and can leave points out.
- Every live statement passes the same literal source check as post-meeting analysis; the audio track ("microphone" or "computer audio") is derived from its evidence, never invented. Live files are created once, re-verified when read and never overwritten or deleted.
- Local models first; a cloud or company endpoint needs a text release for every start and never starts automatically. Optional automatic start with every recording (local only).
- **Local model setup:** detects Ollama and LM Studio on this computer, suggests `gemma4:e2b` or `gemma4:e4b` from installed memory and can ask a running Ollama to download the suggested model. The last local endpoint and model are remembered.

## 1.0.5 — input meters, audit fixes and efficient CI (release candidate)

- Independent live system-audio/microphone meters with clear silence, clipping, stale and control states.
- Preserve healthy capture through diagnostic failures; correct linked-file writes, Markdown exports, report provenance and private source-export boundaries.
- Consistent borders, aligned retention controls and navigation focus; shared derived-job lifecycle.
- Bounded native SDK download retries without weakening certificate, size or hash checks.
- Source checks on drafts/main; complete candidates on ready PRs/manual/tag requests; publication reuses the verified draft without a second build.
- Existing user data, older programs and all prior evidence remain retained. Physical-device/endurance/GPU/signing/upstream limits remain separate.

## 1.0.4 — Meeting for Handy follow-up (released)

- Recording pause/resume and separate app microphone mute with old-buffer privacy gates, explicit silent sections and retained control events.
- Trustworthy Windows microphone discontinuities align to the monotonic/device clock; impossible jumps fail before padding.
- Complete TXT/Markdown/JSON transcript snapshots with original source IDs and section-time semantics.
- Native purpose/path/SHA256-bound executable/import selection; shared operation leases including metadata changes.
- Separate capture, archive, UI, session and command modules; malformed restored audio markers cannot escape their session.
- Meeting for Handy display branding across UI and new packages, with earlier data/update compatibility. Installer helpers preserve execution policy.

## 1.0.3 — recording reliability, recovery and platform follow-ups (in development)

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

## 1.0.2 — library, retention and update controls (in development)

- Show analysis stage, confirmed steps and elapsed time while a meeting is processed; failed or rejected steps stay resumable without repeating confirmed work.
- Add meeting names, VIP flags and a rebuildable local search index for history, with search hits that open the highlighted source section.
- Add disabled-by-default retention policies with a reviewed preview, verified archives, restore, FLAC/Opus audio copies and permanent deletion only after typed confirmation. An interrupted deletion is listed and can be resumed; it removes only logged files whose hash still matches.
- Add optional, separately confirmed speaker experiments and evidence-checked name suggestions. Original transcripts are never changed.
- Check official releases on request; Windows downloads the installer with size and SHA-256 checks and installs only after confirmation, while Linux and macOS link to the release page. Installation waits for recordings and background jobs.
- Keep a single stable Start menu link owned by the installer; user-changed links are preserved.

## 1.0.1 — installer and recovery fixes

- Fix first-time Windows setup when the default parent folders do not exist; retain exclusive destination reservation and existing user files. Native regression checks cover missing nested parents, occupied directories and parent paths blocked by files. Full CI installation now starts with a fresh nested path.
- Split the long combined template workflow test into independent preservation cases, retaining the existing five-second per-case timeout.
- Keep all workspace crates, desktop configuration and lockfile metadata on one application version.
- Require the fresh-parent installation check before accepting a Windows Setup package.
- Bring the three reviewed recovery/package-proof fixes from PR #9 into the next version; they were not included in public 1.0.0.

## 1.0.0 — initial public release



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

## 0.5.0-preview.1 — portable development

- Shared platform-neutral C# core for durable PCM, transcript journals, streaming coordination and source-linked notes.
- Avalonia desktop preview with Ubuntu/Debian capture adapters and macOS capture-helper source.
- Self-contained Linux package and separate Apple Silicon/Intel Mac build targets.
- Synthetic persistence, process-abort, local inference and headless UI checks. Real-device portable validation remains outstanding.
- Portable API keys are session-only; full keychain integration and UI parity are unfinished.

## 0.4.1 — Windows prototype

- Clearer analysis instructions and output-style explanations, with a visible edit action.
- Original logo, application icon and favicon assets.
- Local JSON-RPC-over-stdio agent interface with explicit transcript access, capability descriptions and local-only analysis.
- Independent Windows verification and native streaming regression checks.

## 0.4.0

- Full-transcript copying beyond the bounded live excerpt.
- Named templates, custom ordered tabs and immutable per-run instruction snapshots.
- Copy all result sections as readable text with references and original quotes.
- German/English interface, light/dark appearance and explanatory tooltips.
