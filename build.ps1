$ErrorActionPreference = 'Stop'
$projectRoot = $PSScriptRoot
$env:DOTNET_CLI_HOME = Join-Path $projectRoot '.build\dotnet-home'
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
$env:DOTNET_GENERATE_ASPNET_CERTIFICATE = 'false'
$env:DOTNET_ADD_GLOBAL_TOOLS_TO_PATH = 'false'
Push-Location $projectRoot
try {
    & (Join-Path $projectRoot 'setup-native.ps1')
    $projectFile = Join-Path $projectRoot 'src\HandyMeeting\HandyMeeting.csproj'
    $version = ([xml](Get-Content -LiteralPath $projectFile -Raw)).Project.PropertyGroup.Version
    $outputName = 'dist-' + $version
    $outputPath = Join-Path $projectRoot $outputName
    dotnet publish $projectFile --configuration Release --output $outputPath --no-self-contained --verbosity minimal
    if ($LASTEXITCODE -ne 0) { throw 'Der Build ist fehlgeschlagen.' }
    $launcher = "@echo off`r`nif not exist `"%~dp0$outputName\HandyMeeting.exe`" (`r`n  echo Bitte zuerst build.ps1 ausfuehren.`r`n  pause`r`n  exit /b 1`r`n)`r`nstart `"`" `"%~dp0$outputName\HandyMeeting.exe`"`r`n"
    [IO.File]::WriteAllText((Join-Path $projectRoot 'HandyMeeting starten.cmd'), $launcher, [Text.Encoding]::ASCII)
    Write-Host ('Startbereit: ' + (Join-Path $outputPath 'HandyMeeting.exe'))
} finally { Pop-Location }
