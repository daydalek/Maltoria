#!/usr/bin/env python3
"""Check and query GPUI locale resources (apps/whisky-gpui/locales/*.toml).

Commands:
  check        Validate locale files, call-site patterns, literals and bridge
               error-code coverage. Exits non-zero on failure (the CI gate).
  stats        Print key counts, allowlist sizes and bridge coverage numbers.
  find <text>  List keys whose value in any locale contains <text>.

`en.toml` is the reference locale: every other file must define the same
sections, keys, array lengths, placeholder sets and plural categories. A
`[maps.<name>]` table produces a sorted runtime lookup map.
"""

import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
APP = ROOT / "apps" / "whisky-gpui"
LOCALES = APP / "locales"
SRC = APP / "src"
SWIFT_SRC = ROOT / "WhiskyBridge" / "Sources"
ALLOWLIST_PATH = LOCALES / "i18n-allowlist.toml"

PLACEHOLDER_RE = re.compile(r"\{([a-zA-Z_][a-zA-Z0-9_]*)\}")
CJK_RE = re.compile(r"[぀-ヿ一-鿿가-힯]")
PLURAL_CATEGORIES = {"zero", "one", "other"}

# Banned call patterns outside src/i18n/ (H2 will migrate these sites).
BANNED_PATTERNS = [
    ("crate::i18n::tr(", re.compile(r"crate::i18n::tr\(")),
    ("crate::i18n::pick(", re.compile(r"crate::i18n::pick\(")),
    ("match crate::i18n::locale()", re.compile(r"match\s+crate::i18n::locale\(\)")),
    ("match i18n::locale()", re.compile(r"match\s+i18n::locale\(\)")),
]

BRIDGE_MAPS = ["bridge_static", "bridge_detail", "bridge_common", "bridge_capture"]


class Failure(Exception):
    pass


def placeholders(template):
    """Return the set of `{name}` placeholders, skipping `{{`/`}}` escapes."""
    found = set()
    i = 0
    while i < len(template):
        if template.startswith("{{", i) or template.startswith("}}", i):
            i += 2
        elif template[i] == "{":
            j = template.find("}", i)
            if j < 0:
                break
            found.add(template[i + 1 : j])
            i = j + 1
        else:
            i += 1
    return found


def classify(value):
    """Classify a TOML value: 'scalar', 'array', 'template', or 'plural'."""
    if isinstance(value, str):
        return "template" if placeholders(value) else "scalar"
    if isinstance(value, list):
        return "array"
    if isinstance(value, dict):
        return "plural"
    return "other"


def load_locales():
    files = {}
    for path in sorted(LOCALES.glob("*.toml")):
        if path.stem == "i18n-allowlist":
            continue
        try:
            files[path.stem] = tomllib.loads(path.read_text())
        except tomllib.TOMLDecodeError as e:
            raise Failure(f"{path}: {e}")
    return files


def validate_locales(files):
    """Mirror build.rs rules; return a list of error strings."""
    errors = []
    if "en" not in files:
        return ["locales/en.toml missing (en is the reference locale)"]
    en = files["en"]

    def split(table):
        return (
            {k: v for k, v in table.items() if k != "maps"},
            table.get("maps", {}),
        )

    en_sections, en_maps = split(en)
    for sec, entries in en_sections.items():
        for key, value in entries.items():
            kind = classify(value)
            name = f"{sec}.{key}"
            if kind == "array" and not all(isinstance(v, str) for v in value):
                errors.append(f"en.toml: {name}: array items must be strings")
            if kind == "plural":
                for cat in value:
                    if cat not in PLURAL_CATEGORIES:
                        errors.append(f"en.toml: {name}: invalid plural category `{cat}`")
                if "other" not in value:
                    errors.append(f"en.toml: {name}: plural table is missing required category `other`")
                elif "count" not in placeholders(value["other"]):
                    errors.append(f"en.toml: {name}.other: plural `other` must contain {{count}}")
            if kind == "other":
                errors.append(f"en.toml: {name}: value must be a string, string array, or plural table")
    for map_name, table in en_maps.items():
        for key, value in table.items():
            if not isinstance(value, str):
                errors.append(f"en.toml: maps.{map_name}.{key}: map values must be strings")
            elif placeholders(value):
                errors.append(f"en.toml: maps.{map_name}.{key}: map values must not contain placeholders")

    for stem, table in sorted(files.items()):
        if stem == "en":
            continue
        file = f"locales/{stem}.toml"
        sections, maps = split(table)
        for sec in en_sections:
            if sec not in sections:
                errors.append(f"{file}: missing section [{sec}] (present in en.toml)")
        for sec in sections:
            if sec not in en_sections:
                errors.append(f"{file}: extra section [{sec}] (not in en.toml)")
        for sec, en_entries in en_sections.items():
            entries = sections.get(sec, {})
            for key in en_entries:
                if key not in entries:
                    errors.append(f"{file}: missing key {sec}.{key} (present in en.toml)")
            for key in entries:
                if key not in en_entries:
                    errors.append(f"{file}: extra key {sec}.{key} (not in en.toml)")
            for key, en_value in en_entries.items():
                if key not in entries:
                    continue
                value = entries[key]
                name = f"{sec}.{key}"
                ek, ok = classify(en_value), classify(value)
                if ek != ok:
                    errors.append(f"{file}: {name}: value kind does not match en.toml")
                    continue
                if ek == "array" and len(value) != len(en_value):
                    errors.append(f"{file}: {name}: array length {len(value)} != en.toml ({len(en_value)})")
                if ek == "template" and placeholders(value) != placeholders(en_value):
                    errors.append(f"{file}: {name}: placeholder set does not match en.toml")
                if ek == "plural":
                    for cat in value:
                        if cat not in PLURAL_CATEGORIES:
                            errors.append(f"{file}: {name}: invalid plural category `{cat}`")
                        elif cat == "one":
                            errors.append(f"{file}: {name}: `one` is only allowed in en.toml")
                        elif cat == "other" and "count" not in placeholders(value[cat]):
                            errors.append(f"{file}: {name}.other: plural `other` must contain {{count}}")
                        elif cat not in en_value and cat != "zero":
                            errors.append(f"{file}: {name}: category `{cat}` not present in en.toml")
                        elif cat in en_value and placeholders(value[cat]) != placeholders(en_value[cat]):
                            errors.append(f"{file}: {name}.{cat}: placeholder set does not match en.toml")
                    if "other" not in value:
                        errors.append(f"{file}: {name}: plural table is missing required category `other`")
        for map_name, en_map in en_maps.items():
            other_map = maps.get(map_name)
            if other_map is None:
                errors.append(f"{file}: missing [maps.{map_name}] (present in en.toml)")
                continue
            for key in en_map:
                if key not in other_map:
                    errors.append(f"{file}: missing key maps.{map_name}.{key} (present in en.toml)")
            for key in other_map:
                if key not in en_map:
                    errors.append(f"{file}: extra key maps.{map_name}.{key} (not in en.toml)")
            for key in other_map:
                if key in en_map:
                    value = other_map[key]
                    if not isinstance(value, str) or placeholders(value):
                        errors.append(f"{file}: maps.{map_name}.{key}: map values must not contain placeholders")
        for map_name in maps:
            if map_name not in en_maps:
                errors.append(f"{file}: extra [maps.{map_name}] (not in en.toml)")
    return errors


def rust_files():
    return sorted(p for p in SRC.rglob("*.rs"))


def strip_test_regions(text):
    """Return (code text, set of 1-based line numbers inside #[cfg(test)] blocks).

    Heuristic: a `#[cfg(test)]` line marks the start; the following braced block
    (or the rest of the file when no braces are found on that module) is test code.
    """
    lines = text.split("\n")
    test_lines = set()
    i = 0
    while i < len(lines):
        if "#[cfg(test)]" in lines[i]:
            # consume attributes until the item's opening brace
            depth = 0
            j = i
            opened = False
            while j < len(lines):
                for c in lines[j]:
                    if c == "{":
                        depth += 1
                        opened = True
                    elif c == "}":
                        depth -= 1
                if not opened and ";" in lines[j]:
                    break  # `#[cfg(test)] use ...;` — no block follows
                j += 1
                if opened and depth <= 0:
                    break
            test_lines.update(range(i + 1, min(j, len(lines)) + 1))
            i = j
        else:
            i += 1
    return test_lines


def string_literals(line):
    """Yield string literal contents for a line of Rust code (heuristic)."""
    i = 0
    while i < len(line):
        c = line[i]
        if line.startswith("//", i):
            break
        if c == '"':
            j = i + 1
            content = []
            while j < len(line) and line[j] != '"':
                if line[j] == "\\":
                    j += 1
                else:
                    content.append(line[j])
                j += 1
            yield "".join(content)
            i = j + 1
        else:
            i += 1


def scan_sources():
    """Collect banned-pattern hits, hard-coded literals, and used key names."""
    banned = []  # (file, pattern literal)
    literals = []  # (file, literal content, line number)
    corpus = []  # non-test source text per file, for unused-key search
    for path in rust_files():
        rel = path.relative_to(SRC).as_posix()
        text = path.read_text()
        test_lines = strip_test_regions(text)
        is_test_file = rel.endswith("_tests.rs") or rel.startswith("tests/")
        lines = text.split("\n")
        code = []
        suppressed = set()  # line numbers inside a banned tr()/pick() call extent
        for lineno, line in enumerate(lines, 1):
            if lineno in test_lines or is_test_file:
                continue
            code.append(line)
            if rel.startswith("i18n/"):
                continue
            for literal, pat in BANNED_PATTERNS:
                m = pat.search(line)
                if m:
                    banned.append((rel, literal))
                    if "match" not in literal:
                        # suppress literal checks through the call's balanced extent
                        depth = line.count("(", m.start()) - line.count(")", m.start())
                        k = lineno
                        while depth > 0 and k < len(lines):
                            k += 1
                            suppressed.add(k)
                            depth += lines[k - 1].count("(") - lines[k - 1].count(")")
            if lineno in suppressed:
                continue
            for m in re.finditer(r"\.child\(\s*", line):
                rest = line[m.end() :]
                if rest.lstrip().startswith('"'):
                    for lit in string_literals(rest):
                        if re.search(r"[A-Za-z]", lit):
                            literals.append((rel, lit, lineno))
                        break
            for lit in string_literals(line):
                if CJK_RE.search(lit):
                    literals.append((rel, lit, lineno))
        corpus.append("\n".join(code))
    return banned, literals, corpus


def load_allowlist():
    if not ALLOWLIST_PATH.exists():
        return {"banned_patterns": {}, "hardcoded": {}, "bridge_untranslated": {}, "unused_keys": {}}
    data = tomllib.loads(ALLOWLIST_PATH.read_text())
    out = {}
    for cat in ("banned_patterns", "hardcoded", "bridge_untranslated", "unused_keys"):
        entries = data.get(cat, {})
        # TOML shape: [cat."file::literal"] reason = "..." [count = N]
        normalized = {}
        for key, value in entries.items():
            if isinstance(value, dict):
                normalized[key] = value
            else:
                normalized[key] = {"reason": str(value)}
        out[cat] = normalized
    return out


def allowlisted(entries, file, literal):
    key = f"{file}::{literal}"
    return entries.get(key)


def bridge_codes():
    """Extract literal `BridgeFailure("code", ...)` codes from Swift sources."""
    codes = {}  # code -> set(files)
    pattern = re.compile(
        r'BridgeFailure\(\s*(?:code:\s*)?"([a-zA-Z_][a-zA-Z0-9_]*)"\s*,', re.S
    )
    for path in sorted(SWIFT_SRC.rglob("*.swift")):
        text = path.read_text()
        rel = path.relative_to(ROOT).as_posix()
        for m in pattern.finditer(text):
            codes.setdefault(m.group(1), set()).add(rel)
    return codes


def map_key_sets(en_maps):
    return {name: set(table) for name, table in en_maps.items()}


def cmd_check():
    try:
        files = load_locales()
    except Failure as e:
        print(f"i18n check: {e}", file=sys.stderr)
        return 1
    errors = validate_locales(files)
    warnings = []
    allow = load_allowlist()

    en = files.get("en", {})
    en_maps = en.get("maps", {})

    # (b) unused keys: a non-map key is unused if its name appears nowhere.
    banned, literals, corpus = scan_sources()
    haystack = "\n".join(corpus)
    for sec, entries in en.items():
        if sec == "maps":
            continue
        for key in entries:
            if not re.search(rf"\b{re.escape(key)}\b", haystack):
                sk = f"{sec}.{key}"
                if allow["unused_keys"].get(sk):
                    continue
                errors.append(f"unused key: {sk} appears nowhere in apps/whisky-gpui/src")

    # (c) banned patterns outside src/i18n/.
    grouped = {}
    for rel, literal in banned:
        grouped.setdefault((rel, literal), 0)
        grouped[(rel, literal)] += 1
    for (rel, literal), count in sorted(grouped.items()):
        entry = allowlisted(allow["banned_patterns"], rel, literal)
        if entry and count <= int(entry.get("count", count)):
            continue
        errors.append(
            f"{rel}: banned pattern `{literal}` x{count} "
            "(add to locales/i18n-allowlist.toml or migrate to s())"
        )

    # (d) hard-coded user-facing literals in non-test code outside src/i18n/.
    lit_grouped = {}
    for rel, lit, lineno in literals:
        lit_grouped.setdefault((rel, lit), []).append(lineno)
    for (rel, lit), linenos in sorted(lit_grouped.items()):
        if allowlisted(allow["hardcoded"], rel, lit):
            continue
        preview = lit if len(lit) <= 60 else lit[:57] + "..."
        errors.append(
            f"{rel}:{linenos[0]}: hard-coded literal {preview!r} x{len(linenos)} "
            "(add to locales/i18n-allowlist.toml or move to locales/*.toml)"
        )

    # (e) Swift bridge error-code coverage.
    maps = map_key_sets(en_maps)
    known = set().union(*maps.values()) if maps else set()
    for code, files_set in sorted(bridge_codes().items()):
        check = code
        if code.startswith("capture_"):
            check = code[len("capture_") :]
            hit = check in maps.get("bridge_capture", set())
        else:
            hit = check in known
        if not hit:
            entry = allow["bridge_untranslated"].get(code)
            if not entry:
                files_str = ", ".join(sorted(files_set)[:3])
                errors.append(
                    f"bridge code `{code}` not localized "
                    f"({files_str}) — add to maps.bridge_* in locales/en.toml "
                    "or bridge_untranslated in the allowlist"
                )

    # (f) en-identical values in other locales -> warnings.
    for stem, table in sorted(files.items()):
        if stem == "en":
            continue
        for sec, entries in table.items():
            if sec == "maps":
                continue
            for key, value in entries.items():
                if value == en.get(sec, {}).get(key):
                    warnings.append(f"locales/{stem}.toml: {sec}.{key} identical to en")
        for map_name, table_map in table.get("maps", {}).items():
            for key, value in table_map.items():
                if value == en_maps.get(map_name, {}).get(key):
                    warnings.append(f"locales/{stem}.toml: maps.{map_name}.{key} identical to en")

    for w in warnings:
        print(f"warning: {w}")
    if errors:
        for e in errors:
            print(f"error: {e}")
        print(f"i18n check: {len(errors)} error(s), {len(warnings)} warning(s)")
        return 1
    print(f"i18n check: ok ({len(warnings)} warning(s))")
    return 0


def cmd_stats():
    try:
        files = load_locales()
    except Failure as e:
        print(f"i18n stats: {e}", file=sys.stderr)
        return 1
    allow = load_allowlist()
    for stem, table in sorted(files.items()):
        keys = sum(len(v) for k, v in table.items() if k != "maps")
        map_keys = sum(len(v) for v in table.get("maps", {}).values())
        print(f"{stem}: {keys} keys, {sum(len(v) for v in table.get('maps', {}).values())} map entries")
    for cat, entries in allow.items():
        print(f"allowlist.{cat}: {len(entries)} entr{'y' if len(entries) == 1 else 'ies'}")
    en_maps = files.get("en", {}).get("maps", {})
    known = set().union(*map_key_sets(en_maps).values()) if en_maps else set()
    codes = bridge_codes()
    uncovered = []
    for code in codes:
        check = code[len("capture_") :] if code.startswith("capture_") else code
        if check not in known:
            uncovered.append(code)
    listed = {k.split("::")[-1] for k in allow["bridge_untranslated"]}
    print(f"swift BridgeFailure literal codes: {len(codes)}")
    print(f"uncovered bridge codes: {len([c for c in uncovered if c not in listed])} "
          f"({len(uncovered)} total unlocalized, {len(uncovered) - len([c for c in uncovered if c not in listed])} allowlisted)")
    return 0


def cmd_find(text):
    try:
        files = load_locales()
    except Failure as e:
        print(f"i18n find: {e}", file=sys.stderr)
        return 1
    needle = text.lower()
    hits = 0
    for stem, table in sorted(files.items()):
        for sec, entries in table.items():
            if sec == "maps":
                for map_name, table_map in entries.items():
                    for key, value in table_map.items():
                        if isinstance(value, str) and needle in value.lower():
                            print(f"{stem}: maps.{map_name}.{key} = {value}")
                            hits += 1
                continue
            for key, value in entries.items():
                values = (
                    value.values()
                    if isinstance(value, dict)
                    else value if isinstance(value, list) else [value]
                )
                for v in values:
                    if isinstance(v, str) and needle in v.lower():
                        print(f"{stem}: {sec}.{key} = {v}")
                        hits += 1
    if not hits:
        print(f"no keys contain {text!r}")
    return 0


def main():
    if len(sys.argv) < 2 or sys.argv[1] not in ("check", "stats", "find"):
        print(__doc__, file=sys.stderr)
        return 2
    if sys.argv[1] == "check":
        return cmd_check()
    if sys.argv[1] == "stats":
        return cmd_stats()
    if len(sys.argv) < 3:
        print("usage: i18n.py find <text>", file=sys.stderr)
        return 2
    return cmd_find(sys.argv[2])


if __name__ == "__main__":
    sys.exit(main())
