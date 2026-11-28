# Cimori's languages

Every text Cimori shows lives here, one file per language: `en-US.toml` (the base) and
`pt-BR.toml`. The files are built into `Cimori.exe`; nothing is read from disk at run time.

## Adding a language

1. Copy `en-US.toml` to a file named after the language's [BCP 47](https://www.rfc-editor.org/info/bcp47)
   code, the way Windows names its display languages: `es-ES.toml`, `de-DE.toml`, `fr-FR.toml`.
2. Translate the text between the quotes. Leave the keys (left of `=`) and the `[sections]` as they are.
3. Add one line to `LANGUAGES` in [`crates/isle-core/src/i18n.rs`](../crates/isle-core/src/i18n.rs),
   with the code, the language's own name (as the settings' picker shows it) and the file:
   ```rust
   Language { code: "es-ES", name: "Español", file: include_str!("../../../locales/es-ES.toml") },
   ```
4. Run `cargo test --workspace`. A test fails, naming the keys, if your file lacks one of the base's
   keys, has one the base does not, or changes a `{value}` or the length of a `|` list.
5. Try it: `cargo run --release`, then Settings → General → Language. It changes at once.

With **System** picked, Cimori follows the Windows display language: the exact code first, then
the language alone (`es-MX` gets `es-ES`), and English when nothing matches.

## The format

A small part of TOML: `[section]` lines, `key = "text"` lines and `# comments`. A key is its
section and its name: `name` under `[pref.language]` is `pref.language.name`.

- **Values** in braces are filled in by the app: `"{name} finished"`. Keep every one of them; you
  may move them around the sentence.
- **Counts** have two keys, `.one` for 1 and `.other` for the rest:
  ```toml
  files.one = "{n} file"
  files.other = "{n} files"
  ```
- **Lists** are separated by `|` and keep their length and order (`date.weekdays` starts on Sunday,
  `date.months` on January).
- **Escapes**: `\"` for a quote, `\\` for a backslash, `\n` for a new line.
- **Search words** (`words = "..."` under `[pref.*]`) are extra words the settings' search finds a
  row by. They don't need to be a translation: put what someone in your language would type.

Labels that read the same in every language (`24 h`, `°C`, `Opus`) are written in the code, not here.

## Fixing a text

Change it in its file and open a pull request. For a text in English or Portuguese, change only
that file; the other language keeps its own wording.
