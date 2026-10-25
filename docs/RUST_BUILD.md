# Building the Rust/Tauri beta

The Windows candidate is version `1.0.0-beta.1`. The output is a checked ZIP, not a validated NSIS installer or stable release. Existing C# programs, their launcher, recordings and settings are preserved. Nothing in this workflow publishes a GitHub release or pushes Git history.

## Separate tools and filesystem bookkeeping

Verification uses separate standard tool invocations. `scripts/rust-artifacts.mjs` uses only filesystem, path and cryptographic operations: it never starts processes, runs shell commands, downloads packages or changes security settings. Its receipts record exit codes reported by those separate tools; actual CI logs remain evidence that the commands ran.

The blocked combined verification script is not used, restored or executed from an alternate location. Build and package scripts contain no reference to it. Native SDK preparation and ordinary package-manager setup are separate steps. Use normal foreground developer commands; no antivirus exception or global tool update is required by this design.

## Prerequisites and preparation

Use PowerShell 7.4+, Rust 1.91.1 with rustfmt/clippy, Node 26.7.0 and Visual Studio C++ Build Tools. The preserved C# regression suite also needs the .NET SDK pinned by `global.json`. Running the packaged Rust application does not need .NET.

Prepare the pinned native SDK separately:

```powershell
./scripts/prepare-rust-native.ps1
```

Use npm **12.2.1** through its local `bin/npm-cli.js`, without changing global npm. Set `HANDYMEETING_NPM_CLI` to a working exact-version CLI. Existing reviewed project-local tools can be reused. For a clean machine, the Windows CI preparation step shows ordinary `npm pack npm@12.2.1`, a fixed archive SHA-512 comparison, and `npm install` of that verified local archive under `.build/tools/npm-bootstrap`, with lifecycle scripts disabled.

The reviewed integrity is:

```text
sha512-bKCmFRVT/BB+YPEYbZwQ1XUI3N5t4xKSpU1MxOjo7z3iWzKBJ0ZXRC1xFdEN+TC7N5TdywaBzf2dMXGJq/e2PQ==
```

Application dependencies use the exact `package-lock.json`:

```powershell
node $env:HANDYMEETING_NPM_CLI --prefix apps/desktop ci --ignore-scripts --no-audit --no-fund
```

Keep the command's actual failure visible. Do not record a successful check after a failed tool invocation.

## Independent verification

Start the source-bound check run, then execute each standard tool separately and immediately record its actual exit code. CI implements these as separately named steps.

```powershell
node scripts/rust-artifacts.mjs begin-check

./verify.ps1
# Only after successful completion of the throwing legacy verification script:
node scripts/rust-artifacts.mjs record-check legacy 0

node scripts/rust-artifacts.mjs self-test
node scripts/rust-artifacts.mjs record-check artifact-guards $LASTEXITCODE

cargo fmt --all -- --check
node scripts/rust-artifacts.mjs record-check cargo-format $LASTEXITCODE

node $env:HANDYMEETING_NPM_CLI --prefix apps/desktop test
node scripts/rust-artifacts.mjs record-check frontend-tests $LASTEXITCODE

node $env:HANDYMEETING_NPM_CLI --prefix apps/desktop run build
node scripts/rust-artifacts.mjs record-check frontend-build $LASTEXITCODE
```

Frontend assets must exist before Rust compilation because Tauri's production protocol embeds them. Set the native SDK for the current build process, then run the Rust checks:

```powershell
$env:TRANSCRIBE_DIR = (Resolve-Path .build/rust-native-0.4.1).Path
$env:PATH = (Join-Path $env:TRANSCRIBE_DIR 'bin') + ';' + $env:PATH

cargo test --workspace --all-targets --locked
node scripts/rust-artifacts.mjs record-check rust-tests $LASTEXITCODE

cargo clippy --workspace --all-targets --locked -- -D warnings
node scripts/rust-artifacts.mjs record-check rust-clippy $LASTEXITCODE

node scripts/rust-artifacts.mjs finish-check
```

A nonzero reported exit code is retained as a failed event and makes the recording command fail. Finish requires every fixed named check to succeed, the same source fingerprint, reviewed native SDK identity, and matching Cargo/npm license manifests and retained notice hashes. Dependency/license regeneration is a separate explicit action using `scripts/collect-rust-notices.ps1`. Locks and licenses are never silently rewritten by artifact bookkeeping.

`begin-check --skip-legacy` is available for isolated development checks; such a receipt cannot stage a distributable candidate.

## Exact release executable and staging

The minimal build script requires an already finished matching check receipt. It runs only the fixed release build with compiler warnings rejected and restores its process-local native environment:

```powershell
./scripts/build-rust.ps1 -UseExistingVerification
```

The equivalent fixed Cargo build can also be run directly. It targets `x86_64-pc-windows-msvc` under `.build/rust-target`. Native SDK preparation is never hidden inside verification or artifact staging.

Bind that binary before running its isolated model-free checks:

```powershell
$pending = node scripts/rust-artifacts.mjs begin-release-check | ConvertFrom-Json
& ./.build/rust-target/x86_64-pc-windows-msvc/release/HandyMeeting.exe --release-checks $pending.outputDirectory | Out-Host
# Stop if the executable failed.
node scripts/rust-artifacts.mjs stage
./scripts/package-rust-windows.ps1
```

Keep the native SDK's `bin` directory in the current process PATH for the direct release check. The release-check mode opens no GUI, microphone, Handy installation or model. Staging requires its fresh successful versioned JSON, the exact unchanged executable SHA-256, and the original checked source fingerprint.

The only program destination is `dist-rust-1.0.0-beta.1`. Its payload contains the checked executable, known native DLLs and original native licenses, Rust/frontend notices, project license, and Handy palette attribution. A manifest hashes every retained file. Existing different bytes are rejected before publishing any new payload. The separate launcher is **HandyMeeting Rust Beta starten.cmd**. The original **HandyMeeting starten.cmd** remains untouched.

Rust user data lives outside this directory under local `HandyMeeting/RustBeta` application data. No model weights, recordings, settings or API keys are staged.

## Package correspondence

The filesystem-only package script independently recomputes the same source fingerprint, checks the release receipt, executable SHA-256, payload manifest and launcher hash, and copies only those exact files. It never starts the application or any tools. System.IO.Compression creates the ZIP with a SHA-256 sidecar and verification JSON.

The source fingerprint includes all Rust files in the three crates and desktop app, the preserved C# source/project/XAML files and their root verification/SDK/NuGet configuration, frontend source/configuration, Cargo/npm locks, build scripts including the bookkeeping module, bundled assets and Rust/frontend license records. Dependency trees, build output, TypeScript caches, and generated Tauri schemas are excluded consistently. Added Rust source files change the fingerprint. If source changes between checks and staging/package creation, a new check run is required. The prepared SDK is checked against its complete reviewed DLL manifest, including rejection of additional DLLs; staged files are checked again against their payload hashes.

An existing ZIP is preserved. Use a new version or a fresh `-OutputDirectory` instead of overwriting an earlier candidate. Receipts and individual check events remain under `.build`; they are not private meeting data and contain no model weights.

## CI scope and manual evidence

`.github/workflows/rust.yml` is configured to run the independent legacy, frontend, Rust and artifact checks on Windows, build/check the exact release executable, stage it, and retain a ZIP Actions artifact. It does not publish a release. Third-party Actions are pinned to immutable commits. A prepared workflow is not a successful CI run.

Ubuntu and both Mac architectures run only the three portable Rust crates with default ASR features disabled, including synthetic persistence/transport tests and strict warnings. They do not validate a native GUI, real capture, speech inference or portable recording reliability.

Real local text-model references, the native two-track speech/restart smoke test, actual audio devices/disconnects, and a new two-hour Rust memory/recovery run are separate release gates. Fixtures and raw reports stay outside public source export. The earlier C# two-hour result does not certify this beta. Process termination is not a power-loss test. See `TEST_REPORT.md` and `RUST_STORAGE.md` for actual measurements and outstanding checks.
