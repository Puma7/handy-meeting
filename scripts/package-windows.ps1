$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$version=([xml](Get-Content -LiteralPath (Join-Path $root 'src/HandyMeeting/HandyMeeting.csproj') -Raw)).Project.PropertyGroup.Version
$dist=Join-Path $root ('dist-'+$version)
if (-not (Test-Path -LiteralPath (Join-Path $dist 'HandyMeeting.exe'))) { throw 'Run build.ps1 first.' }
$verified=Get-ChildItem -LiteralPath (Join-Path $root '.build') -Directory -Filter 'verification-*' | Sort-Object Name -Descending | Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName 'agent/agent-checks.json') } | Select-Object -First 1
if (-not $verified) { throw 'Run verify.ps1 before packaging.' }
foreach ($file in @('HandyMeeting.exe','HandyMeeting.dll')) {
 if ((Get-FileHash -LiteralPath (Join-Path $dist $file) -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath (Join-Path $verified.FullName ('app/'+$file)) -Algorithm SHA256).Hash) { throw 'Program differs from verified build.' }
}
$stage=Join-Path $root ('.build/windows-package-'+[guid]::NewGuid().ToString('N'))
$appRoot=Join-Path $stage 'HandyMeeting'
$program=Join-Path $appRoot ('dist-'+$version)
New-Item -ItemType Directory -Path $program -Force | Out-Null
foreach ($file in Get-ChildItem -LiteralPath $dist -File -Recurse) {
 $relative=[IO.Path]::GetRelativePath($dist,$file.FullName)
 if ($file.Extension -eq '.pdb') { continue }
 if ($relative -match '(^|[\\/])(Sessions|data)([\\/]|$)' -or $file.Extension -in @('.wav','.gguf','.pfx','.p12','.key')) { throw 'Unexpected private data in program directory.' }
 $target=Join-Path $program $relative; New-Item -ItemType Directory -Path (Split-Path $target -Parent) -Force | Out-Null; Copy-Item -LiteralPath $file.FullName -Destination $target
}
Copy-Item -LiteralPath (Join-Path $root 'HandyMeeting starten.cmd') -Destination (Join-Path $appRoot 'Start HandyMeeting.cmd')
Copy-Item -LiteralPath (Join-Path $root 'LICENSE') -Destination $appRoot
$instructions="HandyMeeting $version - Windows prototype`n`nExtract this entire archive. Install .NET Desktop Runtime 10 and Handy with a compatible streaming model, then double-click Start HandyMeeting.cmd.`nAudio and transcripts stay in the extracted application folder. Keep the full directory together. This is an independent prototype, not an official Handy release.`n"
[IO.File]::WriteAllText((Join-Path $appRoot 'START-HERE.txt'),$instructions,[Text.UTF8Encoding]::new($false))
$packages=Join-Path $root 'packages'; New-Item -ItemType Directory -Path $packages -Force | Out-Null
$zip=Join-Path $packages ('HandyMeeting-'+$version+'-windows-x64.zip')
if (Test-Path -LiteralPath $zip) { throw 'Package already exists; preserve it and choose a new version.' }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::CreateFromDirectory($stage,$zip)
$hash=(Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText(($zip+'.sha256'),($hash+'  '+[IO.Path]::GetFileName($zip)+"`n"),[Text.UTF8Encoding]::new($false))
[pscustomobject]@{Package=$zip;SHA256=$hash;VerifiedProgram=$true;UserRecordingsIncluded=$false} | ConvertTo-Json
