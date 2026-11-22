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
PLURAL_CATEGORIES = {"zero", "one", "few", "many", "other"}

# Per-locale CLDR plural categories: stem -> (allowed, required). `zero` is an
# optional n == 0 override wherever it is allowed. Mirrored in build.rs.
PLURAL_RULES = {
    # CJK locales inflect identically for every cardinal.
    "zh-Hans": ({"zero", "other"}, {"other"}),
    "ja": ({"zero", "other"}, {"other"}),
    "ko": ({"zero", "other"}, {"other"}),
    # n == 1 is singular (French also treats 0 as singular, same categories).
    "en": ({"zero", "one", "other"}, {"one", "other"}),
    "de": ({"zero", "one", "other"}, {"one", "other"}),
    "es": ({"zero", "one", "other"}, {"one", "other"}),
    "it": ({"zero", "one", "other"}, {"one", "other"}),
    "fr": ({"zero", "one", "other"}, {"one", "other"}),
    # Russian cardinals use one/few/many on mod10/mod100.
    "ru": ({"zero", "one", "few", "many", "other"}, {"one", "few", "many", "other"}),
}

# Banned call patterns outside src/i18n/: all text comes from locales/*.toml.
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
                en_allowed, en_required = PLURAL_RULES["en"]
                for cat in en_required:
                    if cat not in value:
                        errors.append(
                            f"en.toml: {name}: missing required plural category `{cat}` for locale `en`"
                        )
                for cat in value:
                    if cat not in PLURAL_CATEGORIES:
                        errors.append(f"en.toml: {name}: invalid plural category `{cat}`")
                    elif cat not in en_allowed:
                        errors.append(
                            f"en.toml: {name}: plural category `{cat}` not allowed for locale `en`"
                        )
                if "other" in value and "count" not in placeholders(value["other"]):
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
                    rules = PLURAL_RULES.get(stem)
                    if rules is None:
                        errors.append(
                            f"{file}: unknown locale `{stem}` — add its plural-category rules "
                            "to plural_rule() in build.rs and PLURAL_RULES in scripts/i18n.py"
                        )
                        allowed, required = PLURAL_CATEGORIES, {"other"}
                    else:
                        allowed, required = rules
                    for cat in required:
                        if cat not in value:
                            errors.append(
                                f"{file}: {name}: missing required plural category `{cat}` for locale `{stem}`"
                            )
                    for cat in value:
                        if cat not in PLURAL_CATEGORIES:
                            errors.append(f"{file}: {name}: invalid plural category `{cat}`")
                        elif cat not in allowed:
                            errors.append(
                                f"{file}: {name}: plural category `{cat}` not allowed for locale `{stem}`"
                            )
                        elif cat == "other" and "count" not in placeholders(value[cat]):
                            errors.append(f"{file}: {name}.other: plural `other` must contain {{count}}")
                        elif cat in en_value and placeholders(value[cat]) != placeholders(en_value[cat]):
                            errors.append(f"{file}: {name}.{cat}: placeholder set does not match en.toml")
                        elif (
                            cat not in en_value
                            and cat != "zero"
                            and "count" not in placeholders(value[cat])
                        ):
                            # `zero` is a free-form n == 0 override; categories a
                            # locale needs but en lacks must render the count.
                            errors.append(
                                f"{file}: {name}.{cat}: category absent from en.toml must contain {{count}}"
                            )
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


def sanitized_lines(text):
    """Return per-line text with string/comment contents blanked out.

    Tracks `//` comments, `"..."` strings and `r#"..."#` raw strings across the
    whole file so brace counting is not confused by literal contents. Char literals are blanked too; lifetimes like `'a` pass through.
    """
    out = []
    buf = []
    i = 0
    n = len(text)
    mode = None  # None | "str" | ("raw", hashes)
    while i < n:
        c = text[i]
        if mode == "str":
            if c == "\\":
                i += 2
                continue
            if c == '"':
                mode = None
            elif c == "\n":
                out.append("".join(buf))
                buf = []
            i += 1
            continue
        if isinstance(mode, tuple):
            if text.startswith('"' + mode[1], i):
                i += 1 + len(mode[1])
                mode = None
                continue
            if c == "\n":
                out.append("".join(buf))
                buf = []
            i += 1
            continue
        if text.startswith("//", i):
            j = text.find("\n", i)
            if j < 0:
                break
            out.append("".join(buf))
            buf = []
            i = j + 1
            continue
        if c == '"':
            mode = "str"
            i += 1
            continue
        m = re.match(r"r(#*)\"", text[i:])
        if m:
            if m.group(1):
                mode = ("raw", m.group(1))
                i += m.end()
                continue
            mode = "str"  # plain r"..." — closes at the next unescaped quote
            i += m.end()
            continue
        if c == "'":
            j = i + 1
            if j < n and text[j] == "\\":
                j += 2
            else:
                j += 1
            if j < n and text[j] == "'":
                i = j + 1
                continue
        if c == "\n":
            out.append("".join(buf))
            buf = []
            i += 1
            continue
        buf.append(c)
        i += 1
    out.append("".join(buf))
    return out


def strip_test_regions(text):
    """Return the set of 1-based line numbers inside #[cfg(test)] items.

    Heuristic: a `#[cfg(test)]` line marks the start; the following braced block
    is test code.
    """
    lines = sanitized_lines(text)
    test_lines = set()
    i = 0
    while i < len(lines):
        if "#[cfg(test)]" in lines[i]:
            depth = 0
            j = i
            opened = False
            inner = 0  # ( and [ depth, so `;` in `[T; N]` types is ignored
            end = j  # 0-based index of the last line of the item
            while j < len(lines):
                for c in lines[j]:
                    if c in "([":
                        inner += 1
                    elif c in ")]":
                        inner -= 1
                    elif c == "{" and inner == 0:
                        depth += 1
                        opened = True
                    elif c == "}" and inner == 0:
                        depth -= 1
                if not opened and inner == 0 and ";" in lines[j]:
                    end = j  # `#[cfg(test)] use ...;` — item ends on this line
                    break
                end = j
                j += 1
                if opened and depth <= 0:
                    break
            test_lines.update(range(i + 1, min(end + 2, len(lines) + 1)))
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
        return {
            "banned_patterns": {},
            "hardcoded": {},
            "bridge_untranslated": {},
            "bridge_dynamic": {},
            "unused_keys": {},
            "identical": {},
            "naming": {},
        }
    data = tomllib.loads(ALLOWLIST_PATH.read_text())
    out = {}
    for cat in (
        "banned_patterns",
        "hardcoded",
        "bridge_untranslated",
        "bridge_dynamic",
        "unused_keys",
        "identical",
        "naming",
    ):
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


def bridge_dynamic_calls():
    """Find `BridgeFailure(...)` sites whose code argument is not a literal."""
    calls = {}  # expr -> set(files)
    literal = re.compile(
        r'BridgeFailure\(\s*(?:code:\s*)?"[a-zA-Z_][a-zA-Z0-9_]*"\s*,', re.S
    )
    start = re.compile(r"BridgeFailure\(\s*(?:code:\s*)?")
    for path in sorted(SWIFT_SRC.rglob("*.swift")):
        text = path.read_text()
        rel = path.relative_to(ROOT).as_posix()
        for m in start.finditer(text):
            if literal.match(text, m.start()):
                continue
            i = m.end()
            depth = 1
            j = i
            while j < len(text) and depth > 0:
                if text[j] == "(":
                    depth += 1
                elif text[j] == ")":
                    depth -= 1
                elif text[j] == "," and depth == 1:
                    break
                j += 1
            expr = re.sub(r"\s+", " ", text[i:j]).strip()
            calls.setdefault(expr, set()).add(rel)
    return calls


KEY_NAME_RE = re.compile(r"^[a-z][a-z0-9_]*$")
KEY_NAME_MAX = 40
NUMERIC_PLACEHOLDER_RE = re.compile(r"p\d+")


def naming_errors(en):
    """Enforce semantic key/placeholder naming on en.toml (the reference)."""
    errors = []
    for sec, entries in en.items():
        if sec == "maps":
            continue
        for key, value in entries.items():
            name = f"{sec}.{key}"
            if not KEY_NAME_RE.match(key):
                errors.append(
                    f"en.toml: {name}: key name must match {KEY_NAME_RE.pattern}"
                )
            elif len(key) > KEY_NAME_MAX:
                errors.append(
                    f"en.toml: {name}: key name exceeds {KEY_NAME_MAX} characters"
                )
            values = (
                value.values()
                if isinstance(value, dict)
                else (value if isinstance(value, list) else [value])
            )
            for item in values:
                if not isinstance(item, str):
                    continue
                for ph in sorted(placeholders(item)):
                    if len(ph) < 2:
                        errors.append(
                            f"en.toml: {name}: placeholder {{{ph}}} must be at least 2 characters"
                        )
                    elif NUMERIC_PLACEHOLDER_RE.fullmatch(ph):
                        errors.append(
                            f"en.toml: {name}: placeholder {{{ph}}} must be descriptive, not {ph!r}"
                        )
    return errors


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

    # (a2) key/placeholder naming on the en reference.
    for e in naming_errors(en):
        key = e.split(": ")[1].split(":")[0]
        if allow["naming"].get(key):
            continue
        errors.append(e)

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
    for expr, files_set in sorted(bridge_dynamic_calls().items()):
        key = f"{sorted(files_set)[0]}::{expr}"
        if allow["bridge_dynamic"].get(key):
            continue
        errors.append(
            f"{sorted(files_set)[0]}: dynamic BridgeFailure code `{expr}` "
            "— add a bridge_dynamic allowlist entry with a reason"
        )

    # (f) en-identical values in other locales -> warnings.
    for stem, table in sorted(files.items()):
        if stem == "en":
            continue
        for sec, entries in table.items():
            if sec == "maps":
                continue
            for key, value in entries.items():
                if value == en.get(sec, {}).get(key) and not allow["identical"].get(f"{sec}.{key}"):
                    warnings.append(f"locales/{stem}.toml: {sec}.{key} identical to en")
        for map_name, table_map in table.get("maps", {}).items():
            for key, value in table_map.items():
                if value == en_maps.get(map_name, {}).get(key) and not allow[
                    "identical"
                ].get(f"maps.{map_name}.{key}"):
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
    print(f"swift BridgeFailure dynamic sites: {len(bridge_dynamic_calls())}")
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
