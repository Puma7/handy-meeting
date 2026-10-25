# Build and prototype status

Repository: [Puma7/handy-meeting](https://github.com/Puma7/handy-meeting), publicly shared with the owner's approval on 25 October 2026. This is an independent prototype.

## Confirmed GitHub runs

| Check | Result | Evidence |
| --- | --- | --- |
| Windows verification | Success: 86 named checks plus monitor status-file checks | [Run 40210563368](https://github.com/Puma7/handy-meeting/actions/runs/40210563368) |
| Linux package and synthetic core checks | Success on Ubuntu 24.04 | [Portable run 40210637828](https://github.com/Puma7/handy-meeting/actions/runs/40210637828) |
| Apple Silicon helper, DMG, signature and synthetic checks | Success on macOS 15 arm64 | Same portable run |
| Intel Mac helper, DMG, signature and synthetic checks | Success on macOS 15 x64 | Same portable run |

These results apply to commit `60335cd0c26aca341a4013dac21bcf5423f95ffe`. Subsequent packaging refinements, including the explicit macOS-15 minimum and application icon, receive a new build before publication. See the current [Actions runs](https://github.com/Puma7/handy-meeting/actions).

The first private Windows attempt was not started because of the account's Actions budget. After approved public publication, the standard runners executed. The first Mac attempt found an unsigned managed assembly in the app bundle. The corrected script signs the complete .NET payload before the apphost and bundle and explicitly verifies that assembly. Both architectures then passed.

## Downloads

Published prototype downloads are listed in [Releases](https://github.com/Puma7/handy-meeting/releases). A draft or an Actions artifact is not a stable release. Windows is version 0.4.1; portable packages use 0.5.0-preview.1. The manually invoked download-publishing workflow only accepts successful portable artifacts from its exact source commit and never overwrites already published assets.

Mac packages are architecture-specific, require macOS 15+, and are ad-hoc signed rather than Developer-ID signed/notarized. The Linux `.deb` requires the declared local runtime/graphics/audio-service dependencies. Model weights and personal session data are excluded.

## What is not established

CI does not open real microphones or capture a real meeting. Native GUI behavior, device loss and two-hour capture/recovery tests on the portable targets remain outstanding. Neither a successful package build nor synthetic recovery proves reliability on a tester's hardware. The old Windows GPU-memory trend warning is still open in [issue #7](https://github.com/Puma7/handy-meeting/issues/7).

Use disposable test meetings and review AI notes against their original quotes. See [TEST_REPORT.md](TEST_REPORT.md) and [PORTABILITY.md](PORTABILITY.md) for the full scope and limits.
