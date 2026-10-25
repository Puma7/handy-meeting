param([Parameter(Mandatory=$true)][ValidateSet('linux-x64','osx-arm64','osx-x64')][string]$Runtime)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$assets = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'native-assets.json') -Raw | ConvertFrom-Json
$asset = $assets.$Runtime
$target = Join-Path $root ('native/portable-' + $Runtime)
$archive = Join-Path $root ('.build/native-download/' + $asset.file)
New-Item -ItemType Directory -Path (Split-Path $archive -Parent),$target -Force | Out-Null
if (-not (Test-Path -LiteralPath $archive)) { Invoke-WebRequest -Uri ('https://github.com/handy-computer/transcribe.cpp/releases/download/v0.4.1/' + $asset.file) -OutFile $archive }
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $asset.sha256) { throw 'Native runtime checksum mismatch.' }
$entries = & tar -tzf $archive
if ($LASTEXITCODE -ne 0) { throw 'Cannot read native archive.' }
foreach ($entry in $entries) { if ($entry -match '(^|/)\.\.(/|$)' -or $entry.StartsWith('/') -or $entry.Contains('\')) { throw 'Unsafe native archive path.' } }
& tar -xzf $archive -C $target --strip-components=1
if ($LASTEXITCODE -ne 0) { throw 'Native runtime extraction failed.' }
Write-Output $target
