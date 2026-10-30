# Explicit permanent meeting deletion — 2026-10-30

A newly recorded native meeting, including a zero/two-second or failed meeting, can be explicitly removed from History or its detail view without first archiving it. A preview names exactly one meeting and counts its attributed files. The user must type the displayed confirmation word. This is irreversible, has no trash copy, and is separate from automatic retention. Automatic age, VIP and complete-archive rules remain unchanged. Manual confirmation can remove a pinned meeting.

## Scope and safeguards

The operation removes both tracks, PCM/journals/transcript revisions/backups, live and post-analysis snapshots, speaker experiments, converted audio, exports stored inside the meeting, annotation revisions, attributed whole/audio-only ZIPs and retained sources/restoration staging, audio-copy catalog entries and matching content-bearing maintenance plans/logs. The complete rebuildable SQLite/FTS cache, WAL/SHM and retained unreadable cache copies are removed; the index is rebuilt from remaining meetings afterward. Other original meetings/settings/models/keys are not deleted. A failed index rebuild is reported separately from successful file removal.

An exclusive native maintenance reservation refuses any active capture, recognition, analysis, live summary, export, indexing or mutation, including the reservation-to-worker gap. Cross-process source relocation, annotation, archive and index locks protect participating app processes. Bounded ordinary-file inventories reject links/reparse points, foreign identities, unknown cache entries and changed bytes. Each remaining file is rechecked before unlinking, and only empty directories are removed. A changed inventory requires a new preview; late files survive and produce an explicit partial failure.

Cancellation, I/O failure or app exit can leave an irreversible partial removal. A minimal local continuation marker retains only meeting/archive identities and confirmation time, never titles, quotes, paths or an inventory copy. A new preview and confirmation remove remaining bytes even when manifests/receipts were already removed. The marker and its backup disappear after verified completion. Empty stable coordination lock files can remain; they contain no meeting content and must not be unlinked while other processes may refer to them. This is removal of app-managed content, not guaranteed erasure of every filesystem identifier.

Native and React state drop the selected inactive meeting's cached view/results/retry references. Request generations discard late replies, so an old snapshot cannot restore a removed meeting to the interface. Initial focus is on the safe close action; keyboard focus remains within the confirmation.

## Explicit limits

Imported legacy originals are read in place outside app-owned storage; this action does not delete those originals and is not offered on legacy rows. External exports, hardlink copies, clipboard, backups, cloud/provider histories and OS/device forensic traces are outside this operation. No overwrite/free-space wiping or forensic secure-erasure guarantee is made. No real user meeting was deleted during development. No capture, ASR algorithm, model endpoint or prompt was changed. PR37 model-residency coordination is a separate draft change; integration must release its retained meeting reservations if both changes are combined. This PR makes no claim of clearing a model server's memory/history.

## Validation

Fixtures are synthetic local files/PCM, not a hardware recording or real-model test. Tests cover direct short/empty/failed/corrupt native meetings, both tracks, all attributed content classes, whole/audio-only archives, every file interruption boundary, stale/wrong confirmation, late writes, source/index leases, hardlink isolation, Windows directory junction rejection, conflicting source/archive identities, unaffected other originals and FTS content removal. Unix symlink coverage is compiled/executed by target CI; a Windows run does not prove that target result.

Frontend interaction tests cover exact typed confirmation, no Enter bypass, cancellation before a native job ID arrives, foreign preview rejection, partial failure, safe focus/Escape, restart continuation and a delayed pre-erasure app-state reply. The rendered dialog was inspected with synthetic IPC in dark/light and wide/narrow layouts; it is not an installed native end-to-end deletion test.

The original published candidate `1c58782be6786727c8454e43712c5658ee6fd19f` passed local Windows validation: 363 Rust workspace/all-target tests, with two existing explicit entry points normally ignored (native credential provider and crash-writer child); 234 frontend tests; TypeScript/production build; Rust formatting; Clippy with warnings as errors; frontend formatting; and verify.ps1 (47 PCM, 35 notes, 28 templates, 8 agent checks, monitor). Thirteen new core erasure tests, a native preview-capability test and a native admission regression are included. The first verify run hit the unchanged monitor fixture replacement deadline under concurrent test load; a sequential repeat passed the same deadline. Earlier fixture-only compile/encoding corrections and failed checks remain in private evidence; they are not represented as product regressions. Current target CI and review links are tracked in the PR, not inferred from local runs. No release, installer, merge, physical storage erasure, Linux/macOS runtime success or two-hour capture claim follows from local synthetic checks. The historical TEST_REPORT.md remains byte-unchanged; this sanitized note is the current feature-specific evidence boundary.


## First full-review corrections

The first CodeRabbit full review (5712541593) and second Codex code review
(5712644774) found six distinct issues, including a shared orphan-marker issue.
Their full risk/architecture/extra comments were read; neither is recorded as a
clean final-head review. The PR was returned to draft while these were corrected.

- Storage maintenance now has its own reservation flag. Capture start/stop and
  microphone reconnect transitions keep saved text readable; maintenance still
  blocks reads from reservation through worker completion, including stale cache
  insertion. The synthetic transition regression failed with the former guard.
- Native metadata and speaker edits hold the same process-shared archive lease
  across source reading, annotation commit and index refresh. Two isolated service
  instances with real filesystem locks verify the whole write scope; the former
  command had no such fence. This protects cooperating current app instances,
  not arbitrary older binaries or programs that ignore the storage protocol.
- Both primary and backup manifest identities are checked before adding restore
  staging to the selected meeting's inventory. Attributed and orphan staging
  cannot grant permission to remove an intact foreign recording. Missing or
  unreadable staging manifests alone do not prevent removal of an incomplete
  owned tree. The foreign-staging regression failed before this correction.
- Orphaned atomic-write staging files are retained as inert entries, never used
  as deletion authority and never allowed to hide valid continuation markers.
  Actual JSON marker identities remain strict. Recovery discovery and completion
  were tested with a leftover temporary file; discovery failed before the fix.
- The erasure dialog participates in application-wide background inertness and
  keyboard trapping. History navigation and replacement of the submit button by
  progress no longer move focus outside a running destructive dialog. The full
  App regression starts from meeting details, checks focus/inertness through that
  navigation and also rejects a delayed old snapshot. It failed before the fix.
- Search/index contention now gives neutral retry guidance to ordinary readers,
  without suggesting that a search user permanently delete anything.

Function-level erasure capability, inventory and synthetic-fixture intent was
also documented. The documentation coverage requirement remains 80%; only a
fresh provider result can establish the new coverage and clean-review status.
The current UI suite passes all 234 tests plus TypeScript/production build.
Targeted core tests pass 15 erasure cases; native annotation-fence and transition
regressions pass. The corrected local source now passes 367 full workspace/all-target Rust tests
(two existing special entry points ignored), all 234 frontend tests,
TypeScript/production build, formatting, Clippy with warnings as errors and a
fresh sequential verify.ps1 (47 PCM, 35 notes, 28 templates, 8 agent checks and
monitor). This result does not replace fresh target CI or clean external reviews.
No real meeting was used.
