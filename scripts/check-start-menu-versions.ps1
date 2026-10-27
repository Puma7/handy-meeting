# Checks the Start menu version ordering of manage-start-menu.ps1 without touching links,
# the registry or installations: only its version functions are loaded, from the parsed file.
$ErrorActionPreference = 'Stop'
$path = Join-Path $PSScriptRoot 'manage-start-menu.ps1'
$tokens = $null
$errors = $null
$ast = [Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors)
if ($errors.Count -gt 0) { throw ('manage-start-menu.ps1 does not parse: ' + $errors[0].Message) }
$names = @('ConvertTo-HandyVersion', 'Compare-HandyVersion', 'Select-NewestInstallation')
$functions = @($ast.FindAll({ param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $names -contains $node.Name }, $true))
if ($functions.Count -ne $names.Count) { throw 'Start menu version functions missing.' }
foreach ($function in $functions) { . ([scriptblock]::Create($function.Extent.Text)) }

# Each version sorts strictly below the next one.
$ordered = @('1.0.0-alpha', '1.0.0-alpha.1', '1.0.0-alpha.beta', '1.0.0-beta', '1.0.0-beta.2', '1.0.0-beta.9',
    '1.0.0-beta.10', '1.0.0-beta.11', '1.0.0-rc.1', '1.0.0', '1.0.2', '1.0.3-beta.1', '1.0.3-beta.2', '1.0.3', '1.0.10', '1.1.0', '2.0.0')
for ($i = 0; $i -lt $ordered.Count - 1; $i++) {
    $lower = ConvertTo-HandyVersion $ordered[$i]
    $higher = ConvertTo-HandyVersion $ordered[$i + 1]
    if (-not $lower -or -not $higher) { throw ('Version not recognized: ' + $ordered[$i]) }
    if ((Compare-HandyVersion $lower $higher) -ne -1 -or (Compare-HandyVersion $higher $lower) -ne 1) {
        throw ($ordered[$i] + ' must sort below ' + $ordered[$i + 1])
    }
}
if ((Compare-HandyVersion (ConvertTo-HandyVersion '1.0.3-beta.2') (ConvertTo-HandyVersion '1.0.3-beta.2')) -ne 0) { throw 'Equal versions must compare equal.' }
foreach ($invalid in @('', '1.0', 'v1.0.3', '1.0.3-', '1.0.3-beta..1', '1.0.3 beta')) {
    if ($null -ne (ConvertTo-HandyVersion $invalid)) { throw ('Invalid version accepted: ' + $invalid) }
}

# The newest side-by-side installation keeps the link, in any listing order.
$installations = @(
    [pscustomobject]@{ Version = (ConvertTo-HandyVersion '1.0.3-beta.9'); Program = 'beta9' },
    [pscustomobject]@{ Version = (ConvertTo-HandyVersion '1.0.3-beta.10'); Program = 'beta10' },
    [pscustomobject]@{ Version = (ConvertTo-HandyVersion '1.0.2'); Program = 'release102' }
)
if ((Select-NewestInstallation $installations).Program -ne 'beta10') { throw 'beta.10 must be chosen over beta.9.' }
[array]::Reverse($installations)
if ((Select-NewestInstallation $installations).Program -ne 'beta10') { throw 'The choice must not depend on order.' }
if ((Select-NewestInstallation @($installations[2])).Program -ne 'beta9') { throw 'A single installation must be chosen.' }
if ($null -ne (Select-NewestInstallation @())) { throw 'No installation must choose nothing.' }
Write-Output 'Start menu version ordering checks passed.'
