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
