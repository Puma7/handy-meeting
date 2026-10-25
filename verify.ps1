$ErrorActionPreference = 'Stop'
$projectRoot = $PSScriptRoot
$verificationRoot = Join-Path $projectRoot ('.build\verification-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + [guid]::NewGuid().ToString('N').Substring(0, 6))
$env:DOTNET_CLI_HOME = Join-Path $projectRoot '.build\dotnet-home'
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
$env:DOTNET_GENERATE_ASPNET_CERTIFICATE = 'false'
$env:DOTNET_ADD_GLOBAL_TOOLS_TO_PATH = 'false'
Push-Location $projectRoot
try {
    dotnet restore src\HandyMeeting\HandyMeeting.csproj --locked-mode --verbosity minimal
    if ($LASTEXITCODE -ne 0) { throw 'Abhängigkeiten konnten nicht aus der Versionssperre geladen werden.' }
    $appFolder = Join-Path $verificationRoot 'app'
    dotnet publish src\HandyMeeting\HandyMeeting.csproj -c Release -o $appFolder --no-restore --no-self-contained --verbosity minimal
    if ($LASTEXITCODE -ne 0) { throw 'Prüfbuild fehlgeschlagen.' }
    foreach ($check in @(@('--self-test', 'pcm'), @('--notes-checks', 'notes'), @('--template-checks', 'templates'), @('--agent-checks', 'agent'), @('--soak-monitor-checks', 'monitor'))) {
        $outputFolder = Join-Path $verificationRoot $check[1]
        $arguments = @($check[0], ('"' + $outputFolder + '"'))
        $process = Start-Process -FilePath (Join-Path $appFolder 'HandyMeeting.exe') -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -ne 0) { throw ('Prüfung fehlgeschlagen: ' + $check[0] + '. Diagnose: ' + (Join-Path $outputFolder 'failure.json')) }
    }
    $pcm = Get-Content -LiteralPath (Join-Path $verificationRoot 'pcm\tests.json') -Raw | ConvertFrom-Json
    $notes = Get-Content -LiteralPath (Join-Path $verificationRoot 'notes\notes-checks.json') -Raw | ConvertFrom-Json
    $templates = Get-Content -LiteralPath (Join-Path $verificationRoot 'templates\template-checks.json') -Raw | ConvertFrom-Json
    $agent = Get-Content -LiteralPath (Join-Path $verificationRoot 'agent/agent-checks.json') -Raw | ConvertFrom-Json
    [pscustomobject]@{ Success = $true; PcmChecks = $pcm.passed; NotesChecks = $notes.Passed; TemplateChecks = $templates.Passed; AgentChecks = $agent.Passed; MonitorChecks = $true; ResultFolder = $verificationRoot } | ConvertTo-Json
} finally { Pop-Location }
