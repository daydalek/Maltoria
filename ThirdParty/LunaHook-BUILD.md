# LunaTranslator native source build

The native source lock is in `manifest.json`; archives are committed under
`sources/native/`. `python3 scripts/prepare-native-sources.py --output <empty-dir>`
checks SHA-256 before extraction, rejects unsafe archive entries, and applies the
single recorded MinHook pin patch. No source is fetched during Windows builds.

On Windows 2022 with Visual Studio 2022 C++ and .NET Framework 4.x:

```powershell
pwsh -File scripts/build-lunahook.ps1 -Architecture x86 -Output $env:RUNNER_TEMP\MaltoriaNative
pwsh -File scripts/build-lunahook.ps1 -Architecture x64 -Output $env:RUNNER_TEMP\MaltoriaNative
```

The script uses CMake/MSVC with `BUILD_HOST=ON`, `BUILD_HOOK=ON`,
`BUILD_GUI=OFF`, `WIN10ABOVE=ON`, `USE_VC_LTL=OFF`, and fully disconnected,
locally prepared MinHook/uchardet sources. Outputs are `LunaHost32/64.dll`,
`LunaHook32/64.dll`, and `LunaTmpFontLoader.dll`. The Windows workflow builds
each architecture independently and uploads those binaries together with these
exact source archives, preserved license texts, manifest, patch record, and
build instructions. No target attach or game execution occurs in this workflow.
