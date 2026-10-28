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
        $uninstallRoot = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall'
        $registration = Join-Path $uninstallRoot 'MeetingForHandy'
        $programs = [Environment]::GetFolderPath('Programs')
        $startLink = Join-Path $programs 'Meeting for Handy.lnk'
        $desktopLink = Join-Path ([Environment]::GetFolderPath('Desktop')) 'Meeting for Handy.lnk'
        $foreignTarget = Join-Path $env:WINDIR 'notepad.exe'
        $shell = New-Object -ComObject WScript.Shell
        function Invoke-Setup([string[]]$Arguments) { (Start-Process -FilePath $package -ArgumentList $Arguments -WindowStyle Hidden -Wait -PassThru).ExitCode }
        function Assert-Payload([string]$Directory, [string]$Message) {
            foreach ($file in $receipt.programFiles) { if ((Get-FileHash -LiteralPath (Join-Path $Directory $file.path)).Hash.ToLowerInvariant() -ne $file.sha256) { throw $Message } }
        }
        function Literal([string]$Value) { $Value.Replace('$','$$').Replace('"','$\"') }
        function Link-Target([string]$Path) { if (Test-Path -LiteralPath $Path) { [IO.Path]::GetFullPath($shell.CreateShortcut($Path).TargetPath) } else { $null } }
        function Set-ForeignDesktopLink { $link = $shell.CreateShortcut($desktopLink); $link.TargetPath = $foreignTarget; $link.Save() }
        # A folder with foreign files is never used and nothing is registered for it.
        $occupied = Join-Path $work 'preexisting'
        New-Item -ItemType Directory -Path $occupied | Out-Null
        $existingFile = Join-Path $occupied 'HandyMeeting.exe'
        [IO.File]::WriteAllText($existingFile,'Synthetic user-owned original, not an executable.')
        if ((Invoke-Setup @('/S',('/D=' + $occupied))) -eq 0 -or [IO.File]::ReadAllText($existingFile) -ne 'Synthetic user-owned original, not an executable.') { throw 'Installer replaced an existing destination.' }
        if (Test-Path -LiteralPath $registration) { throw 'A rejected destination was registered.' }
        # Synthetic installation of the former one-folder-per-version layout (up to 1.0.5): its own
        # uninstaller removes its program file and registration; a user file must survive.
        $legacy = Join-Path $work 'legacy programs/HandyMeeting/0.0.1'
        New-Item -ItemType Directory -Path (Join-Path $legacy 'Sessions') | Out-Null
        $legacyProgram = Join-Path $legacy 'HandyMeeting.exe'
        [IO.File]::WriteAllText($legacyProgram,'Synthetic former version, not an executable.')
        $legacyUserFile = Join-Path $legacy 'Sessions/legacy-user-file.txt'
        [IO.File]::WriteAllText($legacyUserFile,'Synthetic user data must survive the migration.')
        $legacyRegistration = Join-Path $uninstallRoot 'HandyMeeting-0.0.1'
        $legacySource = Join-Path $work 'legacy-uninstaller.nsi'
        $legacyLines = @('Unicode true', 'SilentInstall silent', 'RequestExecutionLevel user',
            ('OutFile "' + (Literal (Join-Path $legacy 'Uninstall.exe')) + '"'), 'Section',
            ('Delete "' + (Literal $legacyProgram) + '"'),
            'DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-0.0.1"', 'SectionEnd')
        [IO.File]::WriteAllLines($legacySource, $legacyLines, [Text.UTF8Encoding]::new($true))
        & $compiler.compiler /V2 /WX $legacySource
        if ($LASTEXITCODE -ne 0) { throw 'Synthetic former uninstaller compilation failed.' }
        New-Item -Path $legacyRegistration -Force | Out-Null
        New-ItemProperty -LiteralPath $legacyRegistration -Name InstallLocation -Value $legacy -PropertyType String -Force | Out-Null
        # The former folder link with the ownership record the 1.0.5 helper wrote for it.
        $legacyLink = [IO.Path]::GetFullPath((Join-Path $programs 'Meeting for Handy/Meeting for Handy.lnk'))
        New-Item -ItemType Directory -Path (Split-Path $legacyLink -Parent) -Force | Out-Null
        $link = $shell.CreateShortcut($legacyLink)
        $link.TargetPath = [IO.Path]::GetFullPath($legacyProgram)
        $link.Save()
        $legacyOwner = 'HKCU:\Software\HandyMeeting\BrandedStartMenuLink'
        New-Item -Path $legacyOwner -Force | Out-Null
        New-ItemProperty -LiteralPath $legacyOwner -Name Link -Value $legacyLink -PropertyType String -Force | Out-Null
        New-ItemProperty -LiteralPath $legacyOwner -Name Target -Value ([IO.Path]::GetFullPath($legacyProgram)) -PropertyType String -Force | Out-Null
        # A desktop link of the same name that the setup did not create is never replaced.
        if (Test-Path -LiteralPath $desktopLink) { throw 'The disposable CI account already has a Meeting for Handy desktop link.' }
        Set-ForeignDesktopLink
        try {
            # Fresh install with missing parents; the Start menu link needs no PowerShell, so a
            # Restricted execution policy (the Windows client default) cannot block it.
            $install = Join-Path $work 'Fresh programs ä/Meeting for Handy'
            if ((Invoke-Setup @('/S','/DESKTOPSHORTCUT',('/D=' + $install))) -ne 0) { throw 'Silent installer check failed.' }
            Assert-Payload $install 'Installed program differs from checked payload.'
            $registered = Get-ItemProperty -LiteralPath $registration
            if ($registered.InstallLocation -ne $install -or $registered.DisplayName -ne 'Meeting for Handy' -or $registered.DisplayVersion -ne $version) { throw 'Installer registration differs from its own directory or name.' }
            $program = Join-Path $install 'HandyMeeting.exe'
            if (-not (Test-Path -LiteralPath $startLink) -or [IO.Path]::GetFullPath($shell.CreateShortcut($startLink).TargetPath) -ne [IO.Path]::GetFullPath($program)) { throw 'Meeting for Handy Start menu link is missing or points elsewhere.' }
            if (Test-Path -LiteralPath (Join-Path $install 'ManageStartMenu.ps1')) { throw 'The installer still ships the PowerShell Start menu helper.' }
            if ((Test-Path -LiteralPath $legacyProgram) -or (Test-Path -LiteralPath (Join-Path $legacy 'Uninstall.exe')) -or (Test-Path -LiteralPath $legacyRegistration)) { throw 'The former version folder was not migrated.' }
            if ([IO.File]::ReadAllText($legacyUserFile) -ne 'Synthetic user data must survive the migration.') { throw 'Migration removed a user file.' }
            if ((Test-Path -LiteralPath $legacyLink) -or (Test-Path -LiteralPath $legacyOwner)) { throw 'The former Start menu link into a version folder or its record remained.' }
            if ((Link-Target $desktopLink) -ne [IO.Path]::GetFullPath($foreignTarget) -or $null -ne $registered.DesktopShortcut) { throw 'Setup replaced or claimed a foreign desktop link.' }
        } finally {
            Remove-Item -LiteralPath $legacyRegistration -ErrorAction SilentlyContinue
        }
        # Running setup again repairs the registered installation in place and keeps user files.
        $sentinel = Join-Path $install 'Sessions/user-owned-sentinel.txt'
        New-Item -ItemType Directory -Path (Split-Path $sentinel -Parent) | Out-Null
        [IO.File]::WriteAllText($sentinel,'Synthetic user data must survive repair and uninstall.')
        Remove-Item -LiteralPath (Join-Path $install 'LICENSE')
        # Without the foreign link the repair creates and records its own desktop link.
        Remove-Item -LiteralPath $desktopLink
        if ((Invoke-Setup @('/S','/DESKTOPSHORTCUT')) -ne 0) { throw 'Running setup again did not repair the installation.' }
        if ((Link-Target $desktopLink) -ne [IO.Path]::GetFullPath($program) -or (Get-ItemProperty -LiteralPath $registration).DesktopShortcut -ne $desktopLink) { throw 'The requested desktop link is missing, points elsewhere or is not recorded.' }
        Assert-Payload $install 'Repair did not restore the checked payload.'
        if ([IO.File]::ReadAllText($sentinel) -ne 'Synthetic user data must survive repair and uninstall.' -or (Get-ItemProperty -LiteralPath $registration).InstallLocation -ne $install) { throw 'Repair changed user data or the registration.' }
        # A running app is never ended or replaced: setup waits, then stops without changes.
        $lock = [IO.File]::Open($program, 'Open', 'Read', 'None')
        try {
            if ((Invoke-Setup @('/S')) -eq 0) { throw 'Setup replaced a running program.' }
        } finally { $lock.Dispose() }
        Assert-Payload $install 'Setup changed program files while the app was running.'
        & $program --release-checks (Join-Path $work 'installed-checks') | Out-Host
        if ($LASTEXITCODE -ne 0) { throw 'Installed executable checks failed.' }
        # Someone replaced the recorded desktop link with their own: uninstall keeps it.
        Remove-Item -LiteralPath $desktopLink
        Set-ForeignDesktopLink
        $child = Start-Process -FilePath (Join-Path $install 'Uninstall.exe') -ArgumentList @('/S',('_?=' + $install)) -WindowStyle Hidden -Wait -PassThru
        if ($child.ExitCode -ne 0) { throw 'Silent uninstall check failed.' }
        if ((Link-Target $desktopLink) -ne [IO.Path]::GetFullPath($foreignTarget)) { throw 'Uninstall deleted a desktop link it did not create.' }
        Remove-Item -LiteralPath $desktopLink
        foreach ($file in $receipt.programFiles) { if (Test-Path -LiteralPath (Join-Path $install $file.path)) { throw 'Uninstall left an owned program payload file.' } }
        if ([IO.File]::ReadAllText($sentinel) -ne 'Synthetic user data must survive repair and uninstall.') { throw 'Uninstall removed user data.' }
        if ((Test-Path -LiteralPath $startLink) -or (Test-Path -LiteralPath $registration)) { throw 'Uninstall left its Start menu link or registration.' }
        # The setup's own desktop link goes with its uninstall. A Start menu link of the same
        # name that starts another program is neither replaced nor deleted.
        $foreignStart = $shell.CreateShortcut($startLink)
        $foreignStart.TargetPath = $foreignTarget
        $foreignStart.Save()
        $second = Join-Path $work 'Second programs/Meeting for Handy'
        if ((Invoke-Setup @('/S','/DESKTOPSHORTCUT',('/D=' + $second))) -ne 0) { throw 'Second silent installer check failed.' }
        if ((Link-Target $desktopLink) -ne [IO.Path]::GetFullPath((Join-Path $second 'HandyMeeting.exe'))) { throw 'The second installation has no own desktop link.' }
        if ((Link-Target $startLink) -ne [IO.Path]::GetFullPath($foreignTarget)) { throw 'Setup replaced a foreign Start menu link.' }
        $child = Start-Process -FilePath (Join-Path $second 'Uninstall.exe') -ArgumentList @('/S',('_?=' + $second)) -WindowStyle Hidden -Wait -PassThru
        if ($child.ExitCode -ne 0 -or (Test-Path -LiteralPath $desktopLink) -or (Test-Path -LiteralPath $registration)) { throw 'Uninstall left its own desktop link or registration.' }
        if ((Link-Target $startLink) -ne [IO.Path]::GetFullPath($foreignTarget)) { throw 'Uninstall deleted a Start menu link it did not create.' }
        Remove-Item -LiteralPath $startLink
        $tested = $true
    }
    $hash = (Get-FileHash -LiteralPath $package -Algorithm SHA256).Hash.ToLowerInvariant()
    $proof = [ordered]@{success=$true;version=$version;rid='windows-x64';package=[IO.Path]::GetFileName($package);sha256=$hash;bytes=(Get-Item -LiteralPath $package).Length;sourceFingerprintSHA256=$receipt.sourceFingerprintSHA256;executableSHA256=$receipt.executableSHA256;compilerVersion=$compiler.version;compilerArchiveSHA256=$compiler.archiveSHA256;actualInstallUninstallChecked=$tested;existingDestinationRejected=$tested;freshMissingParentsChecked=$tested;reinstallRepaired=$tested;startMenuShortcutChecked=$tested;startMenuWithoutPowerShell=$true;legacyVersionsMigrated=$tested;runningProgramPreserved=$tested;desktopShortcutOwnershipChecked=$tested;unknownUserFilePreserved=$tested;recursiveDeletion=$false;userRecordingsIncluded=$false;signing='unsigned'}
    [IO.File]::WriteAllText(($package+'.sha256'),($hash+'  '+[IO.Path]::GetFileName($package)+"`n"),[Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText(($package+'.verification.json'),(($proof|ConvertTo-Json)+"`n"),[Text.UTF8Encoding]::new($false))
    $proof | ConvertTo-Json
} finally { Pop-Location }
