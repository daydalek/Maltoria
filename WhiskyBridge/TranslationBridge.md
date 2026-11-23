# Translation bridge DTOs

The portable `translation.*` service is available on macOS and Linux. It manages provider profiles, credentials, model metadata, connection tests, translation settings and Wine capture sessions. All frontend traffic remains strict request/response NDJSON; native capture events never appear unsolicited on bridge stdout.

## Methods

| Method | Parameters | Result |
| --- | --- | --- |
| `translation.providers/list` | none | Array of provider DTOs |
| `translation.providers/save` | `{ "provider": ProviderConfiguration }` | Provider DTO |
| `translation.providers/delete` | `{ "providerID": UUID }` | `{ "deleted": true }` |
| `translation.credential.set` | `{ "providerID": UUID, "key": string }` | `{ "credentialReference": string, "storage": "persistentKeychain" or "sessionOnly" }` |
| `translation.credential.delete` | `{ "providerID": UUID }` | `{ "deleted": boolean }` |
| `translation.models` | `{ "providerID": UUID }` | Array of `ModelDescriptor` |
| `translation.modelInfo` | `{ "providerID": UUID, "modelID"?: string }` | `ModelDescriptor` |
| `translation.testConnection` | `{ "providerID": UUID }` | `{ "translation": string }` |
| `translation.settings/get` | none | Settings envelope |
| `translation.settings/save` | `{ "settings": TranslationSettings, "remoteProvidersEnabled"?: boolean }` | Settings envelope |
| `translation.local.status` | none | Local runtime state, mirror config, and catalog models (with live download progress/speed/ETA) |
| `translation.local.setConfig` | `{ "useChinaMirror": boolean }` | Updated local status envelope |
| `translation.local.download` | `{ "modelID": string, "useChinaMirror"?: boolean }` | `{ "jobID": string, "modelID": string }` |
| `translation.local.cancelDownload` | `{ "modelID": string }` | Updated local status envelope |
| `translation.local.delete` | `{ "modelID": string }` | Updated local status envelope |
| `translation.local.unload` | none | Updated local status envelope after terminating idle/active `llama-server` |

Provider DTOs contain `id`, `name`, `kind`, `baseURL`, `modelID`, default languages, optional `modelMetadata`, `openAIChatTokenLimitField`, explicit provider `capabilities`, and `hasCredential`. They never contain a credential or credential reference. Provider save accepts the core `ProviderConfiguration` shape; a newly supplied credential reference is ignored, and an existing profile's reference is preserved. Set and delete credentials by provider ID.

The settings envelope is `{ "settings": TranslationSettings, "remoteProvidersEnabled": boolean, "credentialStorage": string }`. `remoteProvidersEnabled` defaults to `true` and can disable remote translation providers. The application's global offline preference also blocks remote providers. Loopback endpoints remain available for local providers. Core endpoint validation requires HTTPS for non-loopback endpoints.

Profiles and settings are atomically stored in `FileTranslationRepository.defaultDirectory()` in `bridge-profiles.json` with user-only file permissions. Secrets are stored through the platform credential store: Keychain on macOS and session-only memory on Linux. The bridge never returns or writes a credential value.

## Capture and subtitle methods

| Method | Parameters | Result |
| --- | --- | --- |
| `translation.capture.start` | `container`, `entryID`, `providerID`, optional `architecture` (`x64` default), optional `restoreHistory` (`false` default) | Snapshot including process inventory |
| `translation.capture.processes` | none | `{processes, truncated}` |
| `translation.capture.attach` | `{target: processRecord}` copied from inventory | Snapshot |
| `translation.capture.stop`, `translation.capture.detach` | none | Stopped snapshot; helper EOF detaches its own hook |
| `translation.capture.insertHook` | `{hookCode: string}` | Result from explicitly requested hook insertion |
| `translation.threads.list` | optional `offset`, `maxCount` (maximum 64) | `{threads,total,nextOffset}` |
| `translation.threads.select` | `{selections: [{threadID,role}]}` | Selected count; roles are `body`, `speaker`, `choice` |
| `translation.snapshot` | none | `{sessionID,gameID,captureState,lastError,target,subtitle,cursor,processes,historyCount,threads,threadsTotal}` |
| `translation.events` | optional `afterCursor`, `maxCount` (maximum 32) | `{events,cursor,resetRequired}` |
| `translation.history` | optional `offset`, `maxCount` (maximum 20) | `{entries,total,nextOffset}` |
| `translation.history.text` | `entryID`, `field` (`source` or `translation`), optional character `offset`, `maxCharacters` (maximum 16384) | `{text,nextOffset,total}` |
| `translation.history.include` | `{entryID,include: boolean}` | Update context inclusion for a history entry |
| `translation.history.clear` | none | Clear current game's history and cache |
| `translation.cache.clear` | none | Clear current session cache |
| `translation.overlay.read` | none | Overlay settings and platform capabilities |
| `translation.overlay.configure` | `{settings: partialOverlaySettings}` | Overlay settings and platform capabilities |

A process record is `{guestPID,exePath,arch,creationTime}`. `creationTime` is exactly 16 hexadecimal Windows FILETIME digits. Attach accepts only a record from the last inventory, and the native helper independently revalidates the opened process identity. Selecting an x86 process automatically replaces the inventory helper with its x86 counterpart. Both helpers use the bottle's configured engine, engine lease and Wine prefix. Tokens are environment-only. Stop never terminates the game or wineserver.

No text thread translates automatically. Select at most one `body`, one `speaker`, and up to eight total threads; `choice` text translates independently. Thread IDs are opaque strings formed from hexadecimal addresses and contexts. Previews are bounded; the full captured source is preserved in history. Late channel events, source revisions and translation responses cannot replace a newer session's subtitles. Repeated same-stream text is deduplicated by the translation core.

Subtitle fields are `source`, `translation`, `speaker`, `kind`, `status`, `error`, and optionally `translationTruncated`. Source stays visible during pending and failed translations. Snapshot/history translations are limited to 16,384 characters; use `translation.history.text` to retrieve full text in chunks. Snapshots include the first 64 threads; use the thread pager for the rest. Event pages and history pages stay below 512 KiB, within the bridge's 1 MiB frame limit. Events are retained in a 64-entry ring. If `resetRequired` is true, fetch a snapshot and resume from its cursor.

Histories are isolated by a SHA-256 key of the canonical bottle path and library entry ID, with one user-only file per game under `Translation/history`. Starting without `restoreHistory` clears the previous history for that game. Restoring requires explicit selection; retention zero disables persisted history. Provider configuration changes cancel an in-flight translation and invalidate the session cache. Disabling remote access stops a remote capture session while local providers remain usable.

On macOS the AppKit overlay is non-activating, floating and full-screen auxiliary; on Linux the GPUI subtitle window serves it (`{"mode":"gpui","platform":"Linux"}` in the read result). Settings include `visible`, `clickThrough`, `showSource`, `showTranslation`, `fontSize` (12–72), `opacity` (0.1–1), `width`, `height`, and optional `x`/`y` in desktop points. Disable click-through to drag or resize; saved position is recovered onto a connected display. Native overlay capability does not establish real-game full-screen compatibility, which must be tested locally.

Native payloads are resolved beside `whisky-bridge` under `LunaHook/x86` and `LunaHook/x64`. Development runs may set an absolute `MALTORIA_LUNAHOOK_ROOT`. Each architecture requires the helper EXE, matching LunaHost and LunaHook DLLs, and `LunaTmpFontLoader.dll`. Missing payloads produce `capture_payload_unavailable` rather than silently skipping capture.
