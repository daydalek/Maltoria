//! Codegen for `locales/*.toml`: validates every locale file against `en.toml`
//! (the reference locale) and emits `$OUT_DIR/strings.rs`, which
//! `src/i18n/strings.rs` includes.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug)]
enum Entry {
    Scalar(String),
    Array(Vec<String>),
    Template(String, BTreeSet<String>),
    Plural(BTreeMap<String, (String, BTreeSet<String>)>),
}

fn placeholders(template: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' if bytes.get(i + 1) == Some(&b'{') => i += 2,
            b'}' if bytes.get(i + 1) == Some(&b'}') => i += 2,
            b'{' => match template[i + 1..].find('}') {
                Some(rel) => {
                    out.insert(template[i + 1..i + 1 + rel].to_string());
                    i += rel + 2;
                }
                None => break,
            },
            _ => i += 1,
        }
    }
    out
}

fn classify(file: &str, key: &str, value: &toml::Value) -> Entry {
    match value {
        toml::Value::String(s) => {
            let ph = placeholders(s);
            if ph.is_empty() {
                Entry::Scalar(s.clone())
            } else {
                Entry::Template(s.clone(), ph)
            }
        }
        toml::Value::Array(items) => Entry::Array(
            items
                .iter()
                .map(|v| {
                    v.as_str()
                        .unwrap_or_else(|| panic!("{file}: {key}: array items must be strings"))
                        .to_string()
                })
                .collect(),
        ),
        toml::Value::Table(t) => {
            let mut cats = BTreeMap::new();
            for (cat, v) in t {
                if !matches!(cat.as_str(), "zero" | "one" | "few" | "many" | "other") {
                    panic!("{file}: {key}: invalid plural category `{cat}`");
                }
                let s = v.as_str().unwrap_or_else(|| {
                    panic!("{file}: {key}.{cat}: plural categories must be strings")
                });
                cats.insert(cat.clone(), (s.to_string(), placeholders(s)));
            }
            Entry::Plural(cats)
        }
        _ => panic!("{file}: {key}: value must be a string, string array, or plural table"),
    }
}

type Sections = BTreeMap<String, BTreeMap<String, Entry>>;
type Maps = BTreeMap<String, BTreeMap<String, String>>;

fn parse_locale(stem: &str, table: toml::Table) -> (Sections, Maps) {
    let file = format!("locales/{stem}.toml");
    let mut sections: Sections = BTreeMap::new();
    let mut maps: Maps = BTreeMap::new();
    for (name, value) in table {
        if name == "maps" {
            let maps_table = value
                .as_table()
                .unwrap_or_else(|| panic!("{file}: [maps] must be a table"));
            for (map_name, map_value) in maps_table {
                let map = map_value
                    .as_table()
                    .unwrap_or_else(|| panic!("{file}: [maps.{map_name}] must be a table"));
                let mut entries = BTreeMap::new();
                for (k, v) in map {
                    let s = v.as_str().unwrap_or_else(|| {
                        panic!("{file}: maps.{map_name}.{k}: map values must be strings")
                    });
                    if !placeholders(s).is_empty() {
                        panic!("{file}: maps.{map_name}.{k}: map values must not contain placeholders");
                    }
                    entries.insert(k.clone(), s.to_string());
                }
                maps.insert(map_name.clone(), entries);
            }
        } else {
            let section = value
                .as_table()
                .unwrap_or_else(|| panic!("{file}: [{name}] must be a table"));
            let mut entries = BTreeMap::new();
            for (k, v) in section {
                let key = format!("{name}.{k}");
                entries.insert(k.clone(), classify(&file, &key, v));
            }
            sections.insert(name, entries);
        }
    }
    (sections, maps)
}

fn validate_against_en(
    stem: &str,
    en_sections: &Sections,
    en_maps: &Maps,
    sections: &Sections,
    maps: &Maps,
) {
    let file = format!("locales/{stem}.toml");
    for (sec, entries) in en_sections {
        match sections.get(sec) {
            None => panic!("{file}: missing section [{sec}] (present in en.toml)"),
            Some(other) => {
                for key in entries.keys() {
                    if !other.contains_key(key) {
                        panic!("{file}: missing key {sec}.{key} (present in en.toml)");
                    }
                }
                for key in other.keys() {
                    if !entries.contains_key(key) {
                        panic!("{file}: extra key {sec}.{key} (not in en.toml)");
                    }
                }
                for (key, entry) in entries {
                    check_entry(&file, &format!("{sec}.{key}"), stem, entry, &other[key]);
                }
            }
        }
    }
    for sec in sections.keys() {
        if !en_sections.contains_key(sec) {
            panic!("{file}: extra section [{sec}] (not in en.toml)");
        }
    }
    for (map_name, en_map) in en_maps {
        match maps.get(map_name) {
            None => panic!("{file}: missing [maps.{map_name}] (present in en.toml)"),
            Some(other) => {
                for key in en_map.keys() {
                    if !other.contains_key(key) {
                        panic!("{file}: missing key maps.{map_name}.{key} (present in en.toml)");
                    }
                }
                for key in other.keys() {
                    if !en_map.contains_key(key) {
                        panic!("{file}: extra key maps.{map_name}.{key} (not in en.toml)");
                    }
                }
            }
        }
    }
    for map_name in maps.keys() {
        if !en_maps.contains_key(map_name) {
            panic!("{file}: extra [maps.{map_name}] (not in en.toml)");
        }
    }
}

/// CLDR cardinal plural categories a locale file may (`allowed`) and must
/// (`required`) provide in every plural table. `zero` is an optional
/// n == 0 override wherever it is allowed. Mirrored in `scripts/i18n.py`.
fn plural_rule(stem: &str) -> (&'static [&'static str], &'static [&'static str]) {
    match stem {
        // CJK locales inflect identically for every cardinal.
        "zh-Hans" | "ja" | "ko" => (&["zero", "other"], &["other"]),
        // n == 1 is singular (French also treats 0 as singular, same categories).
        "en" | "de" | "es" | "it" | "fr" => (&["zero", "one", "other"], &["one", "other"]),
        // Russian cardinals use one/few/many on mod10/mod100.
        "ru" => (
            &["zero", "one", "few", "many", "other"],
            &["one", "few", "many", "other"],
        ),
        _ => panic!(
            "locales/{stem}.toml: unknown locale `{stem}` — add its plural-category rules to plural_rule() in build.rs and scripts/i18n.py"
        ),
    }
}

fn check_entry(file: &str, key: &str, stem: &str, en: &Entry, other: &Entry) {
    match (en, other) {
        (Entry::Scalar(_), Entry::Scalar(_)) => {}
        (Entry::Array(a), Entry::Array(b)) => {
            if a.len() != b.len() {
                panic!(
                    "{file}: {key}: array length {} does not match en.toml ({})",
                    b.len(),
                    a.len()
                );
            }
        }
        (Entry::Template(_, en_ph), Entry::Template(_, ph)) => {
            if en_ph != ph {
                panic!("{file}: {key}: placeholder set does not match en.toml");
            }
        }
        (Entry::Plural(en_cats), Entry::Plural(cats)) => {
            let (allowed, required) = plural_rule(stem);
            for cat in required {
                if !cats.contains_key(*cat) {
                    panic!(
                        "{file}: {key}: missing required plural category `{cat}` for locale `{stem}`"
                    );
                }
            }
            for (cat, (_, ph)) in cats {
                if !allowed.contains(&cat.as_str()) {
                    panic!("{file}: {key}: plural category `{cat}` not allowed for locale `{stem}`");
                }
                if cat == "other" && !ph.contains("count") {
                    panic!("{file}: {key}.other: plural `other` must contain {{count}}");
                }
                match en_cats.get(cat) {
                    Some((_, en_ph)) => {
                        if en_ph != ph {
                            panic!(
                                "{file}: {key}.{cat}: placeholder set does not match en.toml"
                            );
                        }
                    }
                    // `zero` is a free-form n == 0 override; other categories a
                    // locale needs but en lacks (few/many, and `one` where en
                    // omits it) must at least render the count.
                    None if cat != "zero" => {
                        if !ph.contains("count") {
                            panic!(
                                "{file}: {key}.{cat}: category absent from en.toml must contain {{count}}"
                            );
                        }
                    }
                    None => {}
                }
            }
        }
        _ => panic!("{file}: {key}: value kind does not match en.toml"),
    }
}

// en.toml follows its own allowed/required sets from plural_rule("en");
// `other` must additionally contain {count}.
fn validate_en(entries: &Sections) {
    let (allowed, required) = plural_rule("en");
    for (sec, keys) in entries {
        for (key, entry) in keys {
            if let Entry::Plural(cats) = entry {
                let name = format!("{sec}.{key}");
                for cat in required {
                    if !cats.contains_key(*cat) {
                        panic!(
                            "locales/en.toml: {name}: missing required plural category `{cat}` for locale `en`"
                        );
                    }
                }
                for cat in cats.keys() {
                    if !allowed.contains(&cat.as_str()) {
                        panic!(
                            "locales/en.toml: {name}: plural category `{cat}` not allowed for locale `en`"
                        );
                    }
                }
                if !cats["other"].1.contains("count") {
                    panic!("locales/en.toml: {name}.other: plural `other` must contain {{count}}");
                }
            }
        }
    }
}

fn pascal(name: &str) -> String {
    name.split(['_', '-'])
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn scream(name: &str) -> String {
    name.replace('-', "_").to_uppercase()
}

fn rust_lit(s: &str) -> String {
    format!("{s:?}")
}

fn main() {
    // Windows links the main thread with a 1 MiB default stack reserve, while
    // macOS/Linux give the main thread 8 MiB. The translation settings and
    // cache browser construct deep element trees whose frames overflow 1 MiB
    // (observed as a crash in `__chkstk` under `language_preset_field`), so
    // match the POSIX budget on MSVC builds.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        println!("cargo:rustc-link-arg=/STACK:8388608");
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let locales_dir = manifest.join("locales");
    println!("cargo:rerun-if-changed={}", locales_dir.display());
    let mut stems: Vec<String> = Vec::new();
    for entry in fs::read_dir(&locales_dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", locales_dir.display()))
    {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let stem = path.file_stem().unwrap().to_str().unwrap().to_string();
        if stem == "i18n-allowlist" {
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());
        stems.push(stem);
    }
    stems.sort();
    if !stems.iter().any(|s| s == "en") {
        panic!("locales/en.toml missing (en is the reference locale)");
    }
    // Every locale file needs known plural-category rules before validation.
    for stem in &stems {
        plural_rule(stem);
    }

    let mut parsed: BTreeMap<String, (Sections, Maps)> = BTreeMap::new();
    for stem in &stems {
        let path = locales_dir.join(format!("{stem}.toml"));
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let table = text
            .parse::<toml::Table>()
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        parsed.insert(stem.clone(), parse_locale(stem, table));
    }
    let (en_sections, en_maps) = parsed.remove("en").unwrap();
    validate_en(&en_sections);
    for (stem, (sections, maps)) in &parsed {
        validate_against_en(stem, &en_sections, &en_maps, sections, maps);
    }
    parsed.insert("en".to_string(), (en_sections, en_maps));

    let mut out = String::from("// @generated by build.rs from locales/*.toml -- do not edit\n\n");

    // Section structs.
    for (sec, entries) in &parsed["en"].0 {
        let ty = format!("{}Strings", pascal(sec));
        out.push_str(&format!("pub(crate) struct {ty} {{\n"));
        for (key, entry) in entries {
            match entry {
                Entry::Scalar(_) => out.push_str(&format!("    pub(crate) {key}: &'static str,\n")),
                Entry::Array(items) => out.push_str(&format!(
                    "    pub(crate) {key}: [&'static str; {}],\n",
                    items.len()
                )),
                Entry::Template(..) => {
                    out.push_str(&format!("    __{key}: &'static str,\n"))
                }
                Entry::Plural(..) => {
                    out.push_str(&format!("    __{key}: crate::i18n::Plural,\n"))
                }
            }
        }
        out.push_str("}\n\n");
        // Methods for parameterized entries.
        let mut impls = String::new();
        for (key, entry) in entries {
            let (params, plural) = match entry {
                Entry::Template(_, ph) => (ph.iter().cloned().collect::<Vec<_>>(), false),
                Entry::Plural(cats) => {
                    let mut ph: BTreeSet<String> = BTreeSet::new();
                    for (_, p) in cats.values() {
                        ph.extend(p.iter().cloned());
                    }
                    (ph.into_iter().collect(), true)
                }
                _ => continue,
            };
            let sig: Vec<String> = params
                .iter()
                .map(|p| {
                    if plural && p == "count" {
                        "count: impl crate::i18n::PluralCount".to_string()
                    } else {
                        format!("{p}: impl std::fmt::Display")
                    }
                })
                .collect();
            impls.push_str(&format!(
                "    pub(crate) fn {key}(&self, {}) -> String {{\n",
                sig.join(", ")
            ));
            let args: Vec<String> = params
                .iter()
                .map(|p| format!("({p:?}, &{p} as &dyn std::fmt::Display)"))
                .collect();
            if plural {
                impls.push_str(&format!(
                    "        let template = crate::i18n::pick_plural(&self.__{key}, count);\n        crate::i18n::render(template, &[{}])\n",
                    args.join(", ")
                ));
            } else {
                impls.push_str(&format!(
                    "        crate::i18n::render(self.__{key}, &[{}])\n",
                    args.join(", ")
                ));
            }
            impls.push_str("    }\n");
        }
        if !impls.is_empty() {
            out.push_str(&format!("impl {ty} {{\n{impls}}}\n\n"));
        }
    }

    // Maps struct.
    out.push_str("pub(crate) struct Maps {\n");
    for map_name in parsed["en"].1.keys() {
        out.push_str(&format!(
            "    pub(crate) {map_name}: &'static [(&'static str, &'static str)],\n"
        ));
    }
    out.push_str("}\n\n");

    // Aggregate.
    out.push_str("pub(crate) struct Strings {\n");
    for sec in parsed["en"].0.keys() {
        out.push_str(&format!("    pub(crate) {sec}: {}Strings,\n", pascal(sec)));
    }
    out.push_str("    pub(crate) maps: Maps,\n}\n\n");

    // Per-locale statics (en first for stable output order).
    let mut ordered: Vec<String> = vec!["en".to_string()];
    ordered.extend(stems.iter().filter(|s| s.as_str() != "en").cloned());
    for stem in &ordered {
        let (sections, maps) = &parsed[stem];
        let name = scream(stem);
        out.push_str(&format!("static {name}: Strings = Strings {{\n"));
        for (sec, entries) in sections {
            out.push_str(&format!("    {sec}: {}Strings {{\n", pascal(sec)));
            for (key, entry) in entries {
                match entry {
                    Entry::Scalar(s) => {
                        out.push_str(&format!("        {key}: {},\n", rust_lit(s)))
                    }
                    Entry::Array(items) => {
                        let lits: Vec<String> = items.iter().map(|s| rust_lit(s)).collect();
                        out.push_str(&format!("        {key}: [{}],\n", lits.join(", ")));
                    }
                    Entry::Template(t, _) => {
                        out.push_str(&format!("        __{key}: {},\n", rust_lit(t)))
                    }
                    Entry::Plural(cats) => {
                        let get = |c: &str| match cats.get(c) {
                            Some((s, _)) => format!("Some({})", rust_lit(s)),
                            None => "None".to_string(),
                        };
                        out.push_str(&format!(
                            "        __{key}: crate::i18n::Plural {{ zero: {}, one: {}, few: {}, many: {}, other: {} }},\n",
                            get("zero"),
                            get("one"),
                            get("few"),
                            get("many"),
                            rust_lit(&cats["other"].0)
                        ));
                    }
                }
            }
            out.push_str("    },\n");
        }
        out.push_str("    maps: Maps {\n");
        for (map_name, map) in maps {
            let entries: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("({}, {})", rust_lit(k), rust_lit(v)))
                .collect();
            out.push_str(&format!(
                "        {map_name}: &[{}],\n",
                entries.join(", ")
            ));
        }
        out.push_str("    },\n};\n\n");
    }

    // table_for: variant names come from file stems, so a locale file without a
    // matching `Locale` variant (or vice versa) fails to compile.
    out.push_str(
        "pub(crate) fn table_for(locale: Locale) -> &'static Strings {\n    match locale {\n",
    );
    for stem in &ordered {
        out.push_str(&format!(
            "        Locale::{} => &{},\n",
            pascal(stem),
            scream(stem)
        ));
    }
    out.push_str("    }\n}\n");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(out_dir.join("strings.rs"), out).unwrap();
}
