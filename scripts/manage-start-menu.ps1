# A failed ownership check preserves the existing shortcut. Installer and helper
# never override execution policy; a blocked helper leaves the executable available.
# Exit codes: 0 link ready or nothing to do, 3 link preserved or blocked, 4 link kept on a newer version.
param([Parameter(Mandatory)][ValidateSet('install','uninstall')][string]$Mode,[Parameter(Mandatory)][string]$InstallDirectory,[string]$Version='')
$ErrorActionPreference='Stop'
$program=[IO.Path]::GetFullPath((Join-Path $InstallDirectory 'HandyMeeting.exe'))
$shortcutDirectory=Join-Path ([Environment]::GetFolderPath('Programs')) 'Meeting for Handy'
$shortcut=Join-Path $shortcutDirectory 'Meeting for Handy.lnk'
$owner='HKCU:\Software\HandyMeeting\BrandedStartMenuLink'
$sid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$mutex=[Threading.Mutex]::new($false,('Local\HandyMeeting-StartMenu-'+$sid))
$locked=$false
$temporary=$null
# Get-FileHash is a script-module function in Windows PowerShell 5.1 and fails to load under
# the Restricted user policy; the ownership hash therefore uses .NET directly.
function Get-LinkHash([string]$Path) {
    $sha=[Security.Cryptography.SHA256]::Create()
    $stream=[IO.File]::OpenRead($Path)
    try {return [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-','')} finally {$stream.Dispose();$sha.Dispose()}
}
# A parsed version: numeric release fields and the prerelease identifiers. Unknown text is $null.
function ConvertTo-HandyVersion([string]$Value) {
    if (-not ($Value -match '^([0-9]{1,9})\.([0-9]{1,9})\.([0-9]{1,9})(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$')) {return $null}
    $prerelease=[string[]]@()
    if ($Matches[4]) {$prerelease=[string[]]$Matches[4].Split('.')}
    return [pscustomobject]@{Release=[version]::new([int]$Matches[1],[int]$Matches[2],[int]$Matches[3]);Prerelease=$prerelease}
}
# SemVer precedence, -1, 0 or 1: release fields numerically, then a prerelease below its
# release, then identifiers left to right (numbers numerically and below text, text
# ordinally, a shorter list first), so beta.2 follows beta.1 and beta.10 follows beta.9.
function Compare-HandyVersion($Left,$Right) {
    $order=$Left.Release.CompareTo($Right.Release)
    if ($order -ne 0) {return [Math]::Sign($order)}
    $a=$Left.Prerelease
    $b=$Right.Prerelease
    if ($a.Count -eq 0 -or $b.Count -eq 0) {return [Math]::Sign($b.Count-$a.Count)}
    for ($i=0; $i -lt [Math]::Min($a.Count,$b.Count); $i++) {
        $x=$a[$i]
        $y=$b[$i]
        $xNumber=$x -match '^[0-9]+$'
        $yNumber=$y -match '^[0-9]+$'
        if ($xNumber -and $yNumber) {
            $x=$x.TrimStart('0')
            $y=$y.TrimStart('0')
            $order=if ($x.Length -ne $y.Length) {$x.Length-$y.Length} else {[string]::CompareOrdinal($x,$y)}
        } elseif ($xNumber -or $yNumber) {
            $order=if ($xNumber) {-1} else {1}
        } else {
            $order=[string]::CompareOrdinal($x,$y)
        }
        if ($order -ne 0) {return [Math]::Sign($order)}
    }
    return [Math]::Sign($a.Count-$b.Count)
}
function Select-NewestInstallation($Installations) {
    $newest=$null
    foreach ($item in $Installations) {
        if (-not $newest -or (Compare-HandyVersion $item.Version $newest.Version) -gt 0) {$newest=$item}
    }
    return $newest
}
# Side-by-side installations registered by this installer for the current user.
function Get-HandyInstallations {
    $found=@()
    $keys=@(Get-ChildItem -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' -ErrorAction SilentlyContinue | Where-Object {$_.PSChildName -like 'HandyMeeting-*'})
    foreach ($key in $keys) {
        $parsed=ConvertTo-HandyVersion ($key.PSChildName.Substring(13))
        $location=(Get-ItemProperty -LiteralPath $key.PSPath -ErrorAction SilentlyContinue).InstallLocation
        if (-not $parsed -or -not $location) {continue}
        try {$target=[IO.Path]::GetFullPath((Join-Path $location 'HandyMeeting.exe'))} catch {continue}
        if (Test-Path -LiteralPath $target -PathType Leaf) {$found+=[pscustomobject]@{Version=$parsed;Program=$target}}
    }
    return $found
}
# Publishes a link to $Target and records ownership; existing bytes are backed up first.
function Publish-HandyLink([string]$Target) {
    if (Test-Path -LiteralPath $shortcutDirectory) {
        $directory=Get-Item -LiteralPath $shortcutDirectory -Force
        if (-not $directory.PSIsContainer -or ($directory.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {exit 3}
    } else {New-Item -ItemType Directory -Path $shortcutDirectory | Out-Null}
    if ($exists) {
        $backup=Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'HandyMeeting\shortcut-history'
        New-Item -ItemType Directory -Path $backup -Force | Out-Null
        Copy-Item -LiteralPath $shortcut -Destination (Join-Path $backup ([guid]::NewGuid().ToString('N')+'.lnk'))
    }
    $script:temporary=Join-Path $shortcutDirectory ([guid]::NewGuid().ToString('N')+'.lnk')
    $shell=New-Object -ComObject WScript.Shell
    $link=$shell.CreateShortcut($script:temporary)
    $link.TargetPath=$Target
    $link.WorkingDirectory=[IO.Path]::GetDirectoryName($Target)
    $link.Description='Meeting for Handy'
    $link.Save()
    # The named mutex serializes cooperating installers; verify old bytes again.
    if ($exists -and (Get-LinkHash $shortcut) -ne $record.SHA256) {exit 3}
    # Hash the new link before publishing, so an ownership record is never left without it.
    $hash=Get-LinkHash $script:temporary
    Move-Item -LiteralPath $script:temporary -Destination $shortcut -Force
    New-Item -Path $owner -Force | Out-Null
    New-ItemProperty -LiteralPath $owner -Name SHA256 -Value $hash -PropertyType String -Force | Out-Null
    New-ItemProperty -LiteralPath $owner -Name Target -Value $Target -PropertyType String -Force | Out-Null
    New-ItemProperty -LiteralPath $owner -Name Link -Value $shortcut -PropertyType String -Force | Out-Null
}
try {
    try {$locked=$mutex.WaitOne(15000)} catch [Threading.AbandonedMutexException] {$locked=$true}
    if (-not $locked) {exit 3}
    $record=Get-ItemProperty -LiteralPath $owner -ErrorAction SilentlyContinue
    $exists=Test-Path -LiteralPath $shortcut
    $owned=$false
    if ($exists -and $record -and $record.Link -eq $shortcut) {
        $file=Get-Item -LiteralPath $shortcut -Force
        if (($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0 -and -not $file.PSIsContainer) {
            $owned=[bool]$record.SHA256 -and (Get-LinkHash $shortcut) -eq $record.SHA256
        }
    }
    if ($Mode -eq 'uninstall') {
        if ($owned -and $record.Target -eq $program) {
            # Keep a Start menu entry for the newest remaining installation.
            $newest=Select-NewestInstallation @(Get-HandyInstallations | Where-Object {$_.Program -ne $program})
            if ($newest) {Publish-HandyLink $newest.Program} else {
                Remove-Item -LiteralPath $shortcut
                Remove-Item -LiteralPath $owner
            }
        }
        exit 0
    }
    if ($exists -and -not $owned) {exit 3}
    if ($exists) {
        # Installing an older version beside a newer one keeps the link on the newer version.
        $newVersion=ConvertTo-HandyVersion $Version
        $current=@(Get-HandyInstallations | Where-Object {$_.Program -eq $record.Target})
        if ($newVersion -and $current.Count -gt 0 -and (Compare-HandyVersion $newVersion $current[0].Version) -le 0) {exit 4}
    }
    Publish-HandyLink $program
    exit 0
} catch {exit 3} finally {
    # Only this script's own unpublished temporary link is removed.
    if ($temporary -and (Test-Path -LiteralPath $temporary)) {Remove-Item -LiteralPath $temporary -ErrorAction SilentlyContinue}
    if ($locked){$mutex.ReleaseMutex()};$mutex.Dispose()
}
