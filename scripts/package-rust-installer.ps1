#requires -Version 7.4
param([switch]$RunChecks)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    if ($RunChecks -and $env:GITHUB_ACTIONS -ne 'true') { throw 'Installer execution checks run only in the disposable CI account.' }
    & node scripts/rust-artifacts.mjs require-check
    if ($LASTEXITCODE -ne 0) { throw 'Current source verification required.' }
    $receipt = Get-Content .build/rust-release-verification.json -Raw | ConvertFrom-Json
    if (-not $receipt.success -or -not $receipt.releaseBinaryChecksPassed) { throw 'Exact release executable proof required.' }
    $version = $receipt.version
    if ($version -notmatch '^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?$') { throw 'Invalid installer version.' }
    $program = Join-Path $root $receipt.programDirectory
    foreach ($file in $receipt.programFiles) {
        if ([IO.Path]::IsPathRooted($file.path) -or $file.path.Contains('..') -or $file.path.Contains(':')) { throw 'Unsafe installer input.' }
        if ((Get-FileHash -LiteralPath (Join-Path $program $file.path)).Hash.ToLowerInvariant() -ne $file.sha256) { throw 'Checked installer payload changed.' }
    }
    $compiler = & ./scripts/prepare-installer.ps1 | ConvertFrom-Json
    $work = Join-Path $root ('.build/installer-' + [guid]::NewGuid().ToString('N'))
    $lists = Join-Path $work 'lists'
    & python scripts/windows-installer-files.py $program $lists
    if ($LASTEXITCODE -ne 0) { throw 'Installer payload generation failed.' }
    $package = Join-Path $root ('packages/HandyMeeting-' + $version + '-windows-x64-setup.exe')
    if (Test-Path -LiteralPath $package) { throw 'Existing installer was preserved. Choose a new version or a fresh CI workspace.' }
    $arguments = @('/V2','/WX',('/DAPP_VERSION=' + $version),('/DNUMERIC_VERSION=' + $version.Split('-')[0] + '.0'),('/DOUTPUT_FILE=' + $package),('/DAPP_ICON=' + (Join-Path $root 'assets/handymeeting.ico')),('/DAPP_LICENSE=' + (Join-Path $root 'LICENSE')),('/DINSTALL_FILES=' + (Join-Path $lists 'install.nsh')),('/DUNINSTALL_FILES=' + (Join-Path $lists 'uninstall.nsh')),(Join-Path $PSScriptRoot 'windows-installer.nsi'))
    & $compiler.compiler @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Installer compiler failed.' }
    $tested = $false
    if ($RunChecks) {
        $occupied = Join-Path $work 'preexisting'
        New-Item -ItemType Directory -Path $occupied | Out-Null
        $existingFile = Join-Path $occupied 'HandyMeeting.exe'
        [IO.File]::WriteAllText($existingFile,'Synthetic user-owned original, not an executable.')
        $rejected = Start-Process -FilePath $package -ArgumentList @('/S',('/D=' + $occupied)) -WindowStyle Hidden -Wait -PassThru
        if ($rejected.ExitCode -eq 0 -or [IO.File]::ReadAllText($existingFile) -ne 'Synthetic user-owned original, not an executable.') { throw 'Installer replaced an existing destination.' }
        $install = Join-Path $work 'installed'
        $child = Start-Process -FilePath $package -ArgumentList @('/S',('/D=' + $install)) -WindowStyle Hidden -Wait -PassThru
        if ($child.ExitCode -ne 0) { throw 'Silent installer check failed.' }
        foreach ($file in $receipt.programFiles) { if ((Get-FileHash -LiteralPath (Join-Path $install $file.path)).Hash.ToLowerInvariant() -ne $file.sha256) { throw 'Installed program differs from checked payload.' } }
        $registration = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-' + $version
        if ((Get-ItemProperty -LiteralPath $registration).InstallLocation -ne $install) { throw 'Installer registration differs from its own directory.' }
        $secondInstall = Join-Path $work 'second-install'
        $duplicate = Start-Process -FilePath $package -ArgumentList @('/S',('/D=' + $secondInstall)) -WindowStyle Hidden -Wait -PassThru
        if ($duplicate.ExitCode -eq 0 -or (Test-Path -LiteralPath (Join-Path $secondInstall 'HandyMeeting.exe')) -or (Get-ItemProperty -LiteralPath $registration).InstallLocation -ne $install) { throw 'Duplicate version replaced an existing installation registration.' }
        & (Join-Path $install 'HandyMeeting.exe') --release-checks (Join-Path $work 'installed-checks') | Out-Host
        if ($LASTEXITCODE -ne 0) { throw 'Installed executable checks failed.' }
        $sentinel = Join-Path $install 'Sessions/user-owned-sentinel.txt'
        New-Item -ItemType Directory -Path (Split-Path $sentinel -Parent) | Out-Null
        [IO.File]::WriteAllText($sentinel,'Synthetic user data must survive uninstall.')
        $uninstall = Join-Path $install 'Uninstall.exe'
        $child = Start-Process -FilePath $uninstall -ArgumentList @('/S',('_?=' + $install)) -WindowStyle Hidden -Wait -PassThru
        if ($child.ExitCode -ne 0) { throw 'Silent uninstall check failed.' }
        foreach ($file in $receipt.programFiles) { if (Test-Path -LiteralPath (Join-Path $install $file.path)) { throw 'Uninstall left an owned program payload file.' } }
        if ((Test-Path -LiteralPath (Join-Path $install 'HandyMeeting.exe')) -or [IO.File]::ReadAllText($sentinel) -ne 'Synthetic user data must survive uninstall.') { throw 'Uninstall did not remove only the owned program files.' }
        $tested = $true
    }
    $hash = (Get-FileHash -LiteralPath $package -Algorithm SHA256).Hash.ToLowerInvariant()
    $proof = [ordered]@{success=$true;version=$version;rid='windows-x64';package=[IO.Path]::GetFileName($package);sha256=$hash;bytes=(Get-Item -LiteralPath $package).Length;sourceFingerprintSHA256=$receipt.sourceFingerprintSHA256;executableSHA256=$receipt.executableSHA256;compilerVersion=$compiler.version;compilerArchiveSHA256=$compiler.archiveSHA256;actualInstallUninstallChecked=$tested;existingDestinationRejected=$tested;duplicateVersionRejected=$tested;unknownUserFilePreserved=$tested;recursiveDeletion=$false;userRecordingsIncluded=$false;signing='unsigned'}
    [IO.File]::WriteAllText(($package+'.sha256'),($hash+'  '+[IO.Path]::GetFileName($package)+"`n"),[Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText(($package+'.verification.json'),(($proof|ConvertTo-Json)+"`n"),[Text.UTF8Encoding]::new($false))
    $proof | ConvertTo-Json
} finally { Pop-Location }
