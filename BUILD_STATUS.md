# Build and prototype status

Repository: [Puma7/handy-meeting](https://github.com/Puma7/handy-meeting), publicly shared with the owner's approval on 25 October 2026. This is an independent prototype.

## Current Rust/Tauri beta 2

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
