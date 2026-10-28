# Releases: version tag to downloadable packages

The Rust distribution is versioned together in the workspace Cargo manifest, desktop package/lock and Tauri configuration. Existing native dependency versions and model weights are not changed by a release. **[1.0.4 is public](https://github.com/Puma7/meeting-for-handy/releases/tag/v1.0.4)** as of 28 October 2026, with five packages and twelve checksum/source proofs (17 assets). The [audit corrections](AUDIT_FIXES_20261028.md) are subsequent unreleased source work; the synchronized 1.0.5 release candidate and the 1.0.6 development source (live summary) have no release or tag yet. Future releases must use a new version: update the Cargo workspace, desktop package/lock and Tauri configuration together, then tag that exact reviewed commit. Existing published tags and asset bytes are preserved.

## What is attached

These are the current 1.0.4 asset names. A future release uses its own synchronized application version in every filename and proof; do not reuse or move the published 1.0.4 tag.

| Target | Asset |
| --- | --- |
| Windows x64 installer | `HandyMeeting-1.0.4-windows-x64-setup.exe` |
| Windows x64 portable | `HandyMeeting-1.0.4-windows-x64.zip` |
| Ubuntu/Debian x64 | `HandyMeeting-1.0.4-linux-amd64.deb` |
| macOS 15+ Apple Silicon | `HandyMeeting-1.0.4-osx-arm64.dmg` |
| macOS 15+ Intel | `HandyMeeting-1.0.4-osx-x64.dmg` |

Each has a SHA-256 sidecar and package proof. `SHA256SUMS.txt` covers all five packages. `RELEASE_MANIFEST.json` binds their bytes to the application version, exact Git commit and common source fingerprint. No audio, model, session, setting or key is packaged. The workflow publishes no bare Windows EXE requiring users to reconstruct its native libraries: the Setup EXE and ZIP contain the complete checked payload.

## Each release

1. Finish/review the implementation PR and require its Windows, Linux and both Mac checks to pass. Update `TEST_REPORT.md` with actual results; do not turn a prepared workflow into a test claim.
2. Check the manual release gates below. The product's version number does not prove capture reliability.
3. Merge the verified source into `main`, then create/push **the matching `v<application-version>` tag on that commit**. A tag alone triggers **Build and publish downloads**, which runs the existing verified Windows and portable workflows as reusable jobs.
4. The workflow builds all five assets, including a real Windows install/uninstall preservation check in its disposable CI account. Only after the entire matrix passes does it assemble/upload the bundle into a **GitHub release draft**.
5. Review the populated draft and its notes, then choose **Publish release** when ready. The `published` event verifies the already complete exact-source bundle without recompiling different binaries, and marks a final semantic version as **Latest**. Tags containing a suffix remain prereleases and are not made Latest.

Publishing a release directly in GitHub triggers verification and promotion only. It never starts a replacement build. If the exact-source draft is incomplete, the run fails with recovery instructions and preserves every existing asset. Create the tag first and wait for its populated draft; publishing an empty page is not a build request.

The manual **Run workflow** form accepts the exact existing tag and a `publish` switch. Default `false` keeps a draft. Select the tag's commit (or `main` only when it is still that exact commit). Setting `publish=true` publishes a fully verified bundle and sets Latest for a final version. This is an operator publication action, not a substitute for the manual gates.

## Manual release gates and current limits

- Actual system/microphone capture, denied permissions, device loss, storage errors, owned process crashes and long-session recovery on each supported target must be evidenced separately from synthetic CI.
- A new full-app Rust two-hour test is still outstanding. The original C# two-hour run and the shorter paired native-worker experiment remain historical evidence with their original limits and unresolved memory warning.
- Real Mac/Linux visible UI and device validation remain open. Package integrity, target-host execution and native keychains have separate automated evidence.
- Current Mac packages are **ad-hoc signed, not Developer-ID signed or Apple-notarized**. Current Windows setup is **unsigned**. No signing identities were available during this preparation. Do not claim notarization or instruct users to disable system protection. Signing requires the maintainer's own valid credentials; never put them in a repository, release file or ordinary workflow log.
- Check known dependency advisories and retained licenses. Existing GLib/maintenance notices are not erased by a successful build.

Until these gates have evidence or an explicit, recorded maintainer decision accepting named limitations, retain a release draft. A draft can already contain all reviewable packages; it is not a public Latest release.

## Installation and upgrades

Windows setup (1.0.6 and later) installs for the current user into one folder, `%LOCALAPPDATA%\Programs\Meeting for Handy`, registered as `MeetingForHandy`. It offers English or German and creates the Start menu link natively (no PowerShell, so a Restricted execution policy cannot block it); a desktop link is optional on the finish page (silent setups: `/DESKTOPSHORTCUT`). The setup records the desktop link it created; it never replaces or claims an existing desktop link of the same name. An existing Start menu link is replaced only when it starts this or the previously registered program; one that starts another program is kept and reported. Uninstall removes the Start menu link only when it starts this folder's program, and the desktop link only when this setup recorded creating it and it still starts that program. A new destination must not exist yet and is reserved exclusively before extraction; folders with other files are refused. Running setup again on the registered folder repairs or updates it: setup waits up to 15 seconds for the app to close, never ends it, and the installed uninstaller (`/UPGRADE`) removes only its listed program files before the new ones are written. Installations of the former version-folder layout (`HandyMeeting-<version>` registrations) and a previous registered folder are removed through their own uninstallers after the new files are in place; their user files remain. An installation whose app is still running after the same 15-second wait is kept and named in the setup log: a former version folder stays listed in Windows Settings > Apps, while a previous registered folder loses its entry to the new registration and is removed later with its own `Uninstall.exe`. It never stops Handy or an existing recording. Uninstall removes shared registration only when its directory still belongs to that installation. It deletes literal listed program files and nonempty folders remain; it never recursively removes user data. WebView2 and Microsoft Visual C++ x64 runtime may need to be installed separately. The ZIP is the portable alternative. The compiled installer/stub license is retained in `licenses/installer/NSIS-3.13-COPYING.txt`.

Linux uses package `handymeeting`, versioned program files under `/opt/handymeeting`, and the normal desktop launcher. Install the DEB through the package manager to resolve its declared libraries. Earlier `handymeeting-rust-beta` packages are preserved; remove them explicitly through the package manager if no longer wanted.

Mac users choose the DMG matching their processor, copy the app to Applications and grant the required recording permissions through macOS. The bundle identifier and existing local data location are retained for compatibility; the displayed app name is Meeting for Handy.

The local data location remains `HandyMeeting/RustBeta` below the platform's local application-data directory. This intentionally preserves earlier Rust history without copying or moving a potentially active session. Program updates do not silently switch model/runtime identity for an older unfinished recognition run.

## Failure and retry behavior

- Portable native SDK downloads retry transient connection/body failures and HTTP 408/429/500/502/503/504 at most three times, with one- and two-second delays. The same reviewed URL, 120-second per-attempt timeout, 256 MiB bound and fixed SHA-256 remain enforced. Certificate failures, other HTTP errors and checksum/size mismatches stop immediately. Incomplete attempts are never cached; different existing cache/SDK bytes are retained and rejected.
- Failed/missing checks, targets, checksums, source fingerprints, native-store proofs or installer preservation proofs prevent assembly before any upload.
- Publication uses only artifacts from the current successful reusable jobs and exact tag commit. Artifacts from a different commit/run are not substituted.
- Existing release assets are never overwritten. A conflicting asset aborts; the old bytes are preserved.
- Additional or duplicate remote release assets also block uploading/promotion. The workflow never silently publishes an unrelated attachment or removes it to make validation pass. The already-complete path checks the remote file list again immediately before promotion.
- The verified bundle is retained as an Actions artifact **before** uploads, so a network failure does not discard the reviewed package set. Use **Re-run failed jobs** to retry publishing the same build artifacts. Rebuilding the same version can legitimately produce different bytes and is not an overwrite permission.
- An already complete release is checked by commit, source fingerprint, expected asset names, sizes and SHA-256 values. Publishing the draft does not trigger a second compile or replace its assets.
- Draft discovery checks the paginated release list after a missing published-tag response. GitHub's tag endpoint returns published releases only; its 404 is not permission to create another draft blindly. Uploads, binary fallback downloads and promotion use explicit release/asset IDs.
- Multiple drafts for one tag stop automatic publication before any upload. Preserve them and inspect their IDs, authors, notes and assets. For an explicitly selected original draft, the maintainer can run the verified publisher with `--release-id <id>` and the retained exact-tag `verified-release-bundle`. The ID must still belong to that tag. Omit `--publish` to populate only the draft. Conflicting assets remain a hard failure; an upload failure does not justify rebuilding or replacing different bytes.
- Release assembly tests use fake package bytes and simulated GitHub calls. They are not a real GitHub upload or real provider test. Actual pipeline results are linked in `BUILD_STATUS.md` only after execution.

The development/candidate split, trigger matrix and measured duplicate runs are documented in [CI scope and resource use](CI.md).

Implementation: `.github/workflows/rust-release.yml`, `scripts/release-packages.py`, `scripts/publish-rust-release.py`. The older C# publishing workflow and old release assets remain historical and are not used for Rust 1.0.

References: [GitHub reusable workflows](https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows), [release workflow events](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#release), [GitHub CLI release creation](https://cli.github.com/manual/gh_release_create), [NSIS distribution and license](https://nsis.sourceforge.io/Download).


The physical-device, interruption and endurance limits of the 1.0.4 capture changes remain open for the public release and any follow-up. No Mac/Linux devices are available to the maintainer at this checkpoint. Publication does not turn those limits into passed tests; use the manual gates above for each new candidate. WSL, synthetic PCM/helper checks and virtual CI devices are not substitutes. Native file selections are confirmed through Rust-only dialogs and expire on app restart. The installer runs no PowerShell; disposable CI checks fresh install, migration of a synthetic former version folder, repair, a synthetically locked program file (opened without sharing, standing in for a running app; no real process or recording is started), own and foreign Start menu and desktop links, and uninstall. Internal package/executable/data IDs remain compatible with earlier update clients.
