# Build and release status

## 1.0.1 first-install correction in preparation

The reported Windows 1.0.0 failure was reproduced in an owned synthetic directory: NSIS `GetFullPathName` returned an empty parent for a nonexistent nested default path; `CreateDirectoryW` failed with error 3. Parsing the parent with FileFunc GetParent succeeds without requiring existing folders. A native harness compiles the production reservation function and checks missing parents, occupied directories and a parent blocked by a file. Restoring the old parent lookup makes the new test fail; the correction passes all three cases. No user installation, registry entry or recording was touched by this local harness. Actual CI Setup installation now uses a pristine nested directory; release assembly requires its fresh-parent proof.

The later main-branch Intel-Mac candidate run [40384230272](https://github.com/Puma7/handy-meeting/actions/runs/40384230272) failed one combined template UI workflow at its 5,000 ms deadline, before building the app. The same source had passed the prior PR build. The combined workflow is split into independent migration/create, duplicate/rename and confirmed-removal preservation checks without timeout increases or retries. This is separate from the successful earlier 1.0.0 tag build.

An empty public v1.0.1 was created on `1985750` before application versions were raised, causing the explicit tag/version guard to reject its build. No files from that failed attempt exist. The corrected source consistently uses 1.0.1; publication must retain the existing 1.0.0 bytes and identify the final exact 1.0.1 commit. The previously reviewed PR #9 recovery fixes are included in this follow-up, rather than retroactively claimed for 1.0.0. Full verification and target CI results will be recorded after execution.


## Public downloads and PR #10 (26 October 2026)

[Version 1.0.0](https://github.com/Puma7/handy-meeting/releases/tag/v1.0.0) was published by `Puma7` at 11:27:41 UTC on `cecb03c541ad1b442143f9112e658103d35ccc6b`. The actual [tag-triggered download workflow](https://github.com/Puma7/handy-meeting/actions/runs/40315235014) completed the Windows Setup/ZIP, Linux DEB and both Mac DMGs, native credential/installer checks, assembly and attachment. All 17 public assets were downloaded again and matched GitHub SHA-256 digests and sizes; all 15 manifest file rows, five package hashes/sidecars and the exact tag commit matched. Public download links now resolve. The simultaneous release-event run was cancelled; the successful tag run is the evidence, not the cancelled run. These are package checks, not device/endurance/notarization evidence. Published assets were not changed.

[PR #10](https://github.com/Puma7/handy-meeting/pull/10) was merged as `02a6edd3470f6d69d1b6f8952895d14583032742` after two completed clean Codex code reviews on `527a763` and green [Windows](https://github.com/Puma7/handy-meeting/actions/runs/40328585210), [Rust](https://github.com/Puma7/handy-meeting/actions/runs/40328585180) and [Linux/Mac](https://github.com/Puma7/handy-meeting/actions/runs/40328585311) CI. Four retained target license manifests match the locked serde_with 3.22.0 update. One original Ubuntu lease-test failure passed unchanged on retry; its cause is not proven fixed. A separate security completion is not claimed. This update is newer than the already public 1.0.0 binaries.

Future matching version tags run the full download pipeline and populate a draft automatically; publication makes the files public. Tag/release/manual events for the same version now share the short tag concurrency key, avoiding duplicate same-version publishers. An operator must bump application versions for the next release. The three PR #9 follow-up fixes are preserved for a separately reviewed follow-up; this documentation/workflow change does not claim to ship them in 1.0.0.


Repository: [Puma7/handy-meeting](https://github.com/Puma7/handy-meeting), publicly shared with the owner's approval on 25 October 2026. This is an independent Handy companion application.

## Current 1.0.0 release preparation

On 26 October 2026 at 11:24:27 UTC, `Puma7` merged PR #9 outside this automated review run into the temporary branch `codex/review-baseline-1.0.0` at `4080da7593d33b9de74e2fe50440a9066d07fcc7`. The PR had not been retargeted to `main`, which remained at `cecb03c541ad1b442143f9112e658103d35ccc6b` when checked. Current-head security completion was still unconfirmed. The monitor therefore ends under its unexpected-closure instruction; it did not perform or undo that merge, publish a v1.0.0 release, or post the conditional Handy discussion comment. Existing verified packages and evidence are retained. [Closure evidence](.build/codex-review-loop/unexpected-closure.json). This is not a release approval.

The follow-up [PR #9](https://github.com/Puma7/handy-meeting/pull/9) reviewed the complete migration against a temporary pre-migration baseline. Three confirmed first-round findings were fixed in `588fb54f8118c6d580ea264013916a9c636ddb72`: unfinished-analysis discovery after restart with fresh cloud consent, backup-recoverable history entries, and final portable archive hash/size/name binding. [Round 2 code review](https://github.com/Puma7/handy-meeting/pull/9#issuecomment-6412778624) completed without new findings on that head. The separate security review still has no recorded completion for that head; its clean result for `44dea12` is older evidence. The later merge into the temporary baseline is recorded above.

That corrected application source passed actual [Windows/Core and installer checks](https://github.com/Puma7/handy-meeting/actions/runs/40300203567), [all three portable GUI packages](https://github.com/Puma7/handy-meeting/actions/runs/40300203573), [preserved Windows checks](https://github.com/Puma7/handy-meeting/actions/runs/40300203576), and [release-workflow validation](https://github.com/Puma7/handy-meeting/actions/runs/40300203625). All five downloaded PR packages passed joint assembly against fingerprint `2650f6dce4914d7c8bb4dbede9a12866c0bb4a05a3c9100840fdb7befdf92d31`, including exact final archive hashes/sizes, actual installer preservation evidence, and five real isolated native credential cases per target. [Local package-set proof](.build/release-pr-588fb54/RELEASE_MANIFEST.json). These are PR candidates, not a tagged release build or a published release. The hardware, full-app duration, original GPU warning and signing gates below remain open.

The Rust/Tauri application, bilingual illustrated README and release tooling have been merged through [PR #8](https://github.com/Puma7/handy-meeting/pull/8). Application source `ac6323a42bbefa1586ef4242c79fcc0c56e49da5` passed actual [Windows/Core](https://github.com/Puma7/handy-meeting/actions/runs/40287668333), [Linux/Apple Silicon/Intel GUI packages](https://github.com/Puma7/handy-meeting/actions/runs/40287668343), and [preserved Windows checks](https://github.com/Puma7/handy-meeting/actions/runs/40287668354). The five downloaded packages were assembled and hash-checked together against fingerprint `db86c4ad3734bc23c7feb6448137c842ce1eaff0756d25b92d87454d898b10ce`.

Actual Windows installation/uninstallation, rejection of an existing destination and duplicate version, preservation of an unknown file, and five isolated native credential cases on all four targets passed. Linux installed/executed the DEB in Debian 12/13 containers. These are headless and synthetic checks, not device recording. A final release-workflow adjustment serializes tag and release events for the same version. Its tagged end-to-end run is separate evidence and will be linked after completion.

The release workflow builds Windows Setup/ZIP, an Ubuntu/Debian DEB and two Mac DMGs. A tag creates a complete draft after successful checks; publishing that verified draft selects Latest. [Release instructions](docs/RELEASING.md). No public 1.0.0 Latest release is claimed by this preparation status.

The previous beta.4 Windows CI found an unsynchronized synthetic test helper: process existence was mistaken for fixture readiness. The test now waits for the helper's actual readiness marker, with a separate startup deadline and unchanged complete-PCM requirement. Production recording logic was not changed. Real Mac/Linux capture, a complete Rust two-hour capture/recovery run, the original GPU-memory warning, trusted Windows signing and Mac notarization remain open. Actual evidence is maintained in [TEST_REPORT.md](TEST_REPORT.md).

## Earlier Rust/Tauri beta 3

`1.0.0-beta.3` implements the issue work: revisioned template management, opt-in native credential storage and visible per-session agent consent. Source commit `207678a41084edcba9816d79f4ef35bae4a524ac` passed the actual [Windows/Core checks](https://github.com/Puma7/handy-meeting/actions/runs/40239590566), [Linux and both Mac desktop packages](https://github.com/Puma7/handy-meeting/actions/runs/40239590551), and [preserved C# checks](https://github.com/Puma7/handy-meeting/actions/runs/40239590550). All four targets passed five actual isolated native credential-store checks. Hash/source bindings were verified after download. [Issue implementation PR](https://github.com/Puma7/handy-meeting/pull/8) remains a draft.

| Target | Checked beta.3 candidate |
| --- | --- |
| Windows x64 | [ZIP](packages/rust-beta3-ci-207678a/windows-x64/HandyMeeting-1.0.0-beta.3-windows-x64.zip) |
| Ubuntu/Debian x64 | [DEB](packages/rust-beta3-ci-207678a/linux-x64/HandyMeeting-1.0.0-beta.3-linux-amd64.deb) |
| macOS 13+ Apple Silicon | [DMG](packages/rust-beta3-ci-207678a/osx-arm64/HandyMeeting-1.0.0-beta.3-osx-arm64.dmg) |
| macOS 13+ Intel | [DMG](packages/rust-beta3-ci-207678a/osx-x64/HandyMeeting-1.0.0-beta.3-osx-x64.dmg) |

Windows native UI and pending-agent/EOF behavior were checked in isolated profiles with synthetic data. Human recording approval and physical Mac/Linux capture remain open. The completed paired worker-only GPU experiment is reported separately in [TEST_REPORT.md](TEST_REPORT.md); it does not replace a full-app two-hour test or resolve the original warning. Mac packages remain ad-hoc signed, not notarized. Existing distributions and user data remain intact.

## Earlier Rust/Tauri beta 2

The guided analysis, local-model discovery, meeting-detail history, truthful task progress/elapsed time/cancellation and validated Markdown copy are implemented in `1.0.0-beta.2`. [Draft PR #8](https://github.com/Puma7/handy-meeting/pull/8) remains open; no stable Rust release or merge is implied.

Source commit `9887a875a68e99530b34e834bf93194df65bb790` passed the actual [Windows Rust checks and packaging](https://github.com/Puma7/handy-meeting/actions/runs/40234784590), [Linux/Apple Silicon/Intel GUI builds and packages](https://github.com/Puma7/handy-meeting/actions/runs/40234784600), and [preserved C# checks](https://github.com/Puma7/handy-meeting/actions/runs/40234784610). Local verification passed 77 Rust, 33 frontend, nine artifact and 89 preserved C# cases, plus monitor checks; eight portable filesystem cases also passed. A real local Gemma analysis passed positive and nonbinding reference cases.

Locally downloaded candidates with verified SHA-256 and matching source fingerprints:

| Target | Candidate |
| --- | --- |
| Windows x64 | [ZIP](packages/rust-beta2-ci-9887a87/windows-x64/HandyMeeting-1.0.0-beta.2-windows-x64.zip) |
| Ubuntu/Debian x64 | [DEB](packages/rust-beta2-ci-9887a87/linux-x64/HandyMeeting-1.0.0-beta.2-linux-amd64.deb) |
| macOS 13+ Apple Silicon | [DMG](packages/rust-beta2-ci-9887a87/osx-arm64/HandyMeeting-1.0.0-beta.2-osx-arm64.dmg) |
| macOS 13+ Intel | [DMG](packages/rust-beta2-ci-9887a87/osx-x64/HandyMeeting-1.0.0-beta.2-osx-x64.dmg) |

These candidates are separate from existing local distributions and the older public releases below. Debian 12/13 installation and native headless checks passed; the Mac signatures and DMGs passed integrity checks. macOS packages are ad-hoc signed, not notarized. No CI job captured real devices or exercised a visible desktop. Windows driver discontinuities remain, and a new Rust two-hour run plus real Mac/Linux device tests are still required. See [TEST_REPORT.md](TEST_REPORT.md) for exact evidence and limits.

## Historical C# GitHub runs

| Check | Result | Evidence |
| --- | --- | --- |
| Windows verification | Success: 86 named checks plus monitor status-file checks | [Run 40211743753](https://github.com/Puma7/handy-meeting/actions/runs/40211743753) |
| Linux package and synthetic core checks | Success on Ubuntu 24.04 | [Portable run 40211949623](https://github.com/Puma7/handy-meeting/actions/runs/40211949623) |
| Apple Silicon helper, DMG, signature and synthetic checks | Success on macOS 15 arm64 | Same portable run |
| Intel Mac helper, DMG, signature and synthetic checks | Success on macOS 15 x64 | Same portable run |

These results apply to release commit `cbc0f1e358b51ee3c568ad3f319d8fc02281ca07`, including the explicit macOS-15 minimum and application icon. [Publishing run 40212183105](https://github.com/Puma7/handy-meeting/actions/runs/40212183105) repeated Windows verification, checked the packaged program against the verified build, accepted portable artifacts only from that exact commit, and published both prereleases. Both release tags resolve to that commit. See the current [Actions runs](https://github.com/Puma7/handy-meeting/actions).

The first private Windows attempt was not started because of the account's Actions budget. After approved public publication, the standard runners executed. The first Mac attempt found an unsigned managed assembly in the app bundle. The corrected script signs the complete .NET payload before the apphost and bundle and explicitly verifies that assembly. Both architectures then passed.

## Downloads

The following prototype downloads were published on 25 October 2026. They are prereleases, not stable production releases. The manually invoked download-publishing workflow only accepts successful portable artifacts from its exact source commit and never overwrites already published assets.

| Package | Download | Bytes |
| --- | --- | ---: |
| Windows x64 0.4.1 | [ZIP](https://github.com/Puma7/handy-meeting/releases/download/v0.4.1/HandyMeeting-0.4.1-windows-x64.zip) | 19,096,763 |
| Mac Apple Silicon 0.5.0-preview.1 | [DMG](https://github.com/Puma7/handy-meeting/releases/download/v0.5.0-preview.1/HandyMeeting-0.5.0-preview.1-osx-arm64.dmg) | 54,386,159 |
| Mac Intel 0.5.0-preview.1 | [DMG](https://github.com/Puma7/handy-meeting/releases/download/v0.5.0-preview.1/HandyMeeting-0.5.0-preview.1-osx-x64.dmg) | 55,192,533 |
| Ubuntu/Debian x64 0.5.0-preview.1 | [DEB](https://github.com/Puma7/handy-meeting/releases/download/v0.5.0-preview.1/HandyMeeting-0.5.0-preview.1-linux-amd64.deb) | 44,704,698 |

Checksums: [Windows SHA-256](https://github.com/Puma7/handy-meeting/releases/download/v0.4.1/HandyMeeting-0.4.1-windows-x64.zip.sha256), [portable SHA-256 manifest](https://github.com/Puma7/handy-meeting/releases/download/v0.5.0-preview.1/SHA256SUMS.txt). GitHub reports all six assets uploaded; release metadata records matching artifact SHA-256 digests. Model weights and personal sessions are absent. Dependencies, bundled upstream notices and locked native downloads were reviewed before publication. The test Windows ZIP is extracted alongside existing versions; portable preview storage is separate from the installed application. Problems are reported through Issues, with private security reporting available for sensitive findings.

Mac packages are architecture-specific, require macOS 15+, and are ad-hoc signed rather than Developer-ID signed/notarized. The Linux `.deb` requires the declared local runtime/graphics/audio-service dependencies. Model weights and personal session data are excluded.

## What is not established

CI does not open real microphones or capture a real meeting. Native GUI behavior, device loss and two-hour capture/recovery tests on the portable targets remain outstanding. Neither a successful package build nor synthetic recovery proves reliability on a tester's hardware. The old Windows GPU-memory trend warning is still open in [issue #7](https://github.com/Puma7/handy-meeting/issues/7).

Use disposable test meetings and review AI notes against their original quotes. See [TEST_REPORT.md](TEST_REPORT.md) and [PORTABILITY.md](PORTABILITY.md) for the full scope and limits.

## Download follow-up merged

[PR #11](https://github.com/Puma7/handy-meeting/pull/11) was merged as `1985750a0a1d39f44e92375469208fc27309a6e4` after two clean completed Codex code reviews, a clean completed security review on exact head `cd4f9e8`, and successful required Windows/release-validation CI. Public 1.0.0 assets were read back unchanged after both merges. This local final ledger entry does not create a new release. The separate PR #9 recovery fixes and its unresolved current-head security receipt remain pending for a future version; the conditional Handy discussion announcement was not posted.

1.0.1 local verification completed: 114 Rust checks, 55 UI checks, 20 release guards, 11 portable guards, nine artifact guards, three actual Win32 reservation cases, 89 preserved C# checks and the monitor check passed. Warnings were denied. Actual local Gemma reference completed in 43.102 seconds: one explicit decision/task with literal Anna/Freitag, no decision/task for nonbinding statements, two confirmed steps reused after controlled interruption; no process crash or real cloud test. An initial local Rust invocation ran before frontend assets existed; its failed event is retained, and sequential compilation then passed. Target installer/package CI is still pending.

Current locked dependency audit (26 October 2026): zero vulnerability findings, 9 maintenance warnings and 1 existing unsoundness warning. RustSec database a78282a1414d6c28cc228c4eae3570451affb0dc with 1378 advisories. This is not a vulnerability-free guarantee; the existing GLib warning is retained. [Exact current lock audit](docs/rust-1.0.1-dependency-audit.json).

PR #12 first code-review round found a confirmed P2: failed unfinished-analysis discovery prevented valid completed analyses from loading. Two UI regressions reproduced both directions before the fix and passed after independent guarded result handlers were introduced. Discovery errors stay visible and retained files are not repaired/deleted. This change is being verified on a new source head; the initial 5248481 review is not a clean release approval.


## 1.0.1 reviewed merge and exact-tag build

PR #12 merged as `fae73a06e3a83a7fcad17ea329e1f94fab53e02d`; its tree exactly matches reviewed `cc5bfed`. Current-head required CI is green, full 57-file Codex Security diff scan completed with no findings, and all five PR packages plus twelve proof files passed local assembly. See the appended 1.0.1 evidence in TEST_REPORT.md. The previously empty wrong-version release is now a draft, its tag was advanced using an exact old-OID lease without force, and actual [tag run 40402428229](https://github.com/Puma7/handy-meeting/actions/runs/40402428229) passed validation but is still building. No finished tag build/publication is claimed. Original 1.0.0 assets and all user data remain preserved. Historical device/endurance/GPU/signing/dependency gates still apply.


The exact1.0.1 package matrix and bundle assembly passed in run40402428229, but its attachment step failed because published-tag lookup excludes drafts. The preserved17-file bundle independently reassembled byte-for-byte. PR13 fixes draft discovery and exact-ID transfer; two clean code rounds and current Security completed on961ffbf, required targetCI still running. An authorized draft-only recovery attachment to original release424691025 is in progress; no completed upload or public release is claimed. See state.json before retrying any operation.


PR13 merged as `c4f6f6b854685802f196ec2949cf0abba1aee603` after both clean code reviews, current Security and all required CI passed. Its tree matches reviewed961ffbf. The frozen1.0.1 tag and program assets remain unchanged; one draft-only upload is still running. No finished public release is claimed.

## 1.0.1 draft attachment verified — 2026-10-27 UTC

The single private recovery upload exited successfully and original [draft424691025](https://github.com/Puma7/handy-meeting/releases/tag/untagged-753b28b38a21de9cfe03) now contains all17 files: five platform packages and twelve hash/source proofs. Every asset ID, exact filename, size and GitHub SHA256 matches the retained exact-tag bundle and independently reassembled bytes. Tagv1.0.1 still points to `fae73a06e3a83a7fcad17ea329e1f94fab53e02d`; the common source fingerprint is `43661b8615455fb72ef52172dbe20988999fb033d95d9a597ea9054483c68eea`. Actual Windows install/uninstall preservation with missing parents and all four actual native credential targets are verified. Receipt: `.build/release-1.0.1/draft-verified-20261027T0145.json`.

The original [tag run40402428229](https://github.com/Puma7/handy-meeting/actions/runs/40402428229) remains failed overall: package matrix passed, its original attachment step failed; the corrected actual attachment after merged [PR13](https://github.com/Puma7/handy-meeting/pull/13) is separate successful evidence. No retag, replacement bytes or overwritten assets. Empty bot draft424794429 remains unchanged. Public release/Latest and the authorized discussion comment await explicit acceptance of every named outstanding device, endurance, GPU, signing and dependency limit under docs/RELEASING.md.

## 1.0.1 public release complete — 2026-10-27 UTC

The maintainer explicitly accepted every named device, visible-UI, Rust two-hour, original GPU, signing/notarization and GLib/maintenance limitation in this chat. Exact answer/time are retained in state.json. These remain limitations, not newly satisfied tests. The same original [release424691025 / v1.0.1](https://github.com/Puma7/handy-meeting/releases/tag/v1.0.1) is now public (draft=false, published_at2026-10-27T05:35:54Z) and Latest. All17 original asset IDs, names, sizes and SHA256 values, the fae73a06e3a83a7fcad17ea329e1f94fab53e02d tag and source fingerprint43661b8615455fb72ef52172dbe20988999fb033d95d9a597ea9054483c68eea were rechecked unchanged. No rebuilt/replaced bytes, retag or second upload. Empty bot draft424794429 and older releases remain preserved.

The actual post-publication [run40456228240](https://github.com/Puma7/handy-meeting/actions/runs/40456228240) completed successfully: validate and publish_existing passed, rebuild/upload jobs skipped. Original tag run40402428229 remains failed overall; its package-matrix result and the corrected actual attachment remain separate evidence. PR12 and PR13 are already merged. After reading all discussion comments/replies and finding no duplicate, the single authorized [English Puma7 comment](https://github.com/cjpais/Handy/discussions/1923#discussioncomment-19210789) was posted and exactly read back, created2026-10-27T05:37:33Z. Final receipt: .build/release-1.0.1/final-publication-and-workflow-20261027.json; full comment ID/text/time/URL and accepted limitations are in the ledger. No user data or older evidence was deleted.

Completion cleanup confirmed: only automation handymeeting-1-0-1-release-fertig-pr-fen was deleted through the app; deletion receipt and timestamp retained. Release phase: complete.

## Analysis status bar — 2026-10-27, unreleased source

The meeting-analysis entry previously opened setup without submitting a model request. Setup now explicitly says analysis has not started and offers a guarded Start analysis action. The job panel occupies its own bottom window row, outside scrolling content, with spinner, elapsed seconds/MM:SS, confirmed-task progress/percentage, current step, cancellation and retained terminal feedback. Terminal time no longer advances when the backend's general busy flag lags. A backend job without readable progress is shown as status-checking rather than idle. Cloud consent still gates each run and includes the footer in the inert background.

Verification:3targeted pre-fix failures reproduced;5new cases pass and all62frontend tests pass. TypeScript, Vite production build and Prettier passed. Required ./verify.ps1 passed18PCM/35Notes/28Templates/8Agent+Monitor using its separate program folder. Edge/Playwright rendered tests at1280x850 and390x844 used synthetic IPC only: setup, explicit start,33% confirmed progress, scrolling/page changes,100% completion and failure with a stopped clock, no spinner or console errors. Evidence: .build/notes-statusbar-20261027/ and the external statusbar-ui-verification.json/screenshot set referenced in TEST_REPORT.md. No real provider/device/endurance claim. No dependency/version changes or public release; installed1.0.1 remains unchanged. Existing user data and release evidence preserved.


## 1.0.2 full-input development verification (27 October 2026)

The coordinated Cargo/frontend/Tauri version is now 1.0.2. The complete input, including rename/VIP/search, readable text/recovery, stable installer naming, explicit updates, three separate speaker controls, retention previews, archives, optional compact audio and a new transcription copy, is represented in this development candidate. See docs/1.0.2-USER-GUIDE.md. No 1.0.2 installer, PR, completed CI or public release is claimed at this checkpoint.

Executed local checks: the full Rust workspace suite passed 143 tests (13 bridge unit,14 bridge integration,31 desktop unit,3 agent integration,41 core unit,16 durability,14 notes unit,11 notes integration); two child-process fixtures are intentionally ignored as standalone fixtures and are exercised by parent tests. Clippy and formatting passed with warnings denied. The complete UI suite passed76 tests before the two focused privacy/navigation additions; those two files then passed19 targeted tests. TypeScript and Vite production build passed. ./verify.ps1 passed18 PCM,35 Notes,28 Templates,8 Agent and monitor checks in a separate program folder. Package/publication guards passed29 cases, portable guards11, filesystem artifact guards9. Simulated publisher outputs are not actual uploads.

Archive regressions include exact PCM/text/unknown ordinary file and annotation revision preservation; no overwrite of an existing destination; new files, changed files, active leases, VIP and cancellation; malicious ZIP paths; provenance of readable audio-archived text; grouped explicit permanent deletion; interrupted raw-copy compaction followed by verification and exact restoration. Windows relocation uses an external lease while preserving compatibility with the previous internal session lock; both new writers and an actual legacy-style writer are covered. Index pruning removes only derived rows whose native source folders are absent and does not remove original files. Rules are disabled initially. No user meeting was archived, compacted, converted or deleted by these checks.

Actual local model test: docs/1.0.2-local-name-reference.json. CPU-only owned Gemma4E4B alias,60.769105seconds, invented self-introduction/commitment, hypothetical addressing and nameless noncommittal text. Quoted candidates Anna/Peter/Anna; zero automatic speaker assignments. The owned alias was unloaded afterwards. An earlier test mistakenly used the server root without /v1 and failed; its evidence remains retained. The application default already uses /v1. No cloud provider, real user transcript or audio speaker-quality result is claimed.

Actual FFmpeg8.1.2 check: docs/1.0.2-audio-codec-reference.json. Three seconds of invented tones, FLAC exact decoded PCM, Opus decodable, saved-copy reimport and rejection of a changed source/copy. Original WAV retained. This checks codec/provenance/duration, not speech quality or ASR accuracy. The new app path creates a separate model-pinned session; a full real-model compact-copy transcription remains to be checked.

Synthetic100000-meeting benchmark: docs/1.0.2-search-benchmark.json, Windows, Intel i7-13700/24logical processors, debug Rust, bundled SQLite/rusqlite0.41.0, one text section per invented manifest and no audio. Index build95695ms; reopened query1.982ms;100warm queries median0.244ms/p950.424ms; last history page13.736ms; broad100000-match search218.625ms. No retrospective acceptance threshold or OS cold-cache claim. Working Set, Private Bytes and graphics memory were not measured for this rerun. Codex/source editing and local development checks were concurrent; no foreign processes were stopped.

Rendered Edge/Playwright checks at1280x900 and390x844 used synthetic IPC: rules off, save separated from preview, review plus exact deletion text, visible terminal error with no spinner, restore confirmation, compact-copy model/confirmation, no horizontal overflow or console/page errors. Evidence full-input-102-ui-verification.json and screenshots are retained in the task visualization directory. This is not native hardware UI evidence. Later raw-copy compaction controls still require the final rendered rerun.

Actual Windows audio-only diagnostic lasted15.369seconds: system15184ms, microphone15002ms, difference182ms. The microphone driver reported TWO discontinuities (first packet and later in the run); captureIncomplete=true and the warning were retained. Therefore this is NOT a clean hardware capture pass. Recovery preserved both confirmed checkpoint prefixes; no power-loss, speech recognition or two-hour claim. docs/1.0.2-windows-short-capture.json records the limitation. Read-only installed integration checks passed8 cases with9audio devices,85catalog entries/1downloaded streaming model and the pinned native0.4.1 ABI; these did not open devices.

Locked dependency audit: docs/rust-1.0.2-dependency-audit.json,566packages, RustSec database312a9c703c74985e42b6f19caa6c539e75513370/1379advisories, zero vulnerability findings, nine maintenance warnings and the existing GLib unsoundness warning. Original licenses were collected for Windows335packages, Linux414, Mac ARM322 and Intel323 plus7production frontend packages; no third-party version was upgraded. Separate code/security reviews, real target installer/package CI, compatible-model speaker validation, full-app endurance, original GPU trend and signing/notarization remain outstanding. Old1.0.1 release/assets, earlier programs, user data and all prior evidence remain preserved.


Final local UI follow-up before review export: all78 tests in11files passed together; TypeScript passed. The rendered Edge check was repeated successfully with the raw-copy compaction confirmation included, still with synthetic IPC only. The earlier failed new-test selectors/type declarations were corrected in the test fixture; their logs remain retained. No additional hardware or speaker-quality claim.
