$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
# Source exports are a strict allowlist. Inspect each entry before reading or
# descending into it; lexical relative paths alone do not contain linked files.
function Assert-ExportEntry([IO.FileSystemInfo]$item) {
 $relative=[IO.Path]::GetRelativePath($root,$item.FullName)
 if ([IO.Path]::IsPathRooted($relative) -or $relative -eq '..' -or $relative -match '^\.\.[\\/]') { throw ('Outside source export root: ' + $relative) }
 $target=$item.ResolveLinkTarget($true)
 $resolved=if ($null -eq $target) { $item.FullName } else { $target.FullName }
 $resolvedRelative=[IO.Path]::GetRelativePath($root,$resolved)
 if ([IO.Path]::IsPathRooted($resolvedRelative) -or $resolvedRelative -eq '..' -or $resolvedRelative -match '^\.\.[\\/]') { throw ('Outside source export link target: ' + $relative) }
 if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or $item.LinkType) { throw ('Linked source export entry is not allowed: ' + $relative) }
}
function Get-ExportFiles([string]$directory) {
 $item=Get-Item -LiteralPath $directory -Force
 Assert-ExportEntry $item
 if (-not $item.PSIsContainer) { throw ('Source export directory expected: ' + $item.Name) }
 foreach ($entry in Get-ChildItem -LiteralPath $directory -Force) {
  $relative=[IO.Path]::GetRelativePath($root,$entry.FullName)
  # Do not inspect or traverse private/generated entries at any depth.
  if ($relative -match '(^|[\\/])(bin|obj|node_modules|__pycache__|\.git|target|dist|gen|\.build|\.vite|test-results|playwright-report)([\\/]|$)' -or $entry.Extension -eq '.tsbuildinfo') { continue }
  Assert-ExportEntry $entry
  if ($entry.PSIsContainer) { Get-ExportFiles $entry.FullName } else { $entry }
 }
}
$stamp=Get-Date -Format 'yyyyMMdd-HHmmss'
$stage=Join-Path $root ('.build/source-export-' + $stamp)
New-Item -ItemType Directory -Path $stage | Out-Null
$roots=@('src','scripts','platform','assets','.github','licenses','crates','apps','docs','.cargo')
$names=@('AGENTS.md','README.md','README.de.md','CONTRIBUTING.md','SECURITY.md','CHANGELOG.md','BUILD_STATUS.md','MASTERPLAN.md','TEST_REPORT.md','DAUERTEST.md','PORTABILITY.md','AGENT_API.md','REPOSITORY_READY.md','LICENSE','THIRD_PARTY_NOTICES.txt','System.Numerics.Tensors.LICENSE.txt','System.Numerics.Tensors.NOTICES.txt','.gitignore','.gitattributes','Cargo.toml','Cargo.lock','global.json','NuGet.config','build.ps1','verify.ps1','setup-native.ps1','HandyMeeting starten.cmd','HandyMeeting Rust Beta starten.cmd','HandyMeeting 1.0.0-beta.2 starten.cmd','HandyMeeting 1.0.0-beta.3 starten.cmd','HandyMeeting 1.0.0-beta.4 starten.cmd')
$files=@()
$names+='rust-toolchain.toml'
$versionFile=Join-Path $root 'apps/desktop/package.json'
# This configuration is read before enumeration; validate its full allowlisted
# chain too, so an aliased apps/desktop directory cannot redirect the first read.
foreach ($part in @('apps','apps/desktop','apps/desktop/package.json')) { Assert-ExportEntry (Get-Item -LiteralPath (Join-Path $root $part) -Force) }
$version=(Get-Content -LiteralPath $versionFile -Raw | ConvertFrom-Json).version
if ($version -notmatch '^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?$') { throw 'Invalid source export version.' }
$names+='HandyMeeting 1.0.0 starten.cmd'
$names+=('HandyMeeting ' + $version + ' starten.cmd')
$names+=('Meeting for Handy ' + $version + ' starten.cmd')
foreach ($name in $names) {
 $path=Join-Path $root $name
 $item=Get-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
 if ($null -ne $item) { Assert-ExportEntry $item; if ($item.PSIsContainer) { throw ('Source export file expected: ' + $name) }; $files+=$item }
}
foreach ($folder in $roots) {
 $path=Join-Path $root $folder
 $item=Get-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
 if ($null -ne $item) { $files+=Get-ExportFiles $path }
}
$manifest=foreach ($file in $files) {
 # Recheck the selected entry immediately before its contents are read/copied.
 Assert-ExportEntry (Get-Item -LiteralPath $file.FullName -Force)
 $relative=[IO.Path]::GetRelativePath($root,$file.FullName)
 if ($relative.StartsWith('..') -or [IO.Path]::IsPathRooted($relative) -or $relative -match '[\\/](Sessions|data|native|packages|dist[^\\/]*)[\\/]') { throw ('Excluded export path: ' + $relative) }
 if ($file.Extension -in @('.pfx','.p12','.key','.gguf','.wav','.mp3','.dmg','.deb','.dll','.exe')) { throw ('Unexpected export file: ' + $relative) }
 if ($file.Extension -notin @('.cs','.xaml','.axaml','.csproj','.props','.targets','.json','.config','.manifest','.md','.ps1','.py','.sh','.swift','.svg','.png','.ico','.yml','.yaml','.txt','.cmd','.rs','.toml','.lock','.html','.css','.ts','.tsx','.js','.mjs','.nsi') -and $file.Name -notin $names -and $file.Name -ne 'CODEOWNERS') { throw ('File type not approved for export: ' + $relative) }
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
