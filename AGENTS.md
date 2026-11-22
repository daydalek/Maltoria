# Maltoria

## GPUI app (`apps/whisky-gpui`)

```bash
cargo check --locked --manifest-path apps/whisky-gpui/Cargo.toml
cargo test --locked --manifest-path apps/whisky-gpui/Cargo.toml
```

Standalone Cargo package (own `Cargo.lock`, exact `=` pinned deps). `build.rs`
compiles `locales/*.toml` into the i18n string tables.

## i18n

```bash
python3 scripts/i18n.py check        # CI gate: structure, unused keys, banned patterns, literals, bridge codes
python3 scripts/i18n.py stats        # key/allowlist/coverage counts
python3 scripts/i18n.py find <text>  # locate a UI string by content
python3 -m unittest discover -s scripts -p 'test_i18n.py' -v
```

Translations live in `apps/whisky-gpui/locales/{en,zh-Hans,ja,ko}.toml`; `en` is
the reference locale every other file must match. Exceptions go in
`apps/whisky-gpui/locales/i18n-allowlist.toml` with a reason. See the
Localization section of `CONTRIBUTING.md`.

### Working with i18n — rules

- **Never inline text.** All user-facing strings go in the TOML files. `tr()`,
  `pick()` and inline `match i18n::locale()` for text no longer exist — new
  ones will not compile. Access strings via `s().section.key`,
  `s().section.method(args)` or `i18n::lookup(s().maps.<name>, id)`.
- **Always update all 4 locale files.** `en` defines the key set and placeholder
  set; build.rs fails the build on missing/extra keys, wrong kinds, mismatched
  placeholders or invalid plural categories in any other file.
- **Name keys by role, not by their English text**: `snake_case`, max 40 chars
  (e.g. `llama_cpp_ready_detail`, `prevent_throttling_tip`; suffixes like
  `_title`/`_tip`/`_detail`/`_label`/`_notice`/`_error`). Placeholders get
  descriptive names (`{count}`, `{label}`), never `{n}`/`{p01}`. `i18n.py check`
  enforces both.
- **Value kinds**: plain string → `&'static str` field; string array →
  `[&str; N]`; string with `{name}` → generated method; table with
  `one`/`other`/`zero` → plural method (`other` + `{count}` required; `one` is
  en-only; `zero` used for `n == 0`); `[maps.*]` → runtime-keyed lookup, no
  placeholders allowed in values.
- **Placeholder order trap**: method parameters are the placeholders in
  *alphabetical* order, not template order. When adding/renaming a placeholder,
  the generated signature may change silently — check every call site.
- **fmt.rs is logic-only.** Composition/formatting logic stays there; its
  literals belong in TOML (e.g. `maps.month_name`, `play_time_*` templates).
- **Runtime-keyed maps** (`provider_label`, `source_label`, `config_*`,
  `bridge_*`): when adding a runtime ID in Rust (provider kind, metadata
  source, config field, `BridgeFailure` code in Swift), add the map key in all
  locales or the coverage tests / `check` will fail.
- **Exceptions**: only permanent, justified ones (product names, native
  language names, wire-schema titles, glyphs). Never add "temporary/pending"
  allowlist entries — translate instead.
- **Gotchas**: non-ASCII characters in TOML strings are fine as UTF-8; escape
  literal braces as `{{`/`}}`. `s()` is `pub(crate)` — views in other files use
  `crate::i18n::s()`.

### Verifying i18n changes

- After touching locales or call sites: `cargo check --locked` (build.rs
  validation), then `python3 scripts/i18n.py check` (must be 0 errors, 0
  warnings) before considering the change done.
- `i18n.py find <visible text>` locates the key for a string seen in the UI.
- Linux-gated files (`linux_hud.rs`, `upscale/linux.rs`, `linux_overlay.rs`)
  do not compile on macOS hosts — keep edits there mechanical and grep-verify
  referenced keys exist in `en.toml`.
