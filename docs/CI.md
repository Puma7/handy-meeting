# CI scope and resource use

Source checks and complete downloadable candidates serve different purposes. A green source-only run does **not** certify an installer, native keychain, GUI package or real recording device.

| Event | Source checks | Complete Windows/Linux/Mac candidates | Release action |
| --- | --- | --- | --- |
| Relevant draft PR update | Windows legacy/UI/Rust/guards, portable core on three targets, dependency audit | Omitted | None |
| PR marked ready, or relevant update while ready | Windows legacy/UI/Rust/guards and dependency audit; portable full workspace | All four target hosts, five packages, installer/native-store/GUI/relocation checks | None |
| Relevant main push | Same source-only checks as a draft | Omitted | None |
| Manual candidate workflow | Full by default; `packages=false` selects source-only Windows checks | Only the explicitly selected Windows or portable workflow | None |
| Release tag (sets the version) | Reusable complete verification | All five packages once for that release attempt | Assemble and populate a draft |
| Publish an already complete draft | Recheck exact commit, fingerprint, 17 assets and hashes | Omitted; existing bytes retained | Promote/Latest for final versions |
| Publish an incomplete draft | Actionable failure | Never fall back to recompiling | Preserve assets; recover the tag run |

The ready-for-review transition is subscribed explicitly; it does not require an extra code push. Converting a PR back to draft also refreshes its scope and cancels obsolete PR work. Reusable release jobs receive `packages=true` explicitly. Full package mode retains every installer, credential, ABI, frontend, Rust, Linux virtual-audio and relocation check. The smaller no-ASR core matrix is omitted only when that same run already executes both no-ASR test/Clippy commands and the full native matrix in the same target checkout. No physical-device or endurance evidence is implied.

Both candidate workflows use the same PR path filters. Legacy-only `src/**`/`verify.ps1`, portable-only `platform/**`, shared SDK settings and changes to either candidate workflow therefore trigger the complete ready-PR pair. Drafts still select source-only mode.

The **Windows checks** workflow preserves the existing required `verify` context on every PR and remains manually available. Its additional branch/tag push trigger is removed: a same-repository PR update no longer runs it twice. The Rust workflow retains its own source-bound `./verify.ps1` result for candidate proofs. This small required PR check also runs on documentation changes; branch protection is not weakened to save runtime.

## Avoiding duplicate packages

Development source checks do not perform the optimized release compile or upload candidate archives. Ready PRs deliberately build reviewable candidates; the exact release tag later builds the reviewed release independently. PR merge-ref artifacts are not substituted for a tagged commit. Main no longer creates another identical platform package set between those two stages.

The tag workflow and publication verifier share a per-tag concurrency group and never cancel an active release operation. Publication is verification/promotion only, even if the earlier tag build failed. A missing bundle is an error, not permission to build the same version again. For interrupted publication, use **Re-run failed jobs** with the retained `verified-release-bundle`; inspect conflicts rather than overwrite different bytes. An intentional **Re-run all jobs** or manual request can still rebuild: that is an explicit operator action, not an automatic retry policy.

PR updates cancel obsolete checks. Manual candidate and release work is allowed to finish so that an incidental PR update cannot discard a release attempt. Separate workflow concurrency groups prevent unrelated workflows from cancelling each other.

## Storage and safety

Only complete candidate runs upload program archives. Source-only Windows runs retain their small verification receipts; portable draft runs report their scope and create no archives. Existing candidate/proof retention stays 14 days, complete release bundles 30 days. This change neither deletes existing artifacts nor touches releases, tags, recordings, models, keys or local proof folders.

No broad compiler/output cache is introduced. In particular, no private build directory, session file, model, credential or opaque PR-produced executable is promoted into a trusted release cache. Package locks, reviewed native SDK hashes, compiler warnings as errors, source fingerprints, exact asset lists and no-overwrite rules remain enforced. The dependency audit still reads the current RustSec database on every applicable run.

## Measured baseline, 28 October 2026

The GitHub Jobs API confirmed three automatic package-build attempts for the same v1.0.4 source commit `c8cd6092ef45a6a756458c627586a8ad6c0df1f1`:

| Trigger | Runs | Approximate summed job elapsed minutes | Result |
| --- | --- | --- | --- |
| Main push | [Windows/core](https://github.com/Puma7/meeting-for-handy/actions/runs/40637653201), [portable](https://github.com/Puma7/meeting-for-handy/actions/runs/40637653234) | 78 | Success |
| Tag push | [Downloads](https://github.com/Puma7/meeting-for-handy/actions/runs/40640382683) | 62 | Failed on Intel Mac; three target builds succeeded |
| Release published | [Downloads](https://github.com/Puma7/meeting-for-handy/actions/runs/40640382852) | 80 | Rebuilt all four targets; success |

Two [v1.0.3 tag](https://github.com/Puma7/meeting-for-handy/actions/runs/40637124498)/[publication](https://github.com/Puma7/meeting-for-handy/actions/runs/40637124879) attempts also repeated failing checks. They remain failed historical evidence.

For the later draft head `313b49a`, [Windows/core](https://github.com/Puma7/meeting-for-handy/actions/runs/40655227012) plus [portable](https://github.com/Puma7/meeting-for-handy/actions/runs/40655227059) consumed about 77 summed job elapsed minutes. These figures sum each job's `completed_at - started_at`; parallel jobs are counted separately. They are **not** wall-clock duration, billed minutes or a cost estimate. New routing savings must be measured from actual new runs; these historical numbers are not a promised speedup.

References: [GitHub PR event types](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#pull_request), [workflow concurrency](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency), [cache scope](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching).
