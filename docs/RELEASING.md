# Releasing

A release needs exactly one action: **create a GitHub release with a new version tag.** The version, the packages and the release notes follow from it automatically.

## Steps

1. Merge the reviewed changes into `main` (the PR's Windows, Linux and Mac checks must be green).
2. On GitHub open **Releases → Draft a new release**.
3. **Choose a tag** → type a new tag such as `v1.0.8` (or `v1.1.0-beta.1` for a prerelease) → *Create new tag on publish*, target `main`.
4. Click **Generate release notes**. GitHub lists the merged pull requests since the previous release; edit the text if you like. For a version with a suffix tick **Set as a pre-release**.
5. Click **Publish release**.

That is all. Publishing creates the tag, and the tag push starts **Build and publish downloads** (`.github/workflows/rust-release.yml`). The run started by the publication itself never builds: it finds the tag run of the same commit and leaves the packages to it.

- Every job first writes the tag's version into `Cargo.toml`, `Cargo.lock`, `apps/desktop/package.json`, `package-lock.json` and `tauri.conf.json` (`scripts/set-version.py`). **Nothing has to be bumped by hand**, and the version in the repository files does not have to match the tag.
- Windows (installer and ZIP), Linux (DEB) and macOS (two DMGs) are built and checked, including a real install/uninstall run on Windows.
- Only when the whole matrix passed are the five packages, their checksums and proofs attached to the release. This takes about 30–40 minutes; until then the release page shows no downloads.
- A final version (`v1.2.3`) becomes **Latest**; a version with a suffix (`v1.2.3-beta.1`) stays a prerelease. A release published on GitHub keeps the choice made there.

Release notes are the changelog: [CHANGELOG.md](../CHANGELOG.md) keeps the history up to 1.0.7 and points to the releases page for everything after.

### Alternatives

- **Tag only:** pushing a tag `vX.Y.Z` without creating a release builds everything and creates a **draft** release whose notes are generated the same way (merged pull requests, then the fixed download notes from `.github/RELEASE_NOTES.md`). Publish the draft when ready.
- **Manual run:** *Actions → Build and publish downloads → Run workflow* on the tag, with `publish` on or off, rebuilds a missing release or publishes a complete draft.

### If a build failed

Fix the cause on `main`, then delete the release **and** its tag on GitHub and create the release again with the same version. An existing asset is never replaced; a release that already has packages keeps them. If only the upload failed, use **Re-run failed jobs**: the verified bundle is kept as the `verified-release-bundle` artifact for 30 days.

## What is attached

| Target | Asset |
| --- | --- |
| Windows x64 installer | `HandyMeeting-<version>-windows-x64-setup.exe` |
| Windows x64 portable | `HandyMeeting-<version>-windows-x64.zip` |
| Ubuntu/Debian x64 | `HandyMeeting-<version>-linux-amd64.deb` |
| macOS 15+ Apple silicon | `HandyMeeting-<version>-osx-arm64.dmg` |
| macOS 15+ Intel | `HandyMeeting-<version>-osx-x64.dmg` |

Each package has a SHA-256 sidecar and a proof (`.verification.json` or `.manifest.json`). `SHA256SUMS.txt` covers all five packages; `RELEASE_MANIFEST.json` binds their bytes to the version, the exact commit and the source fingerprint. No audio, model, session, setting or key is packaged. The `HandyMeeting-` file prefix stays because updaters before 1.0.6 only look for it.

## Before you publish

CI runs the dependency audit (`cargo audit`) and checks the retained licenses. These gates are not covered by CI; record their evidence in [TEST_REPORT.md](../TEST_REPORT.md) or accept a named limit explicitly:

- Real system/microphone capture, denied permissions, device loss, storage errors and long-session recovery on each supported system.
- A new full-app two-hour run (still outstanding for the Rust app).
- Real Mac/Linux interface and device checks.
- Signing: macOS packages are ad-hoc signed and not notarized, the Windows setup is unsigned. Never put signing credentials into the repository or ordinary workflow logs.
- Advisories the audit reports and the decision to accept or fix them.

## Safety rules of the pipeline

- Packages are attached only from the current run and the exact tag commit; artifacts of another commit or run are never used.
- Missing or failed checks, targets, checksums, source fingerprints, credential-store or installer proofs stop the assembly before any upload.
- Existing assets are never overwritten or deleted; a different or unexpected asset stops the upload and promotion.
- Several drafts for one tag stop automatic publication. Download the run's `verified-release-bundle` artifact into a folder and select one draft explicitly with an authenticated `gh`: `python3 scripts/publish-rust-release.py --folder <bundle folder> --repository Puma7/meeting-for-handy --tag vX.Y.Z --commit <tag commit> --notes .github/RELEASE_NOTES.md --release-id <id>` (add `--publish` to publish it).
- Native SDK downloads retry transient failures at most three times; checksum, size and certificate failures stop at once.

## Installation and upgrades

**Windows:** the setup installs for the current user into `%LOCALAPPDATA%\Programs\Meeting for Handy` and adds a Start menu entry (no PowerShell needed); a desktop link is optional (`/DESKTOPSHORTCUT` for silent setups). Running a newer or the same setup again updates or repairs that folder after waiting up to 15 seconds for the app to close; it never ends the app or a recording and removes only its own listed program files. Version folders of 1.0.5 and earlier are removed through their own uninstallers; one whose app is still running is kept and can be removed later. Links are only replaced or removed when this setup owns them. Uninstall never deletes user data recursively.

**Linux:** package `handymeeting` with program files under `/opt/handymeeting`; install the DEB through the package manager.

**macOS:** copy the app from the DMG matching the processor into Applications and grant the recording permissions.

User data stays in `HandyMeeting/RustBeta` below the local application-data directory on every system; updates never move it.

Further reading: [CI scope and resource use](CI.md), `scripts/release-packages.py`, `scripts/publish-rust-release.py`.
