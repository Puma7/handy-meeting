# Meeting for Handy user guide

This guide describes the released [1.0.7 controls](https://github.com/Puma7/meeting-for-handy/releases/tag/v1.0.7). The microphone reconnection and live-overview recovery below are newer, unreleased source changes. AI notes and speaker results remain reviewable drafts with original section references. Installation checks do not establish recording reliability on every device. See the [English](../README.md) or [German README](../README.de.md) for packages, prerequisites and the first meeting.

## Recording controls and transcript files

During a recording, pause/resume controls the app's capture; the separate app microphone mute affects only its microphone track. Neither control mutes Teams, Zoom or another meeting app. Paused/muted intervals retain explicit silence and control events on the meeting timeline. Devices may remain open and the operating system may continue to show its recording indicator. Confirmed UI state follows the native control acknowledgement. Device permissions, interruptions and endurance still need the separate hardware checks in [TEST_REPORT.md](../TEST_REPORT.md).

Use **Save transcript** to choose a complete TXT, Markdown or JSON snapshot of the selected meeting, including both available tracks. Markdown escapes active input; JSON retains source IDs and section times. These are section times, not precise word alignment. Each export is an additional file; original journals, saved analyses and earlier exports are retained. **Open session folder** opens that meeting's local audio, journals and exports. General MP3/MP4/WAV import and individual result-tab export remain [roadmap work](OPTIONAL_FEATURE_ROADMAP.md).

<a id="live-input-levels-unreleased-development"></a>

## Audio devices

While the app window is visible, connected or removed devices appear automatically within a few seconds; the refresh button next to **System audio** reads the list at once. If a selected device disappears, a notice names the device chosen instead. During a recording, a red warning names the track whose device left the list, and says the track stopped once the microphone capture confirms it; audio saved so far is kept. The released 1.0.7 app requires ending the recording and starting a new one once the device is back.

## Microphone reconnection and overview recovery (unreleased)

If the microphone track has actually ended while system audio still records, **Reconnect microphone** retries only the originally selected device. Device-list refresh alone does not reconnect capture. No replacement microphone is selected during recording. The action waits for native acknowledgement; pause and app mute remain in effect. Previously closed audio is verified and retained, and missing microphone time is recorded as silence with a gap event. Missing speech is never recovered. If the device or saved audio cannot be verified, the retry fails while the system track stays owned. Agent-owned recordings require revoking agent control first. A maximum of 32 explicit retries per recording bounds diagnostic generations. Ordinary recording startup still refuses to reuse an existing audio track.

The rolling overview now has its own coverage and update time. Newer individual notes do not make an older overview complete. **Retry summary** resumes the matching saved run after a failure, or begins a separate run when the model or prompt identity changed; earlier windows and overview versions stay on disk. Copied notes include overview coverage and the incomplete warning. A long recognition backlog produces a visible waiting notice after two minutes; it does not terminate audio capture. Stop live notes and retry later if necessary. Cloud processing continues to require a fresh explicit text release.

Bounded local diagnostic files distinguish connection, HTTP, incomplete response, validation and persistence failures without saving prompts, replies, transcript text, keys or endpoint paths. New number and nonbinding-task checks are conservative filters, not proof that a model interpretation is correct; always inspect original quotes. Older saved analyses remain readable under their original evidence contract.

## Live input levels

The released app provides one meter under **System audio** and one under **Microphone**. Both stay visible when recording setup is collapsed. They become available after you start a recording; opening this view does not start a device preview or grant microphone access. Green indicates signal, yellow a high level and red a very high level. If **Clipping** appears, lower the source's volume or microphone gain and check again. Silence is shown separately from missing measurements. The meters follow this app's pause and microphone-mute controls; stopping or switching meetings clears old values.

Values are digital peaks in **dBFS**, with 0 at the full-scale limit. They measure the effective 16 kHz mono samples after the platform adapter's downmix and capture gates, rather than room loudness or separate hardware input channels. Brief peaks/clipping are held for about 300 ms; RMS weights the samples in the same bounded window. If packets stop, the last received value is retained for less than one second, then shown as unavailable. Fast polling clears stale or delayed IPC responses; padded timeline gaps are not invented input signal. Only scalar levels reach the UI, without raw audio, transcript text or file paths.

A moving meter helps check whether a track receives sound; use the separate saved-audio counters and warnings to check recording progress. It does not prove a disk checkpoint, speech quality or a successful transcript. Physical-device and long-session validation remain separate from synthetic meter tests and virtual-device CI.

## Visible work and errors

Opening **Evaluate this meeting** opens setup. Choose a local text model and explicitly start analysis. The bottom status bar shows the current step, elapsed time, confirmed-work percentage when the total is known, and cancellation. Unknown totals use an indeterminate bar. A completed, failed or cancelled job stops its clock; errors and previously confirmed work stay visible. Long archive, search, download and optional speaker work also have separate status displays.

## History, names and readable text

- Rename a live or historical meeting and mark important meetings **VIP**. Original capture dates and durations stay independent of the display name. Edits have separate retained revisions.
- Search titles and original transcript sections in **History**. Update the local search index to include older existing meetings; indexing runs in the background and can be cancelled. Results are paged and open the original source section.
- `transcript.txt` is an additional continuously refreshed copy beside the authoritative journals. If an observed external edit differs from the app's ownership receipt, the app preserves it and displays a warning. Durable journals remain the recovery source; no power-loss guarantee is implied.

<a id="three-separate-optional-speaker-features"></a>

## Optional speaker tools

1. **Post-meeting labels and name candidates:** select an original section, enter a personal label and a literal quote, and explicitly confirm. The local name-candidate template can propose names quoted in the text, but never automatically assigns a person. Being mentioned or addressed does not prove who is speaking. Review suggestions before confirming a label.
2. **Post-meeting audio separation · beta:** choose an already installed compatible Handy model and start a separate local CPU job. Unsupported native roles fail visibly without changing original text or audio.
3. **During recording · experimental:** a separate worker examines newly closed audio sections, with a corresponding delay. It does not share or control the recording/ASR worker. Cancel stops only this optional speaker job.

Neutral voice IDs are scoped to one section and track. They may change between sections and do not identify people. No automatic cross-section voice matching or precise word alignment is established. Model/language quality and a positive compatible-model result must be evaluated separately; weights are never downloaded automatically.

### Speaker-check recovery (since 1.0.7)

The follow-up groups the optional speaker tools and makes model-capability checks available in Settings before the first recording. Completed proofs can be reused only for the same freshly verified model and native runtime. If the active proof folder is full or contains conflicting results, **Archive check records** preserves the existing complete and partial proofs together in a separate local archive. Click it explicitly, then run the model check again. Every file stays on disk; recordings and analysis results are unchanged. Recording, analysis and another model check block this action. A temporary lock failure leaves the retry action available. Linked or unexpected records stop archival and remain unchanged. No proof is removed automatically.

## Retention, archives and compact audio

Open **Settings → Retention and archive**. Every rule starts disabled. Save rules, create a read-only preview, review one proposal, then confirm that individual action. Changing rules or source files invalidates an older preview. A preview is usable once and expires after 24 hours. VIP, active, incomplete, recent and unreadable meetings are protected.

- **Audio only:** verify a lossless ZIP before removing the matching raw audio files. Original text/journals stay in history, linked to the archived original source. Restore audio through **Local archives** before continuing ordinary recognition.
- **Whole meeting:** verify a ZIP containing all ordinary files and retained label revisions, then move the original folder out of normal history into the local archive area. The move initially keeps both ZIP and an additional original source copy. To reclaim space, use **Keep archive compactly**, review the separate confirmation, then keep the verified lossless ZIP and remove only its redundant raw copy. Unknown or changed files block compaction; interrupted compaction can be continued safely. Restoring creates a new destination and never overwrites an existing session. Archive copies remain until a later explicit action.
- **Additional FLAC/Opus copies:** requires a separately installed FFmpeg executable selected through the native picker in settings and confirmed for the current app start. FFmpeg is not bundled. FLAC must reproduce the original PCM exactly. Opus is lossy, must decode within the declared duration bound, and may affect recognition. Conversion retains original audio. A new transcription additionally requires exact saved PCM/duration verification, a model choice and confirmation. It creates a separate meeting/model snapshot; old transcripts and analyses are retained. Restore a whole archived meeting before using its compact copies.
- **Permanent deletion:** only a whole archived meeting absent from normal history can be proposed. The preview covers all verified archive versions, retained source copies and label revisions belonging to that meeting. Type the exact displayed `DELETE <meeting ID>` text. VIP/state/inventory are checked again immediately before deletion. Cancellation stops remaining steps; files already deleted cannot be restored. A stopped or interrupted deletion appears under **Local archives** and can be continued with the same typed confirmation; it deletes only the logged files that are still unchanged. A per-file operation log records confirmed progress. Changed, unreadable or unknown files cause a visible stop and are preserved for inspection. This is ordinary file deletion, not a claim of physical secure erasure.

Nothing is automatically applied merely because an age/count rule is enabled. Partial restoration or failed maintenance retains its available files and reports the next action; inspect the local operation log rather than deleting folders to make an error disappear.

## Updates and Windows installation

Use **Settings → Updates** to check for a new version. On Windows, then explicitly download and confirm installation in the app. Downloads use the official GitHub release, exact installer size and SHA-256; partial or changed bytes never execute. This verifies integrity from the release source; the installer itself is not code-signed. On Linux and macOS the app only checks: download the `.deb` or `.dmg` from the linked official release page and install it as described in the README. Update checks and cloud text models connect directly and do not use a system or corporate proxy; where a proxy is mandatory, download updates in the browser and use a local text model. Complete recording, recognition, notes, speaker and archive work before installation. Other Handy/LM programs are not stopped. The visible Start menu name is **Meeting for Handy**; Windows 1.0.6 and later use one program folder, as described in the README; earlier version folders follow the documented migration rules. Owned shortcuts can be updated while user-modified or foreign shortcuts are preserved. Earlier installed files and user recordings remain retained.

See [TEST_REPORT.md](../TEST_REPORT.md) for actual executed checks and outstanding device, endurance, signing and dependency limitations. Prepared workflows are not test results.
