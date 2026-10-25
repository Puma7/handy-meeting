$ErrorActionPreference = 'Stop'
$nativeRoot = Join-Path $PSScriptRoot 'native\transcribe-0.4.1'
if ((Test-Path -LiteralPath (Join-Path $nativeRoot 'transcribe.dll')) -and (Test-Path -LiteralPath (Join-Path $nativeRoot 'contract.json'))) { return }
$downloadRoot = Join-Path $PSScriptRoot '.build\native-download'
$archivePath = Join-Path $downloadRoot 'transcribe-0.4.1-windows.tar.gz'
New-Item -ItemType Directory -Path $downloadRoot, $nativeRoot -Force | Out-Null
$source = 'https://github.com/handy-computer/transcribe.cpp/releases/download/v0.4.1/transcribe-native-0.4.1-windows-x86_64-cpu-vulkan.tar.gz'
Invoke-WebRequest -Uri $source -OutFile $archivePath
$expectedHash = '272EB97795048CCB557C85BB9DD44C9E530E1C928443114C39816A8A5F76269D'
if ((Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash -ne $expectedHash) { throw 'Die Prüfsumme der Streaming-Laufzeit stimmt nicht.' }
$archiveEntries = & tar -tzf $archivePath
if ($LASTEXITCODE -ne 0) { throw 'Die Streaming-Laufzeit kann nicht gelesen werden.' }
foreach ($entry in $archiveEntries) {
    if (-not $entry.StartsWith('transcribe-native-windows-x86_64-cpu-vulkan/') -or $entry -match '(^|/)\.\.(/|$)' -or $entry.Contains('\')) { throw 'Unerwarteter Archivpfad.' }
}
& tar -xzf $archivePath -C $nativeRoot --strip-components=1
if ($LASTEXITCODE -ne 0) { throw 'Die Streaming-Laufzeit konnte nicht entpackt werden.' }
