# LunaTranslator native source build and capture helper

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
capture helper and GDI fixture. Outputs are `LunaHost32/64.dll`,
`LunaHook32/64.dll`, `LunaTmpFontLoader.dll`, `MaltoriaLunaHost32/64.exe`, and
`MaltoriaUnicodeFixture.exe`.

The helper uses the pinned LunaHost C ABI and normal same-bitness Windows DLL
loading for an explicitly selected process. It validates the full image path,
creation time and architecture on the same open process handle before and after
attachment. It never elevates, copies DLLs into the game, or terminates a game or
wineserver. Existing hooks owned by another host are rejected. DLL loading and
connection waits are bounded; a timed-out loader's argument is deliberately
left allocated because freeing memory still in use would damage the target.
Windows CI captures Japanese Unicode text from our authored GDI fixture and
checks identity rejection, detach and fixture survival. It is not evidence of
Wine or real-game compatibility.

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

`attach` takes the exact process record selected from `listProcesses`:

```json
{"type":"command","id":"req-2","session":"<session-id>","command":"attach","params":{"target":{"guestPID":72,"exePath":"C:\\Games\\game.exe","arch":"x64","creationTime":"01DC123456789ABC"}}}
```

The response result contains `target`. Failures have `ok:false` and
`error:{code,message}`; errors never contain dialogue. Only one target can be
attached per helper. `detach` cleans up that attachment; `insertHook` takes
`params:{hookCode:"..."}` for an explicit user-provided hook code. No automatic
memory search or embedding is enabled. `ping` returns an empty result: send it
at least every 15 seconds to stay within the 30-second receive timeout.

Asynchronous events may arrive between any responses; all carry `session`:

- `process`: `action` is `connected` or `disconnected`, with `guestPID`.
- `thread`: `action` is `created` or `removed`, with `thread` below.
- `text`: monotonic `sequence`, UTF-8 `text`, and `thread`.
- `error`: safe `code`; `events_dropped` also includes `dropped`.

The `thread` object contains `guestPID`, `hookCode`, `hookName`, and
`addressHex`, `contextHex`, `context2Hex` as `0x`-prefixed hexadecimal strings.
Callbacks only copy bounded data and try to enqueue it. The writer has a
256-event queue and reports overflow; dialogue is never silently truncated or
logged. Original LunaHost diagnostic text is discarded because it can contain
game text. The native service selects which text threads become translations.

`shutdown` is acknowledged before closing. EOF, malformed/oversized frames or
a session mismatch also detach and close. A failed attach can leave a pending
loader; stop that helper rather than retrying indefinitely. Only the helper's
own sockets, handles and attachment are cleaned up.

## Reproducible source payload

Each Windows artifact contains `sources/Maltoria/`, a path-preserving copy of
the helper source, build/prepare/validation scripts, workflow, dependency
archives, all notices, and build instructions. `SOURCE-COMMIT.txt` identifies the
exact Maltoria commit. `scripts/package-native-sources.py` validates the locked
source hashes before staging that payload. The workflow runs on feature pushes,
main pushes, pull requests to main, version tags, and as a reusable
`workflow_call`; it only has `contents: read` permission and does not publish a
release.
