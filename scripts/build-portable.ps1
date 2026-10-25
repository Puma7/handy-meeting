param([ValidateSet('linux-x64','osx-arm64','osx-x64','win-x64')][string]$Runtime='linux-x64')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$env:DOTNET_CLI_HOME=Join-Path $root '.build/dotnet-home'
$target=Join-Path $root ('.build/portable/' + $Runtime)
$project=Join-Path $root 'src/HandyMeeting.Portable/HandyMeeting.Portable.csproj'
dotnet restore $project --locked-mode --verbosity minimal
if ($LASTEXITCODE -ne 0) { throw 'Locked restore failed.' }
dotnet publish $project -c Release -r $Runtime --self-contained true -o $target --no-restore --verbosity minimal
if ($LASTEXITCODE -ne 0) { throw 'Portable build failed.' }
if ($Runtime -ne 'win-x64') {
    $native=& (Join-Path $PSScriptRoot 'prepare-native.ps1') -Runtime $Runtime
    $nativeTarget=Join-Path $target 'native'
    New-Item -ItemType Directory -Path $nativeTarget -Force | Out-Null
    Copy-Item -Path (Join-Path $native '*') -Destination $nativeTarget -Recurse -Force
}
Copy-Item -LiteralPath (Join-Path $root 'THIRD_PARTY_NOTICES.txt') -Destination $target
Copy-Item -LiteralPath (Join-Path $root 'LICENSE') -Destination $target
Copy-Item -LiteralPath (Join-Path $root 'licenses') -Destination $target -Recurse -Force
Write-Output $target
