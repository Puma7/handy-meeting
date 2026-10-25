# Dependency and license evidence for the Rust beta

Status: 25 October 2026, `1.0.0-beta.1`, Windows x64. This records supplied license texts and dependency checks; it is not a claim of general correctness, complete security or portable-device reliability.

## Scope and reproduction

Run from the repository root after installing the locked Rust dependencies and the frontend's `npm ci` dependencies:

```powershell
./scripts/collect-rust-notices.ps1
# Reproduce from already collected remote texts without network access:
./scripts/collect-rust-notices.ps1 -Offline
```

The script reads `cargo metadata --locked --filter-platform x86_64-pc-windows-msvc`. It selects package IDs from the **active resolve nodes**, rather than every package listed in cross-platform `Cargo.lock` metadata. The result contains **313 third-party Rust packages** and excludes the four workspace crates. It includes active build dependencies as well as application dependencies; 313 is not a count of libraries necessarily linked into the running app. Linux GLib/GTK and macOS Objective-C dependencies are absent from this Windows manifest.

Evidence for this collection:

- Cargo lock SHA-256: `bf189970c52cdf7a332ee9424f850112309bfd1bc4b2113664c086c4ea66e212`.
- [Windows Rust manifest](../licenses/rust/x86_64-pc-windows-msvc/manifest.json): package name, version, declared SPDX expression, repository/source archive, original license filename and SHA-256. All **558 referenced text files** were checked against their recorded SHA-256 values after collection.
- [Frontend manifest](../licenses/frontend/manifest.json): **seven production npm packages**, plus the separately attributed Handy palette; nine referenced license/notice texts. Development tools are excluded from this shipped-frontend list.
- Frontend lock SHA-256: `edee24f4b27f3df508ebabf577b35465eff65d583f7b9e1a36706938141bf769`.

All copied license/notice files use `.txt` names. Their contents are preserved, including the original copyright and disclaimer text. The manifests contain no absolute local paths or credentials. Historical slash-separated Cargo license expressions, such as `MIT/Apache-2.0`, are retained as declared metadata rather than silently rewritten.

Existing license files that differ from a newly collected copy cause the script to stop and preserve the existing file for review. No dependency versions are updated by collection. Remote fallback texts are pinned by source revision and by reviewed SHA-256. An offline run verifies the same hashes.

## Main components and original texts

| Component | Version | Declared license | Evidence |
| --- | --- | --- | --- |
| transcribe-cpp | 0.4.1 | MIT | Supplied crate `LICENSE`, copied verbatim |
| transcribe-cpp-sys | 0.4.1 | MIT | Supplied crate `LICENSE`, copied verbatim |
| wasapi | 0.26.0 | MIT | Supplied `LICENSE.txt`, copied verbatim |
| hound | 3.5.1 | Apache-2.0 | Supplied lowercase `license`, copied verbatim |
| tauri | 2.11.5 | Apache-2.0 OR MIT | Both supplied license files retained |
| @tauri-apps/api | 2.11.0 | Apache-2.0 OR MIT | Both supplied npm license files retained |
| React / React DOM | 18.3.1 | MIT | Supplied npm `LICENSE` files retained |
| Lucide React | 0.542.0 | ISC | Supplied npm `LICENSE` retained |
| Handy shared palette | commit 73c095857bc4eadff097f64767c40ad129cc372d | MIT | Original CJ Pais notice and license in [HANDY_STYLE_NOTICE](../apps/desktop/HANDY_STYLE_NOTICE.md), also copied into the frontend notices |

The full graph also includes BSD, Unicode, Boost, ISC, Zlib, CDLA and MPL declarations, including packages with combined license expressions. The project's MIT license does not replace their licenses. The manifest preserves every original expression and corresponding supplied texts rather than assuming the entire dependency graph is MIT.

Eleven published crates omit standalone license files. Their actual upstream license texts were retrieved from the revisions recorded in those crates' `.cargo_vcs_info.json`: alloc-stdlib, clipboard-win, five `unic-*` crates and three `webview2-com` crates. Their exact source URLs and hashes are recorded in the manifest. For `selectors`, its supplied source-header notice explicitly references Mozilla's MPL 2.0. That notice and the [canonical Mozilla text](https://www.mozilla.org/media/MPL/2.0/index.txt) were retained; no legal text was generated from an SPDX name.

MPL-declared source packages in this Windows build graph are available through the exact source-archive links in the manifest: `cssparser 0.36.0`, `cssparser-macros 0.6.1`, `dtoa-short 0.3.5`, `option-ext 0.2.0` and `selectors 0.36.1`. The collection does not modify their source files.

## Packaging boundaries

The beta package must include the applicable target's Rust notices, the frontend notices and [ADDITIONAL_NOTICES.txt](../licenses/rust/x86_64-pc-windows-msvc/ADDITIONAL_NOTICES.txt). Keep the reused Handy MIT notice with the frontend attribution.

The existing SDK at `.build/rust-native-0.4.1` and native transcribe.cpp runtime are unchanged by this collection. Preserve the runtime's existing `native/transcribe-0.4.1/licenses` tree in the distributed native directory, including its upstream transcribe.cpp, ggml and other component notices. These native licenses are additional to the Rust-wrapper licenses. Model weights are not bundled and have separate licenses.

Old .NET/Avalonia notices belong to the preserved prototypes. This collection is an additional beta notice set and does not rewrite the historical [THIRD_PARTY_NOTICES.txt](../THIRD_PARTY_NOTICES.txt). A Windows collection is not a completed Mac/Linux license or packaging review; regenerate and review the corresponding target graph before distributing those targets.

## Dependency security checks and unresolved warnings

The initially inherited dependency lock reported 12 known vulnerabilities. Compatible dependency updates were applied to the beta's own lock rather than treating upstream parity as sufficient security evidence.

An independent read-only `cargo-audit` run against the locally available RustSec database reported **zero vulnerability entries**, **six unmaintained-package warnings**, and **one unsoundness warning**. Its local raw report is `.build/rust-license-audit.json`; the database reported 1,352 advisories without a Git commit/time field. This is a check against that available database, not proof that every current vulnerability is known or that the code is safe.

| Category | Package | Version | Advisory |
| --- | --- | --- | --- |
| Unmaintained | proc-macro-error | 1.0.4 | [RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html) |
| Unmaintained | unic-char-property | 0.9.0 | [RUSTSEC-2025-0081](https://rustsec.org/advisories/RUSTSEC-2025-0081.html) |
| Unmaintained | unic-char-range | 0.9.0 | [RUSTSEC-2025-0075](https://rustsec.org/advisories/RUSTSEC-2025-0075.html) |
| Unmaintained | unic-common | 0.9.0 | [RUSTSEC-2025-0080](https://rustsec.org/advisories/RUSTSEC-2025-0080.html) |
| Unmaintained | unic-ucd-ident | 0.9.0 | [RUSTSEC-2025-0100](https://rustsec.org/advisories/RUSTSEC-2025-0100.html) |
| Unmaintained | unic-ucd-version | 0.9.0 | [RUSTSEC-2025-0098](https://rustsec.org/advisories/RUSTSEC-2025-0098.html) |
| Unsoundness | glib | 0.18.5 | [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html) |

The GLib entry belongs to the cross-platform lock's Linux stack and is **absent from the active Windows dependency graph**. That qualification does not dismiss the Linux issue: the beta's Linux target needs a compatible fix or a supported upstream migration before release validation. The six unmaintained dependencies remain tracked maintenance risks; they were not hidden or described as fixed.

The frontend's full npm audit initially found development-tool advisories. Exact Vite `6.4.5` and Vitest `4.1.12` updates removed those reports while preserving the existing React/Tauri API compatibility. The final **full audit, including development dependencies**, reported zero known npm advisories; local evidence is `.build/frontend-audit-patched.json`. The updated frontend build and twenty-one synthetic frontend regressions passed, including completed-history preservation during bounded polling, an asynchronous ending state, guarded focus-refresh of Handy models and explicit reselection when a chosen model disappears. Audit results do not replace device, recovery, privacy-boundary or long-session checks.
