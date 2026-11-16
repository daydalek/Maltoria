# Third-party notices and source attribution

Maltoria is distributed under GPL-3.0-or-later. Copyright and license notices for
its dependencies are preserved in `ThirdParty/licenses/`. The application carries
this document and that directory under `Contents/Resources/ThirdParty/`.
`ThirdParty/manifest.json` records the resolved source revisions and file hashes.

## Application ancestry

This fork, <https://github.com/daydalek/Maltoria>, derives from
<https://github.com/frankea/Whisky> and the original project by Isaac Marovitz,
<https://github.com/whisky-app/Whisky>. Their contributions and the contributions
of the Whisky authors remain covered by the original file notices and
GPL-3.0-or-later. The full GPL text is in `licenses/Whisky-GPL-3.0.txt`.
The source corresponding to a release is available from this repository at its
release tag; the release also carries an application source archive.

## Linked dependencies

| Component | Version | Source | License / notice file in `licenses/` |
| --- | --- | --- | --- |
| SemanticVersion | 0.4.0 | <https://github.com/SwiftPackageIndex/SemanticVersion> | Apache-2.0; `SemanticVersion.txt`; copyright Dave Verwer, Sven A. Schmidt, and other contributors |
| Sparkle | 2.8.1 | <https://github.com/sparkle-project/Sparkle> | MIT and bundled component notices; `Sparkle.txt` (includes bsdiff/bspatch, sais-lite, Ed25519, and other upstream notices) |
| PostHog | 3.59.3 | <https://github.com/PostHog/posthog-ios> | MIT; `posthog-ios.txt`; copyright PostHog |
| swift-argument-parser | 1.5.0 | <https://github.com/apple/swift-argument-parser> | Apache-2.0 with Swift Runtime Library Exception; `swift-argument-parser.txt`; copyright Apple Inc. and the Swift project authors |

The application retains its PostHog link dependency while telemetry is disabled
by the fork's application code. PostHog's vendored components have additional
notices: libwebp (`PostHog-libwebp-COPYING.txt`, BSD-3-Clause, copyright Google Inc.,
and `PostHog-libwebp-PATENTS.txt`); PLCrashReporter
(`PostHog-PLCrashReporter.txt`, MIT and its stated component terms, copyright
Microsoft Corporation, Plausible Labs Cooperative, Inc., and contributors);
protobuf-c (`PostHog-protobuf-c.txt`, BSD-2-Clause, copyright Dave Benson and the
protobuf-c authors). Preserve the complete PLCrashReporter notice, including its
Apache-2.0 attribution; the Apache text is supplied in `swift-argument-parser.txt`.

`SemanticVersion`, `Sparkle`, `PostHog`, `swift-argument-parser`, and
`swift-docc-*` are pinned SwiftPM dependencies in `WhiskyKit`/`WhiskyCmd`,
whereas the desktop GPUI application (`apps/whisky-gpui`) uses Rust +
`WhiskyBridge`.

## Desktop UI and Rust dependencies

- **`gpui-kit` 0.8.0** — <https://crates.io/crates/gpui-kit>; Apache-2.0, derived
  from Zed's GPUI (<https://github.com/zed-industries/zed>). Provides the native
  desktop UI framework for `apps/whisky-gpui`.
- **Locked Rust crate dependencies** — All direct and transitive Rust crate
  dependencies are pinned in `apps/whisky-gpui/Cargo.lock` (`chrono`, `image`,
  `metal` on macOS, `serde`, `serde_json`, `sha2`, `smol`, `uuid`, and their
  transitive crates). Linux distribution archives also ship individual crate
  licenses in `share/licenses/whisky/crates/` alongside
  `share/whisky/dependencies.json` and locked `.crate` archives in
  `share/whisky/sources/crates/`.

## Bundled tools and their source

- **Winetricks 20260125** — <https://github.com/Winetricks/winetricks/tree/20260125>.
  LGPL-2.1-or-later; full text in `Winetricks-LGPL-2.1.txt`. The distributed
  `winetricks` script is its source and preserves the complete author copyright
  block (Dan Kegel, Austin English, and the contributors listed there).
  `verbs.txt` is a generated index of this script's verbs.
- **cabextract 1.11** — <https://www.cabextract.org.uk/>; copyright 2000–2023
  Stuart Caie. Source files permit GPL-2.0-or-later; this distribution uses the
  GPLv3 option, with upstream's full `cabextract-COPYING.txt` and
  `cabextract-AUTHORS.txt`. The bundled libmspack source is LGPL-2.1 (copyright
  Stuart Caie); its license text is supplied in `Winetricks-LGPL-2.1.txt`.
  `sources/cabextract-1.11.tar.gz` is the unmodified official source distribution,
  including its bundled libmspack, build scripts, and tests. Maltoria rebuilds the
  arm64 executable from it. See `cabextract-BUILD.md` for commands and provenance.
- **MinGW-w64 runtime support** — <https://www.mingw-w64.org/>. The Windows
  helpers have their source in `scripts/whisky_audio_test.c` and
  `scripts/discord-bridge/bridge.c`, and their build commands in the adjacent
  scripts/Makefile. Preserve `mingw-w64-COPYING.txt` and `mingw-w64-AUTHORS.txt`
  for MinGW-w64 runtime support (ZPL-2.1 and the exceptions stated in that file).
  `mingw-w64-runtime.txt` preserves the MinGW-w64 v14.0.0 runtime's component
  notices from `COPYING.MinGW-w64-runtime/COPYING.MinGW-w64-runtime.txt` at
  <https://github.com/mingw-w64/mingw-w64/tree/v14.0.0>.
  GCC support routines use GPLv3 with GCC Runtime Library Exception 3.1;
  `GCC-Runtime-Exception.txt` preserves upstream `COPYING.RUNTIME` from
  <https://github.com/gcc-mirror/gcc/tree/releases/gcc-16.1.0>.
  The accompanying GPLv3 text is in `Whisky-GPL-3.0.txt`.

## Build and documentation dependencies

`swift-docc-plugin` 1.4.5 and `swift-docc-symbolkit` 1.0.0 are provided by
<https://github.com/swiftlang/swift-docc-plugin> and
<https://github.com/swiftlang/swift-docc-symbolkit>. Their Apache-2.0 texts and
Swift Runtime Library Exception, where supplied upstream, are preserved in
`swift-docc-plugin.txt` and `swift-docc-symbolkit.txt`.

## Bundled shader source

- **Anime4K CNN x2 S** — <https://github.com/bloc97/Anime4K>; MIT, copyright
  2019–2021 bloc97; full text in `Anime4K.txt`. The GPUI library's cover
  upscaler (`apps/whisky-gpui/src/upscale/anime4k.metal`) is that network's four
  convolutions and its depth-to-space pass, translated to Metal by this fork's
  sibling Wine repository and kept verbatim, licence header included. Weights
  are part of the shader; no model file is distributed. `anime4k.rs` supplies the
  dispatch code written for this application.

## Bundled model weights

- **Real-CUGAN models-se `up2x-no-denoise`** — <https://github.com/bilibili/ailab>
  (Real-CUGAN) and <https://github.com/nihui/realcugan-ncnn-vulkan>; MIT for both;
  full texts in `RealCUGAN-bilibili-MIT.txt` and `RealCUGAN-ncnn-MIT.txt`. The
  ncnn graph is converted to a fixed-size Core ML program by this fork
  (`convert-realcugan-coreml.py`) and shipped as
  `realcugan-384x384.mlpackage`; `realcugan-helper` tiles a cover into that
  window, blends the overlaps, and writes the enlarged PNG. Weights are MIT and
  were checked against the digest pinned in `apps/whisky-gpui/tools/upscaler-sources.json` before
  conversion.

## Metadata design reference and data attribution

The metadata provider design references PotatoVN by GoldenPotato137:
<https://github.com/GoldenPotato137/PotatoVN>. Its Apache-2.0 license is preserved
in `PotatoVN.txt`, including its own third-party notices. This entry records the
design reference already documented by this fork; a line-level code reuse map
has yet to be established.

Game descriptions, titles, and cover artwork come from Bangumi
(<https://bangumi.github.io/api/>), VNDB (<https://api.vndb.org/kana>),
Steam (<https://store.steampowered.com/>), YMGal
(<https://www.ymgal.games/developer>), and NextMoe (<https://nextmoe.com/>).
Copyright remains with the respective platforms and original rightsholders.
YMGal's public API is offered for noncommercial use. These provider entries
describe data fetched at runtime.

## Separately installed runtime and Apple components

Wine, CrossOver-derived Wine code, GE-Proton patches, DXVK, DXMT, MoltenVK,
msync, and other runtime libraries are distributed through
<https://github.com/daydalek/ge-proton-macos-wine-poc>. Their applicable licenses,
patches, notices, and corresponding source belong with each runtime release.
Consult that release's source and license materials for its exact payload.
Apple's D3DMetal/Game Porting Toolkit components are imported under Apple's
applicable terms by the user. This application notice covers the application
and the bundled tools listed above.

## Native Windows text-hook build sources

- **LunaTranslator / LunaHook** (<https://github.com/HIllya51/LunaTranslator>) —
  pinned to commit `178b4372c4ed7ac0f67aa2e217007916e937250d`, GPL-3.0; provides
  the core Windows text-hooking and injection engine (`LunaHook32.dll`,
  `LunaHook64.dll`, `LunaHost32.dll`, `LunaHost64.dll`, `LunaTmpFontLoader.dll`)
  paired with Maltoria's IPC wrapper `MaltoriaLunaHost32/64.exe`
  (`GPL-3.0-or-later`). Full upstream license and copyright notices are
  preserved in `licenses/LunaTranslator-GPL-3.0.txt` and the corresponding
  source archive.
- **MinHook** — pinned to commit `c3fcafdc10146beb5919319d0683e44e3c30d537`
  (v1.3.4), BSD-2-Clause, copyright Tsuda Kageyu; full upstream notice is
  `licenses/MinHook-BSD-2-Clause.txt`. It also preserves the BSD notices for
  the bundled HDE32/HDE64 disassemblers, including Vyacheslav Patkov's credits.
- **uchardet** — pinned to commit `06029ec3340cdf6bf9a6a537dafb3f39eda0560e`,
  whose 61 compiled source files declare
  `MPL-1.1 OR GPL-2.0-or-later OR LGPL-2.1-or-later`. Maltoria selects the
  `GPL-2.0-or-later` option for this GPL-3.0-or-later distribution. The upstream
  MPL-1.1 `COPYING` and verbatim alternative-license notice are preserved in
  `licenses/uchardet-COPYING.txt` and
  `licenses/uchardet-license-options.txt`; the source archive retains every
  original file notice.
- **nlohmann/json** — pinned to commit
  `55f93686c01528224f448c19128836e7df245f72` (v3.12.0), MIT, copyright Niels
  Lohmann; full license is `licenses/nlohmann-json-MIT.txt`.

The exact source archives, their SHA-256 values, and a separate `native_packages`
inventory are in `manifest.json`. `LunaHook-BUILD.md` records the local-source
build procedure. No YY-Thunks or VC-LTL source is included; VC-LTL is disabled.
