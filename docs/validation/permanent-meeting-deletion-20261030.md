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
The preceding published candidate02 passed all 234 UI tests plus TypeScript/production build.
Its targeted core suite passed 15 erasure cases; native annotation-fence and transition
regressions passed. That source passed 367 full workspace/all-target Rust tests
(two existing special entry points ignored), all 234 frontend tests,
TypeScript/production build, formatting, Clippy with warnings as errors and a
fresh sequential verify.ps1 (47 PCM, 35 notes, 28 templates, 8 agent checks and
monitor). This result does not replace fresh target CI or clean external reviews.
No real meeting was used.

## Further complete-review corrections

Maintenance can hold a source snapshot after its session lease ends. Every native
search-index update now revalidates the source identity while holding shared index
admission through the write. Explicit erasure holds exclusive admission across
source removal and cache purging. A delayed restore/audio-archive snapshot cannot
recreate erased transcript content. The regression failed before this correction
for the actual native and archived snapshot forms.

A canceled archive writer can leave a ZIP without its final archive manifest.
Validated pending attribution now permits its explicit removal. Available ZIP,
session and backup identities must still agree; readable foreign identities and
unattributed ZIPs remain blocked. Real cancellation of the synthetic archive
writer failed to reach erasure preview before the correction.

Interrupted atomic metadata is parsed by bounded JSON structure, including a
complete meetingId field in an otherwise EOF-truncated record. IDs embedded in
titles, paths or quoted JSON are never deletion authority. Foreign records remain
byte-identical. Malformed ordinary non-temporary records still fail closed.
Unassignable temporary remnants are preserved and their count is displayed before
confirmation; they are explicitly outside the claimed removal of attributed data.
A changed count invalidates the preview. Their contents cannot be claimed erased.

After three consecutive failed/missing/unknown status replies, the erasure dialog
stops automatic retries and requests cancellation. It retains the exact native
job and blocks a new deletion until a known terminal status is actually observed.
An explicit status retry remains available; cancellation acknowledgment alone
never calls the stopped/completed callbacks. Synthetic failure/recovery and safe
focus tests cover this path. The former observer continued polling indefinitely.

The annotation-admission fixture now acquires the real erasure OS lease before
invoking a second Service's metadata/speaker command. Both must refuse with source
bytes intact; both succeed after release. This removes the competing lock probe
and scheduler-dependent two-second fixture deadline. It remains synthetic
two-Service/OS-lock coverage, without a separate-process runtime claim.

The preceding candidate03 passed 371 Rust workspace/all-target tests (the two
existing explicit special entry points normally ignored), 236 frontend tests,
TypeScript/production build, Rust/frontend formatting and Clippy with warnings
as errors. Eighteen synthetic core erasure tests passed. A sequential verify.ps1
passed all 47 PCM, 35 notes, 28 template and 8 agent checks plus monitor.
The real rendered dialog was checked with synthetic IPC at 1280px and 390px,
including light/dark warning disclosure, bounded status failure and a later known
cancellation. Neither viewport overflowed. No real meeting or native application
was erased; these are not hardware, separate-process or endurance results.


## Candidate04 review corrections

Retention preview construction and the atomic publication of its content-bearing
plan now share erasure's process-shared archive lease. A delayed cooperating app
cannot write an old title-bearing plan after successful erasure. A deterministic
two-Service fixture holds the real OS lease, checks refusal without publication,
then verifies normal publication and removal after release. This is not a
separate-process runtime test. The former publisher failed that exclusion check.

Archive attribution now reconciles readable ZIP, primary and backup source
identities before a foreign receipt or earlier orphan can filter a copy out.
Contradictory attribution blocks removal before any source bytes change. Synthetic
ZIP/source fixtures with a foreign receipt failed before the correction and
preserve all original bytes after refusal. Incomplete unreadable ZIP bytes still
require explicit pending attribution; no guessed foreign ownership is introduced.

Shared archive receipt staging files use the same bounded exact meetingId parser.
Unassignable metadata/speaker staging files have no trustworthy meeting ID, so
they remain untouched and contribute to the disclosed remnant count. The final
success notice also retains that warning: such files can still contain meeting
data and need manual inspection. Titles, quotes and embedded IDs never authorize
their deletion. Stable archives/.leases coordination remains, accepting only
ordinary empty SHA-256-named lock files. Unexpected or nonempty entries block the
operation. Existing archive recovery tests caught a temporary misclassification
of that directory; the corrected full recovery suite passes without weaker rules.

A rejected cancellation request after the native destructive job ID arrives no
longer ends observation or clears destructive admission. The dialog continues
watching that exact job until its terminal status is established; it never offers
a new erasure merely because cancellation failed. The delayed-ID/cancel-failure
regression failed with the former handler. A separate full-App test verifies both
normal completion and the surviving-remnant warning after clearing stale replies.

This candidate passed 375 Rust workspace/all-target tests, with the two existing
special entry points normally ignored; 21 synthetic core erasure cases; 238
frontend tests; TypeScript and production build. Rust/frontend formatting and Clippy with warnings as errors also passed. A fresh
verify.ps1 passed 47 PCM, 35 notes, 28 template and 8 agent checks plus monitor.
Its build briefly overlapped the tail of an additional filtered native test
invocation; the complete earlier Rust run and Clippy had already finished. That
filtered invocation selected no case because its module prefix was omitted; it
is retained as such, and a corrected exact-name rerun is recorded separately. No real user meeting, provider service, audio device or
external export was deleted. Hardware, separate-process, Linux/macOS runtime,
GPU, physical/forensic erasure and endurance claims remain outside this proof.

## Candidate 05: embedded identity, repeated interruption and publication exclusion

A successful archive inventory/hash verification now also checks each readable
embedded primary and backup session identity before the ZIP can be selected for
erasure. The same check applies to orphan ZIPs without a receipt. Four synthetic
fixtures keep the outer manifest and all content hashes valid while introducing
a foreign primary or backup identity, with and without a receipt. All refuse
erasure and preserve the existing files. The former code accepted the first such
ZIP despite its foreign embedded identity.

An already confirmed interruption marker retains the checked archive identity
when a later interruption has removed the original creation pending file but
has not yet removed its incomplete ZIP. A new preview and typed confirmation
remain necessary. Readable contradictory ZIP identities still block removal;
an unrelated unverified ZIP gains no authority. A regression uses an actual
cancelled archive creation and stops erasure between pending-file and ZIP removal.
The former retry failed; the corrected retry removes the remaining attributed
bytes and clears its minimal marker.

Analysis, including resumed analysis of an archived transcript, now holds a
process-shared publication lease from before reading sources or snapshots until
all checkpoint/result writes end. Live-summary and optional speaker workers
retain the same shared lease through their worker lifetime, and transcript
exports retain it through saving. Explicit erasure and the existing archived
permanent-deletion paths require exclusive publication admission before removal.
This adds exclusion, without changing automatic retention age or VIP rules.

The stable coordination lock is outside meeting and cache namespaces. Publishers
can coexist and do not reacquire the archive/annotation lease: no capture handle
or recording transition is held by this new lease. A core fixture proves that
two concurrent publishers are admitted, and erasure remains blocked until both
end. Native fixtures pause new and resumed archived analyses at their own
loopback HTTP stub after persisted snapshot creation. Erasure is refused until
the request and writes end, then succeeds. The old analysis failed this exclusion
check. These are synthetic same-process threads/Services exercising OS locks,
not a separate-process, real model-provider or physical-device test. External
programs and older binaries that ignore these locks are outside this guarantee.

The review's suspected maintenance-guard lifetime problem was not reproduced.
The actual deletion worker is now directly testable through a small extracted
start helper, with its guard scope unchanged. Its native regression performs a
synthetic erasure, observes the exact job's complete status, then immediately
acquires indexing admission and successfully rebuilds the derived index. The
guard ends before terminal status publication; no admission rule was relaxed.

The full Rust workspace/all-target run passed 380 tests; the two existing special
entry points remain normally ignored. All 24 synthetic core erasure cases pass.
The frontend, dependencies, review rules and complete historical TEST_REPORT.md
remain byte-identical to published candidate 04. Its 238 frontend tests and
TypeScript/production build are previous evidence for those unchanged bytes,
not newly executed frontend suites. No real meeting, external export, model
service or user program was changed or erased.

Rust formatting and Clippy with warnings as errors passed. The sequential
verify.ps1 run also passed 47 PCM, 35 notes, 28 template and 8 agent checks plus
monitor. Earlier restricted temporary-folder access and an invalid status in a
new completion fixture are retained as infrastructure/fixture failures; neither
is reported as a failing application regression or a successful verification.

## Candidate 06: bounded exclusive index admission

Exclusive erasure admission now uses the existing metadata lock helper instead
of a single try-lock attempt. It retains that helper's 25 retries with 10 ms
delays; no wait or test limit is increased. Shared index readers still fail
immediately while erasure owns the lock. A sustained reader still prevents
erasure admission, before any destructive operation. The branch results remain
io::Result values before the existing user-facing error mapping; no error is
converted to an uninformative unit value.

The new synthetic regression holds an actual shared OS file lease and releases
it only after the exclusive path has observed contention. A scoped, one-shot,
thread-local test observer makes that ordering deterministic without sleeping
or competing probe loops. The old single-attempt path failed this test; the
corrected path acquires admission after the reader ends. The same fixture checks
that new shared admission remains blocked until the exclusive lease is dropped.
A second case keeps its reader throughout the bounded attempts: erasure remains
blocked and its synthetic content file is byte-identical. These are two file
handles in one process, not a separate-process erasure or physical-device test.
The observer is compiled only for tests and restores prior thread state even
during unwinding.

All 382 Rust workspace/all-target tests pass with warnings denied and the same
two existing special entry points regularly ignored. The 24 core erasure cases
are retained alongside the two new index-admission cases. Frontend sources,
dependencies, protection/review rules and the entire historical TEST_REPORT.md
remain byte-identical to published candidate 05. The earlier 238 frontend tests
and TypeScript/production build apply to those unchanged bytes and were not
rerun here. No real user meeting, external export, provider or installed program
was changed. All previously documented erasure and model-residency integration
limits remain in force.
