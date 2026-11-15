param(
    [Parameter(Mandatory=$true)][ValidateSet('x86','x64')][string]$Architecture,
    [Parameter(Mandatory=$true)][string]$Output
)
$ErrorActionPreference = 'Stop'
$trimCharacters = [char[]]@([System.IO.Path]::DirectorySeparatorChar,
                             [System.IO.Path]::AltDirectorySeparatorChar)
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..')).TrimEnd($trimCharacters)
$outputRoot = [System.IO.Path]::GetFullPath($Output).TrimEnd($trimCharacters)
$repoPrefix = $root + [System.IO.Path]::DirectorySeparatorChar
if ($outputRoot.Equals($root, [System.StringComparison]::OrdinalIgnoreCase) -or
    $outputRoot.StartsWith($repoPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'Output must be outside the source checkout.'
}
$temporaryRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } elseif ($env:TEMP) { $env:TEMP } else { [System.IO.Path]::GetTempPath() }
$work = Join-Path $temporaryRoot ("MaltoriaNativeBuild-$Architecture-$([guid]::NewGuid().ToString('N'))")
if (Test-Path $work) { throw "Fresh build directory already exists: $work" }
New-Item -ItemType Directory -Path $work | Out-Null
$native = Join-Path $work 'sources'
$build = Join-Path $work 'lunahook'
$hostBuild = Join-Path $work 'host'
$archArg = if ($Architecture -eq 'x86') { 'Win32' } else { 'x64' }
$bits = if ($Architecture -eq 'x86') { '32' } else { '64' }

function Invoke-CheckedNative {
    param([string]$Executable, [string[]]$CommandArguments)
    & $Executable @CommandArguments
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) { throw "$Executable exited with code $exitCode." }
}

Invoke-CheckedNative -Executable 'python' -CommandArguments @(
    (Join-Path $root 'scripts/prepare-native-sources.py'), '--output', $native)
$source = Join-Path $native 'LunaTranslator/src/NativeImpl/LunaHook'
$configureArguments = @(
    '-S', $source, '-B', $build, '-A', $archArg,
    '-DBUILD_HOST=ON', '-DBUILD_HOOK=ON', '-DBUILD_GUI=OFF',
    '-DWIN10ABOVE=ON', '-DUSE_VC_LTL=OFF',
    "-DFETCHCONTENT_SOURCE_DIR_MINHOOK=$(Join-Path $native 'MinHook')",
    "-DFETCHCONTENT_SOURCE_DIR_UCHARDET=$(Join-Path $native 'uchardet')",
    '-DFETCHCONTENT_FULLY_DISCONNECTED=ON')
Invoke-CheckedNative -Executable 'cmake' -CommandArguments $configureArguments
Invoke-CheckedNative -Executable 'cmake' -CommandArguments @(
    '--build', $build, '--config', 'Release', '--target',
    'LunaHostDll', 'LunaHook', 'LunaTmpFontLoader', '--parallel')

$nlohmannInclude = Join-Path $native 'nlohmann-json/single_include'
$hostSource = Join-Path $root 'Native/MaltoriaLunaHost'
Invoke-CheckedNative -Executable 'cmake' -CommandArguments @(
    '-S', $hostSource, '-B', $hostBuild, '-A', $archArg,
    "-DNLOHMANN_JSON_INCLUDE_DIR=$nlohmannInclude")
Invoke-CheckedNative -Executable 'cmake' -CommandArguments @(
    '--build', $hostBuild, '--config', 'Release', '--target',
    'MaltoriaLunaHost', 'MaltoriaUnicodeFixture', '--parallel')

New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null
$payload = Join-Path $outputRoot $Architecture
if (Test-Path $payload) { Remove-Item -LiteralPath $payload -Recurse -Force }
New-Item -ItemType Directory -Force -Path $payload | Out-Null
$binaryDir = Join-Path $source 'builds/Release_win10'
foreach ($name in @("LunaHost$bits.dll", "LunaHook$bits.dll", 'LunaTmpFontLoader.dll')) {
    $file = Join-Path $binaryDir $name
    if (!(Test-Path $file -PathType Leaf)) { throw "Missing expected build output: $file" }
    Copy-Item $file $payload
}

$hostBinary = Join-Path $hostBuild "Release/MaltoriaLunaHost$bits.exe"
$fixtureBinary = Join-Path $hostBuild 'Release/MaltoriaUnicodeFixture.exe'
foreach ($file in @($hostBinary, $fixtureBinary)) {
    if (!(Test-Path $file -PathType Leaf)) { throw "Missing expected build output: $file" }
    Copy-Item $file $payload
}
Write-Output "Staged fresh $Architecture native build under $payload"
