#requires -Version 7.4
param([switch]$UseExistingVerification)
$ErrorActionPreference = 'Stop'
if (-not $IsWindows -or -not [Environment]::Is64BitProcess) { throw 'This entry point builds Windows x64 only.' }
$root = Split-Path $PSScriptRoot -Parent
$originalNative = $env:TRANSCRIBE_DIR
$originalPath = $env:PATH
$originalFlags = $env:RUSTFLAGS
Push-Location $root
try {
    # The check receipt comes from separate, completed standard tool invocations.
    & node scripts/rust-artifacts.mjs require-check
    if ($LASTEXITCODE -ne 0) { throw 'Complete the independent verification steps first.' }
    $env:TRANSCRIBE_DIR = Join-Path $root '.build/rust-native-0.4.1'
    $env:PATH = (Join-Path $env:TRANSCRIBE_DIR 'bin') + [IO.Path]::PathSeparator + $originalPath
    if ($env:CARGO_ENCODED_RUSTFLAGS) { throw 'Encoded Rust flags override warning enforcement. Use a clean build environment.' }
    $env:RUSTFLAGS = if ([string]::IsNullOrWhiteSpace($originalFlags)) { '-D warnings' } else { $originalFlags + ' -D warnings' }
    & cargo build -p handy-meeting-desktop --release --locked --target x86_64-pc-windows-msvc --target-dir .build/rust-target
    if ($LASTEXITCODE -ne 0) { throw 'Rust release build failed.' }
    Write-Host 'Release built. Next run rust-artifacts.mjs begin-release-check, the exact executable --release-checks, then rust-artifacts.mjs stage as separate steps.'
} finally {
    $env:TRANSCRIBE_DIR = $originalNative
    $env:PATH = $originalPath
    $env:RUSTFLAGS = $originalFlags
    Pop-Location
}
