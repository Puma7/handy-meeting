# Audit corrections, 28 October 2026

Base: main `c8cd6092ef45a6a756458c627586a8ad6c0df1f1`, tree-identical to the retained PR27 head `2302b0fe0dd30d8c5b0855d3a62d3318cc8b66cf`. This patch keeps version 1.0.4 and all dependency locks. It is a source correction, not a new public release or hardware-readiness declaration.

| Audit item | Correction and regression boundary |
| --- | --- |
| Capture diagnostics could stop healthy system audio after optional microphone failure, or skip remaining joins | Diagnostic save failures remain visible in the open session; every owned writer is joined before returning collected errors. Windows/portable diagnostic paths preserve writable PCM. Real PCM-write errors still propagate. Deterministic tests use an unwritable manifest-backup entry while the real PCM/journal path remains writable. |
| Legacy predictable temporary-file write could affect an unrelated file | Shared persistence uses random exclusive sibling creation and rejects linked destinations/backups and untrusted child paths. Supplied repair inputs are checked before writes. Atomic replacement preserves final hardlink siblings. Independent review additionally identified hardlinked WAV repair and journal append; repairs now replace a bounded copied file and the first journal append detaches existing bytes, retaining exact backups. |
| Legacy transcript Markdown retained active input | Shared literal escaping covers title, model, warnings and body, consistent with notes exports. TXT/JSON and source objects retain their original values. |
| Undefined border token | Existing design-system line color replaces all four undefined token references. |
| Retention checkboxes appeared above centered labels | Labels use horizontal alignment and wrap without detaching controls from their text. |
| Navigation retained the old workspace scroll position | Primary page changes reset the actual workspace and focus its heading. Source jumps and background updates remain independent. Heading focus uses the existing focus color. Detail speaker components have distinct keys. |
| Memory trend reports lost a phase when PIDs repeated | Phase and process identity are separate report keys; regression covers a common controller PID and reused worker PID. |
| Soak report hardcoded an old version | New plans retain the tested assembly version; old plans explicitly lack that evidence. |
| Interrupted recovery invented an old file-lock cause and fixed crash count | Reports keep cause unknown, use actual receipt counts and preserve existing recovery reports. Reference-word claims include the actual checked section count and the 29-second threshold; short-only runs do not receive a positive semantic claim. |
| SQLite dangling links | Existing link validation also covers absent-target DB/WAL/SHM entries. This is local-storage hardening, not a newly established attacker boundary. |
| Source export link containment | Reject symbolic/reparse files and directories before traversal/read/copy and enforce resolved containment. Real exporter regressions preserve unrelated sentinel content and private nested evidence. |
| Repeated background-job lifecycle | Search and maintenance share admission, cancellation, clock and terminal publication. Their separate error policies and captured operation leases remain intact. |
| Stale migration/security descriptions | Current Rust mute/adapters/native keyring are distinguished from retained C# previews and from actual physical-device readiness. |

The unproven descendant-pipe lifetime hypothesis remains a documented hardening lead, not a confirmed defect: no supported Handy query retaining inherited handles was demonstrated, and approved native code already has user-context execution authority. It was not used to justify changing process termination or killing descendants.

## Validation

- Locked/offline Windows Rust workspace, all targets, no default features: **255 passed**, two dedicated child-fixture entrypoints ignored by their outer harness. Full-scope Clippy denies warnings; Rust formatting passed.
- Frontend: **124 tests** in 13 files; TypeScript, final Vite production build and formatting passed. Fresh synthetic Edge/Playwright previews checked 1280×900 and the supported 800×600 minimum, light/dark focus, row alignment, borders and navigation. A 390px exploratory view is not claimed as native-window support. No native data/device/model was used.
- Required `verify.ps1` on the final source: **41 PCM/filesystem/export, 35 notes, 28 template, 8 agent and monitor checks passed**, in a new isolated program folder. The monitor includes pure version/cause/receipt/reference-scope regressions.
- Additional C# streaming/recovery: **10 passed**, including damaged-manifest backup, torn journal continuation, retained text history and a synthetic accelerated two-hour file timeline. That timeline is not a real two-hour capture test.
- Shared portable C# core compiled with locked restore and zero warnings/errors on Windows. This validates the portable code variant's build, not macOS/Linux execution.
- Python combined report/export/package suites: **27 passed**. New source-export link cases and phase-preservation checks failed against the original implementation before passing after correction.
- An isolated original-source reproduction changed/removed its synthetic external sentinel and retained raw Markdown. The same trigger against fixed production sources preserved the exact sentinel SHA256 and escaped the markup. No external renderer or network fetch was needed. Current shared regressions additionally preserve hardlinked audio/journal siblings, exact backup bytes and ordinary repeated saves.
- Independent security candidate review found the additional audio/journal hardlink routes, which were corrected and regressed. Independent integration review found the reference-section overclaim, which was corrected and included in the final monitor check. No remaining concrete finding in their reviewed scope is claimed without this disposition.

All earlier snapshots, programs, user recordings/transcripts/journals and failed/successful test evidence remain retained. These local automated checks do not replace real portable audio devices, human accessibility checks, a new full Rust endurance run, the original GPU trend investigation, signing/notarization or existing upstream/dependency gates. No real cloud-provider test, live audio capture, foreign-process termination or publication occurred during local validation.

The user's complete optional feature input and comparative research are tracked separately in [OPTIONAL_FEATURE_ROADMAP.md](OPTIONAL_FEATURE_ROADMAP.md).
