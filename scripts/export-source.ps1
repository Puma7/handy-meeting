$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$stamp=Get-Date -Format 'yyyyMMdd-HHmmss'
$stage=Join-Path $root ('.build/source-export-' + $stamp)
New-Item -ItemType Directory -Path $stage | Out-Null
$roots=@('src','scripts','platform','assets','.github','licenses')
$names=@('AGENTS.md','README.md','README.de.md','CONTRIBUTING.md','SECURITY.md','CHANGELOG.md','BUILD_STATUS.md','MASTERPLAN.md','TEST_REPORT.md','DAUERTEST.md','PORTABILITY.md','AGENT_API.md','REPOSITORY_READY.md','LICENSE','THIRD_PARTY_NOTICES.txt','System.Numerics.Tensors.LICENSE.txt','System.Numerics.Tensors.NOTICES.txt','.gitignore','global.json','NuGet.config','build.ps1','verify.ps1','setup-native.ps1','HandyMeeting starten.cmd')
$files=@()
foreach ($name in $names) { $path=Join-Path $root $name; if (Test-Path -LiteralPath $path -PathType Leaf) { $files+=Get-Item -LiteralPath $path } }
foreach ($folder in $roots) {
 $path=Join-Path $root $folder
 if (Test-Path -LiteralPath $path) { $files+=Get-ChildItem -LiteralPath $path -File -Recurse -Force | Where-Object { $_.FullName -notmatch '[\\/](bin|obj|node_modules|\.git)[\\/]' } }
}
$manifest=foreach ($file in $files) {
 $relative=[IO.Path]::GetRelativePath($root,$file.FullName)
 if ($relative.StartsWith('..') -or [IO.Path]::IsPathRooted($relative) -or $relative -match '[\\/](Sessions|data|native|packages|dist[^\\/]*)[\\/]') { throw ('Excluded export path: ' + $relative) }
 if ($file.Extension -in @('.pfx','.p12','.key','.gguf','.wav','.mp3','.dmg','.deb','.dll','.exe')) { throw ('Unexpected export file: ' + $relative) }
 if ($file.Extension -notin @('.cs','.xaml','.axaml','.csproj','.props','.targets','.json','.config','.manifest','.md','.ps1','.py','.sh','.swift','.svg','.png','.ico','.yml','.yaml','.txt','.cmd') -and $file.Name -notin $names -and $file.Name -ne 'CODEOWNERS') { throw ('File type not approved for export: ' + $relative) }
 if ($file.Extension -notin @('.png','.ico')) {
   $text=[IO.File]::ReadAllText($file.FullName)
   if ($text -match '(ghp_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,}|sk-[A-Za-z0-9]{24,}|-----BEGIN [A-Z ]*PRIVATE KEY-----)') { throw ('Possible credential in export: ' + $relative) }
 }
 $destination=Join-Path $stage $relative
 New-Item -ItemType Directory -Path (Split-Path $destination -Parent) -Force | Out-Null
 Copy-Item -LiteralPath $file.FullName -Destination $destination
 [pscustomobject]@{ Path=$relative.Replace('\','/'); SHA256=(Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash; Bytes=$file.Length }
}
$manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $stage 'SOURCE_MANIFEST.json') -Encoding utf8
$zip=Join-Path $root ('.build/HandyMeeting-source-' + $stamp + '.zip')
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::CreateFromDirectory($stage,$zip)
[pscustomobject]@{ Files=$manifest.Count; SourceFolder=$stage; Archive=$zip; SHA256=(Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash; UserRecordingsIncluded=$false; Published=$false } | ConvertTo-Json
