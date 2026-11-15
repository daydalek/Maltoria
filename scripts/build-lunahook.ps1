param(
    [Parameter(Mandatory=$true)][ValidateSet('x86','x64')][string]$Architecture,
    [Parameter(Mandatory=$true)][string]$Output
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$native = Join-Path $env:RUNNER_TEMP "native-$Architecture"
$build = Join-Path $env:RUNNER_TEMP "lunahook-$Architecture"
$archArg = if ($Architecture -eq 'x86') { 'Win32' } else { 'x64' }
python (Join-Path $root 'scripts/prepare-native-sources.py') --output $native
$source = Join-Path $native 'LunaTranslator/src/NativeImpl/LunaHook'
cmake -S $source -B $build -A $archArg `
    -DBUILD_HOST=ON -DBUILD_HOOK=ON -DBUILD_GUI=OFF -DWIN10ABOVE=ON -DUSE_VC_LTL=OFF `
    "-DFETCHCONTENT_SOURCE_DIR_MINHOOK=$(Join-Path $native 'MinHook')" `
    "-DFETCHCONTENT_SOURCE_DIR_UCHARDET=$(Join-Path $native 'uchardet')" `
    -DFETCHCONTENT_FULLY_DISCONNECTED=ON
cmake --build $build --config Release --target LunaHostDll LunaHook LunaTmpFontLoader --parallel
$payload = Join-Path $Output $Architecture
New-Item -ItemType Directory -Force -Path $payload | Out-Null
$binaryDir = Join-Path $source 'builds/Release_win10'
$bits = if ($Architecture -eq 'x86') { '32' } else { '64' }
foreach ($name in @("LunaHost$bits.dll", "LunaHook$bits.dll", 'LunaTmpFontLoader.dll')) {
    $file = Join-Path $binaryDir $name
    if (!(Test-Path $file -PathType Leaf)) { throw "Missing expected build output: $file" }
    Copy-Item $file $payload
}
