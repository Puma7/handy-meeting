#requires -Version 7.4
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$pin = (Get-Content (Join-Path $PSScriptRoot 'installer-tools.json') -Raw | ConvertFrom-Json).nsis
$tools = Join-Path $root '.build/installer-tools'
New-Item -ItemType Directory -Path $tools -Force | Out-Null
$archive = Join-Path $tools ('nsis-' + $pin.version + '.zip')
if (-not (Test-Path -LiteralPath $archive)) {
    Invoke-WebRequest -Uri $pin.url -OutFile $archive
}
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $pin.sha256) { throw 'Pinned installer compiler archive differs; existing files were preserved.' }
$expanded = Join-Path $tools ('nsis-' + $pin.version)
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [IO.Compression.ZipFile]::OpenRead($archive)
try {
    foreach ($entry in $zip.Entries) {
        $name = $entry.FullName.Replace('\','/')
        if ($name.StartsWith('/') -or $name.Contains(':') -or ($name.Split('/') -contains '..') -or (($entry.ExternalAttributes -shr 16) -band 0xF000) -eq 0xA000) { throw 'Unsafe compiler archive entry.' }
    }
    if (-not (Test-Path -LiteralPath $expanded)) { Expand-Archive -LiteralPath $archive -DestinationPath $tools }
    # Compare all retained compiler inputs to the pinned archive, including stubs/includes.
    foreach ($entry in $zip.Entries | Where-Object { $_.Name }) {
        $file = Join-Path $tools $entry.FullName
        $input = $entry.Open()
        try { $expected = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($input)).ToLowerInvariant() } finally { $input.Dispose() }
        if (-not (Test-Path -LiteralPath $file -PathType Leaf) -or (Get-FileHash -LiteralPath $file).Hash.ToLowerInvariant() -ne $expected) { throw 'Installer compiler input changed; no files were replaced.' }
    }
} finally { $zip.Dispose() }
[pscustomobject]@{ version=$pin.version; archiveSHA256=$pin.sha256; compiler=(Join-Path $expanded 'makensis.exe') } | ConvertTo-Json -Compress
