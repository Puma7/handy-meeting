# Build and release status

## Exact-tag package builds passed; draft attachment repair

Tag run40402428229 built and checked the entire Windows/Linux/Apple-Silicon/Intel matrix and retained its verified 17-file bundle as artifact12691197240. Only the subsequent attachment step failed: GitHub's published-tag endpoint does not locate draft releases. The corrected publisher discovers paginated drafts and binds uploads/readback/promotion to exact IDs, rejecting ambiguous duplicate drafts. Nine new simulated regressions and the existing20 release guards pass. Original tag package bytes and both empty drafts are retained; no completed upload/publication is claimed here.

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
