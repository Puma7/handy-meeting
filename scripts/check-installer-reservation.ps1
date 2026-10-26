#requires -Version 7.4
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
if (-not $IsWindows) { throw 'This native directory check requires Windows.' }
$compiler = & (Join-Path $PSScriptRoot 'prepare-installer.ps1') | ConvertFrom-Json
$script = [IO.File]::ReadAllText((Join-Path $PSScriptRoot 'windows-installer.nsi'))
$match = [regex]::Match($script, '(?ms)^Function ReserveNewDirectory\r?\n.*?^FunctionEnd')
if (-not $match.Success) { throw 'Production directory reservation function missing.' }
$work = Join-Path $root ('.build/reservation-checks-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
function Literal([string]$value) { $value.Replace('$','$$').Replace('"','$\"') }
$fresh = Join-Path $work 'Fresh programs ä/HandyMeeting/1.0.1'
$occupied = Join-Path $work 'occupied'
New-Item -ItemType Directory -Path $occupied | Out-Null
$sentinel = Join-Path $occupied 'user-original.txt'
[IO.File]::WriteAllText($sentinel, 'Synthetic original must survive.')
$blocked = Join-Path $work 'parent-is-file'
[IO.File]::WriteAllText($blocked, 'Synthetic parent file must survive.')
$cases = @(
    @{Name='missing-parents'; Path=$fresh; Expected='1'},
    @{Name='existing-directory'; Path=$occupied; Expected='0'},
    @{Name='parent-is-file'; Path=(Join-Path $blocked 'version'); Expected='0'}
)
foreach ($case in $cases) {
    $exe = Join-Path $work ($case.Name + '.exe')
    $result = Join-Path $work ($case.Name + '.txt')
    $source = Join-Path $work ($case.Name + '.nsi')
    $lines = @('Unicode true', '!include FileFunc.nsh', 'SilentInstall silent', 'RequestExecutionLevel user',
        ('OutFile "' + (Literal $exe) + '"'), ('InstallDir "' + (Literal $case.Path) + '"'),
        $match.Value, 'Section', 'Call ReserveNewDirectory', 'Pop $0',
        ('FileOpen $9 "' + (Literal $result) + '" w'), 'FileWrite $9 "$0"', 'FileClose $9', 'SectionEnd')
    [IO.File]::WriteAllLines($source, $lines, [Text.UTF8Encoding]::new($true))
    & $compiler.compiler /V2 /WX $source
    if ($LASTEXITCODE -ne 0) { throw ('Directory probe compilation failed: ' + $case.Name) }
    $child = Start-Process -FilePath $exe -WindowStyle Hidden -Wait -PassThru
    if ($child.ExitCode -ne 0 -or [IO.File]::ReadAllText($result) -ne $case.Expected) { throw ('Directory reservation regression: ' + $case.Name) }
}
if (-not (Test-Path -LiteralPath $fresh -PathType Container) -or [IO.File]::ReadAllText($sentinel) -ne 'Synthetic original must survive.' -or [IO.File]::ReadAllText($blocked) -ne 'Synthetic parent file must survive.') { throw 'Directory reservation changed a preserved original.' }
[pscustomobject]@{success=$true;checks=3;actualWin32=$true;programInstalled=$false;registryChanged=$false;outputDirectory=$work} | ConvertTo-Json
