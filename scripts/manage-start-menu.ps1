# A failed ownership check preserves the existing shortcut. The installer passes a
# process-scope -ExecutionPolicy only; persistent and Group Policy settings are untouched.
param([Parameter(Mandatory)][ValidateSet('install','uninstall')][string]$Mode,[Parameter(Mandatory)][string]$InstallDirectory)
$ErrorActionPreference='Stop'
$program=[IO.Path]::GetFullPath((Join-Path $InstallDirectory 'HandyMeeting.exe'))
$shortcutDirectory=Join-Path ([Environment]::GetFolderPath('Programs')) 'HandyMeeting'
$shortcut=Join-Path $shortcutDirectory 'HandyMeeting.lnk'
$owner='HKCU:\Software\HandyMeeting\StartMenuLink'
$sid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$mutex=[Threading.Mutex]::new($false,('Local\HandyMeeting-StartMenu-'+$sid))
$locked=$false
$temporary=$null
try {
    try {$locked=$mutex.WaitOne(15000)} catch [Threading.AbandonedMutexException] {$locked=$true}
    if (-not $locked) {exit 3}
    $record=Get-ItemProperty -LiteralPath $owner -ErrorAction SilentlyContinue
    $exists=Test-Path -LiteralPath $shortcut
    $owned=$false
    if ($exists -and $record -and $record.Link -eq $shortcut) {
        $file=Get-Item -LiteralPath $shortcut -Force
        if (($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0 -and -not $file.PSIsContainer) {
            $owned=(Get-FileHash -LiteralPath $shortcut -Algorithm SHA256).Hash -eq $record.SHA256
        }
    }
    if ($Mode -eq 'uninstall') {
        if ($owned -and $record.Target -eq $program) {
            Remove-Item -LiteralPath $shortcut
            Remove-Item -LiteralPath $owner
        }
        exit 0
    }
    if ($exists -and -not $owned) {exit 3}
    if (Test-Path -LiteralPath $shortcutDirectory) {
        $directory=Get-Item -LiteralPath $shortcutDirectory -Force
        if (-not $directory.PSIsContainer -or ($directory.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {exit 3}
    } else {New-Item -ItemType Directory -Path $shortcutDirectory | Out-Null}
    if ($exists) {
        $backup=Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'HandyMeeting\shortcut-history'
        New-Item -ItemType Directory -Path $backup -Force | Out-Null
        Copy-Item -LiteralPath $shortcut -Destination (Join-Path $backup ([guid]::NewGuid().ToString('N')+'.lnk'))
    }
    $temporary=Join-Path $shortcutDirectory ([guid]::NewGuid().ToString('N')+'.lnk')
    $shell=New-Object -ComObject WScript.Shell
    $link=$shell.CreateShortcut($temporary)
    $link.TargetPath=$program
    $link.WorkingDirectory=[IO.Path]::GetDirectoryName($program)
    $link.Description='HandyMeeting'
    $link.Save()
    # The named mutex serializes cooperating installers; verify old bytes again.
    if ($exists -and (Get-FileHash -LiteralPath $shortcut -Algorithm SHA256).Hash -ne $record.SHA256) {exit 3}
    Move-Item -LiteralPath $temporary -Destination $shortcut -Force
    New-Item -Path $owner -Force | Out-Null
    New-ItemProperty -LiteralPath $owner -Name Link -Value $shortcut -PropertyType String -Force | Out-Null
    New-ItemProperty -LiteralPath $owner -Name Target -Value $program -PropertyType String -Force | Out-Null
    New-ItemProperty -LiteralPath $owner -Name SHA256 -Value (Get-FileHash -LiteralPath $shortcut -Algorithm SHA256).Hash -PropertyType String -Force | Out-Null
} catch {exit 3} finally {
    # Only this script's own unpublished temporary link is removed.
    if ($temporary -and (Test-Path -LiteralPath $temporary)) {Remove-Item -LiteralPath $temporary -ErrorAction SilentlyContinue}
    if ($locked){$mutex.ReleaseMutex()};$mutex.Dispose()
}
