//! The interface's texts, one file per language in `locales/` (how to add one: locales/README.md).
//!
//! A file is a small part of TOML: `[section]` lines, `key = "text"` lines and `#` comments.
//! A key is its section and its name ("tab.media"). Texts take `{name}` for values; a count's
//! text has `.one` and `.other` keys. A key missing from a language falls back to the base one.
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

pub struct Language {
    /// BCP 47, as Windows names its display languages ("pt-BR").
    pub code: &'static str,
    /// Its own name, as the settings' picker shows it.
    pub name: &'static str,
    file: &'static str,
}

/// The languages, the base first: every other one has its keys, and falls back to it.
pub const LANGUAGES: &[Language] = &[
    Language { code: "en-US", name: "English", file: include_str!("../../../locales/en-US.toml") },
    Language { code: "pt-BR", name: "Português (Brasil)", file: include_str!("../../../locales/pt-BR.toml") },
];

/// pt-BR until the app sets one (Cimori's texts were first written in it; the tests read them).
static CURRENT: AtomicUsize = AtomicUsize::new(1);

type Table = HashMap<String, &'static str>;

fn tables() -> &'static [Table] {
    static TABLES: OnceLock<Vec<Table>> = OnceLock::new();
    TABLES.get_or_init(|| {
        LANGUAGES.iter().map(|l| parse(l.file).unwrap_or_else(|e| panic!("locales/{}.toml: {e}", l.code))).collect()
    })
}

/// The file's keys and texts; an error names the line that is not one of the three kinds.
pub fn parse(file: &str) -> Result<HashMap<String, &'static str>, String> {
    let mut table = HashMap::new();
    let mut section = String::new();
    for (n, line) in file.lines().enumerate() {
        let line = line.trim();
        let err = |why: &str| format!("line {}: {why}", n + 1);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            if !valid_key(name) { return Err(err("bad section name")); }
            section = format!("{name}.");
            continue;
        }
        let (key, value) = line.split_once('=').ok_or_else(|| err("expected key = \"text\""))?;
        let key = key.trim();
        if !valid_key(key) { return Err(err("bad key")); }
        let text = unquote(value.trim()).ok_or_else(|| err("the text must be one \"quoted\" string"))?;
        // Leaked once per text at the first lookup: the tables live as long as the app.
        if table.insert(format!("{section}{key}"), &*Box::leak(text.into_boxed_str())).is_some() {
            return Err(err("key given twice"));
        }
    }
    Ok(table)
}

fn valid_key(k: &str) -> bool {
    !k.is_empty() && k.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
}

/// A TOML basic string: `\"`, `\\`, `\n` and `\t` escapes.
fn unquote(v: &str) -> Option<String> {
    let inner = v.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push(match chars.next()? {
                'n' => '\n',
                't' => '\t',
                '"' => '"',
                '\\' => '\\',
                _ => return None,
            }),
            '"' => return None,
            c => out.push(c),
        }
    }
    Some(out)
}

/// The language a setting picks: its code, or "" for Windows' display languages (`system`, in
/// Windows' order), matched whole and then by their first part ("pt-PT" gets pt-BR); else the base.
pub fn resolve(setting: &str, system: &[String]) -> usize {
    let wanted: Vec<&str> = if setting.is_empty() { system.iter().map(String::as_str).collect() } else { vec![setting] };
    let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    let first = |c: &str| c.split('-').next().unwrap_or("").to_owned();
    wanted.iter().find_map(|w| LANGUAGES.iter().position(|l| same(l.code, w)))
        .or_else(|| wanted.iter().find_map(|w| LANGUAGES.iter().position(|l| same(&first(l.code), &first(w)))))
        .unwrap_or(0)
}

/// The language the texts come from now (`LANGUAGES`' index).
pub fn set(index: usize) {
    CURRENT.store(index.min(LANGUAGES.len() - 1), Ordering::Relaxed);
}
pub fn current() -> &'static Language {
    &LANGUAGES[CURRENT.load(Ordering::Relaxed)]
}

/// The text of `key` in the language now, or in the base one.
pub fn lookup(key: &str) -> Option<&'static str> {
    let t = tables();
    t[CURRENT.load(Ordering::Relaxed)].get(key).or_else(|| t[0].get(key)).copied()
}

/// The text of `key`; a key no language has is shown as it is (labels such as "24 h").
pub fn tr(key: &'static str) -> &'static str {
    lookup(key).unwrap_or(key)
}

/// A text kept as a key (data made before the language changed): its text now, or itself.
pub fn tr_or(key_or_text: &str) -> &str {
    lookup(key_or_text).unwrap_or(key_or_text)
}

/// The text of `key` with its `{name}`s filled in.
pub fn trf(key: &str, args: &[(&str, &dyn Display)]) -> String {
    fill(lookup(key).unwrap_or(key), args)
}

/// `key.one` for a count of 1, `key.other` otherwise; `{n}` is the count.
pub fn trn(key: &str, n: i64, args: &[(&str, &dyn Display)]) -> String {
    let form = if n == 1 { "one" } else { "other" };
    let text = lookup(&format!("{key}.{form}")).unwrap_or(key);
    fill(&fill(text, args), &[("n", &n)])
}

/// The `i`th of a `|`-separated list ("dom|seg|..."); "" past its end.
pub fn nth(key: &'static str, i: usize) -> &'static str {
    tr(key).split('|').nth(i).unwrap_or("")
}

fn fill(text: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = text.to_owned();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), &value.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholders(text: &str) -> Vec<String> {
        let mut found: Vec<String> = text.split('{').skip(1).filter_map(|s| s.split_once('}').map(|(n, _)| n.to_owned())).collect();
        found.sort();
        found
    }

    /// Every language has exactly the base's keys, with the same `{values}` and list lengths.
    #[test]
    fn every_language_has_the_base_keys() {
        let t = tables();
        for (lang, table) in LANGUAGES.iter().zip(t).skip(1) {
            let mut missing: Vec<_> = t[0].keys().filter(|k| !table.contains_key(*k)).collect();
            let mut extra: Vec<_> = table.keys().filter(|k| !t[0].contains_key(*k)).collect();
            missing.sort();
            extra.sort();
            assert!(missing.is_empty(), "locales/{}.toml lacks {missing:?}", lang.code);
            assert!(extra.is_empty(), "locales/{}.toml has keys the base does not: {extra:?}", lang.code);
            for (key, base) in &t[0] {
                let text = table[key];
                assert_eq!(placeholders(base), placeholders(text), "locales/{}.toml: {key} has other {{values}}", lang.code);
                assert_eq!(base.matches('|').count(), text.matches('|').count(), "locales/{}.toml: {key} has another list length", lang.code);
            }
        }
    }

    #[test]
    fn pt_br_is_the_language_before_the_app_picks_one() {
        assert_eq!(LANGUAGES[1].code, "pt-BR");
    }

    #[test]
    fn the_files_parse_and_values_fill_in() {
        let t = parse("# c\n[a]\nb = \"x {n} \\\"y\\\"\"\n[c.d]\ne = \"z\"\n").unwrap();
        assert_eq!(t["a.b"], "x {n} \"y\"");
        assert_eq!(t["c.d.e"], "z");
        assert!(parse("a = x").is_err());
        assert!(parse("a = \"x\"\na = \"y\"").is_err());
        assert_eq!(fill("{a} e {b}", &[("a", &1), ("b", &"dois")]), "1 e dois");
    }

    #[test]
    fn the_system_language_is_matched_whole_then_by_its_first_part() {
        let code = |s: &str, sys: &[&str]| LANGUAGES[resolve(s, &sys.iter().map(|s| s.to_string()).collect::<Vec<_>>())].code;
        assert_eq!(code("", &["pt-BR"]), "pt-BR");
        assert_eq!(code("", &["pt-PT", "en-GB"]), "pt-BR");
        assert_eq!(code("", &["de-DE", "en-GB"]), "en-US");
        assert_eq!(code("", &["ja-JP"]), "en-US");
        assert_eq!(code("", &[]), "en-US");
        assert_eq!(code("en-US", &["pt-BR"]), "en-US");
        assert_eq!(code("pt-BR", &["en-US"]), "pt-BR");
    }
}
