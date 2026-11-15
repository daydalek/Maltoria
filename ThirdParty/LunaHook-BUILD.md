# LunaTranslator native source build and process inventory helper

The native source lock is in `manifest.json`; archives are committed under
`sources/native/`. `python3 scripts/prepare-native-sources.py --output <empty-dir>`
checks SHA-256 before extraction, preflights every archive path/root, rejects
unsafe members, and applies the single recorded MinHook pin patch. No source is
fetched during Windows builds.

On Windows 2022 with Visual Studio 2022 C++ and .NET Framework 4.x:

```powershell
pwsh -File scripts/build-lunahook.ps1 -Architecture x86 -Output $env:RUNNER_TEMP\MaltoriaNative
pwsh -File scripts/build-lunahook.ps1 -Architecture x64 -Output $env:RUNNER_TEMP\MaltoriaNative
```

The script uses CMake/MSVC with `BUILD_HOST=ON`, `BUILD_HOOK=ON`,
`BUILD_GUI=OFF`, `WIN10ABOVE=ON`, `USE_VC_LTL=OFF`, and fully disconnected,
locally prepared MinHook/uchardet sources. It also builds the separately authored
process-inventory helper and GDI fixture. Outputs are `LunaHost32/64.dll`,
`LunaHook32/64.dll`, `LunaTmpFontLoader.dll`, `MaltoriaLunaHost32/64.exe`, and
`MaltoriaUnicodeFixture.exe`.

The helper is deliberately process-inventory-only in this revision. It does not
load LunaHook into a target, call LunaHost attachment APIs, capture hook text, or
perform attach/detach/hook insertion; those commands return
`unsupported_operation`. Windows CI starts only the authored Unicode fixture,
checks its process identity over IPC, and verifies the fixture remains alive.
It does not run a game or prove hook capture.

## Local IPC schema

The host reads `MALTORIA_HOOK_PORT`, `MALTORIA_HOOK_TOKEN`, and
`MALTORIA_HOOK_SESSION` from its environment, then connects only to
`127.0.0.1:<port>`. The token is not a command-line argument and is never
printed. Each UTF-8 NDJSON frame, including its newline, is limited to 64 KiB;
connect, send, and receive operations have finite timeouts.

The first host frame is:

```json
{"type":"hello","version":1,"token":"<secret>","session":"<session-id>","arch":"x64"}
```

After validating the token/session/version, the service acknowledges:

```json
{"type":"hello","version":1,"session":"<session-id>","ok":true}
```

Every command and response carries the same session. Request IDs are strings
and are echoed by responses:

```json
{"type":"command","id":"req-1","session":"<session-id>","command":"listProcesses"}
```

The `listProcesses` result has `processes` and `truncated`. Each process record
contains `guestPID` (integer), `exePath` (full UTF-8 image path), `arch`
(`x86`, `x64`, `arm64`, or `unknown`), and `creationTime` (16-digit FILETIME hex
string). A successful reply has the form:

```json
{"type":"response","id":"req-1","session":"<session-id>","ok":true,"result":{"processes":[],"truncated":false}}
```

`attach`, `detach`, and `insertHook` receive an id-correlated error response:

```json
{"type":"response","id":"req-2","session":"<session-id>","ok":false,"error":{"code":"unsupported_operation","message":"Target attachment and hook operations are disabled in this helper."}}
```

`shutdown` is acknowledged before the host closes the socket. EOF, malformed
frames, an oversized frame, or a session mismatch closes the helper. This
process-inventory build does not emit thread/text callback events.

## Reproducible source payload

Each Windows artifact contains `sources/Maltoria/`, a path-preserving copy of
the helper source, build/prepare/validation scripts, workflow, dependency
archives, all notices, and build instructions. `SOURCE-COMMIT.txt` identifies the
exact Maltoria commit. `scripts/package-native-sources.py` validates the locked
source hashes before staging that payload. The workflow runs on feature pushes,
main pushes, pull requests to main, version tags, and as a reusable
`workflow_call`; it only has `contents: read` permission and does not publish a
release.
