// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 zickrian
// Adapted from zickrian/isle @ a29a7c4 (crates/isle-daemon/src/agents/json_hooks.rs): takes the whole
// hook, so Claude's can use exec form (`command` + `args`, no shell: same on Windows and Linux).

//! Isle's hooks in a JSON hooks file of the shape Claude Code's settings.json and Codex's hooks.json share.
//! Everything else in the file is left as it was, in its order.

use std::path::Path;

use anyhow::{Context, bail};
use serde_json::{Value, json};

use super::file;

/// Which hooks are Isle's: the events with the time (s) each may take, the mark in their command, and
/// events Isle no longer uses, whose hooks go when it is turned off.
pub struct Spec {
    pub events: &'static [(&'static str, Option<u64>)],
    pub mark: &'static str,
    pub retired: &'static [&'static str],
}

/// Whether all of Isle's hooks of `spec` are in the file.
pub fn hooked(path: &Path, spec: &Spec) -> bool {
    file::read(path)
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .is_some_and(|settings| has_hooks(&settings, spec))
}

/// Isle's hooks to put in again as they were: written before the token, which the island now refuses, or
/// running the app under its old name (isle-win.exe, now Cimori.exe). `Some(host)` with the `--host` they
/// aim at (`None`: this machine).
pub fn outdated(path: &Path, spec: &Spec) -> Option<Option<String>> {
    let settings: Value = serde_json::from_str(&file::read(path).ok()??).ok()?;
    let hook = spec.events.iter().flat_map(|(event, _)| hooks_of(&settings, event))
        .find(|hook| is_ours(hook, spec) && (!crate::auth::has_token(hook) || runs_old_name(hook) || runs_missing_program(hook)))?;
    let words: Vec<&str> = match hook["args"].as_array() {
        Some(args) => args.iter().filter_map(Value::as_str).collect(),
        None => hook["command"].as_str().unwrap_or_default().split_whitespace().collect(),
    };
    Some(words.windows(2).find(|w| w[0] == "--host").map(|w| w[1].trim_matches('\'').to_owned()))
}

/// Adds Isle's hooks (or updates them to `hook`); a file that is not JSON is never written.
pub fn install(path: &Path, hook: &Value, spec: &Spec) -> anyhow::Result<()> {
    if !crate::auth::has_token(hook) { bail!("Token dos hooks ausente ou inválido; confira as configurações antes de instalar."); }
    let text = file::read(path).with_context(|| format!("could not read {}", path.display()))?;
    let mut settings = parse(path, text.as_deref())?;
    if add_hooks(&mut settings, hook, spec).with_context(|| format!("in {}", path.display()))? {
        save(path, &settings, text.as_deref())?;
    }
    Ok(())
}

/// Takes Isle's hooks out; whether there were any.
pub fn remove(path: &Path, spec: &Spec) -> anyhow::Result<bool> {
    let text = file::read(path).with_context(|| format!("could not read {}", path.display()))?;
    let Some(text) = text else {
        return Ok(false);
    };
    let mut settings = parse(path, Some(&text))?;
    let removed = remove_hooks(&mut settings, spec);
    if removed {
        save(path, &settings, Some(&text))?;
    }
    Ok(removed)
}

/// The file's JSON; a missing or empty file is an empty object.
fn parse(path: &Path, text: Option<&str>) -> anyhow::Result<Value> {
    match text {
        Some(text) if !text.trim().is_empty() => serde_json::from_str(text)
            .with_context(|| format!("{} is not valid JSON; fix it first", path.display())),
        _ => Ok(json!({})),
    }
}

/// Written as the AIs write it: two spaces, and the old file's final newline.
fn save(path: &Path, settings: &Value, old: Option<&str>) -> anyhow::Result<()> {
    let mut text = serde_json::to_string_pretty(settings)?;
    if old.is_none_or(|old| old.ends_with('\n')) {
        text.push('\n');
    }
    file::backup_once(path).with_context(|| format!("could not back up {}", path.display()))?;
    file::write_atomic(path, &text).with_context(|| format!("could not write {}", path.display()))
}

/// Isle's hooks carry its mark in the command line (`command` and, in exec form, `args`).
fn is_ours(hook: &Value, spec: &Spec) -> bool {
    let args = hook["args"].as_array().into_iter().flatten();
    let line = std::iter::once(&hook["command"])
        .chain(args)
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    line.contains(spec.mark)
}

/// Every hook listed under `event`, whatever its matcher.
fn hooks_of<'a>(settings: &'a Value, event: &str) -> impl Iterator<Item = &'a Value> {
    settings["hooks"][event]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry["hooks"].as_array())
        .flatten()
}

/// Every event has Isle's hook, with the right timeout and an isled still there.
fn has_hooks(settings: &Value, spec: &Spec) -> bool {
    spec.events.iter().all(|(event, timeout)| {
        hooks_of(settings, event).any(|hook| {
            is_ours(hook, spec)
                && crate::auth::has_token(hook)
                && hook["timeout"].as_u64() == *timeout
                && runs_something(hook)
        })
    })
}

/// Whether the program a hook runs is still there (not moved by a reinstall): else it is put in again.
fn runs_something(hook: &Value) -> bool {
    Path::new(program(hook)).is_file()
}

/// Whether a hook runs the app by its name before Cimori, wherever that program is.
/// A Cimori moved or uninstalled (the portable folder elsewhere, an older version removed): its hooks
/// would fail silently, so the running one takes them over.
fn runs_missing_program(hook: &Value) -> bool {
    let program = program(hook);
    Path::new(program).is_absolute() && !Path::new(program).exists()
}

fn runs_old_name(hook: &Value) -> bool {
    Path::new(program(hook)).file_stem().is_some_and(|stem| stem.eq_ignore_ascii_case("isle-win"))
}

/// The program a hook runs.
fn program(hook: &Value) -> &str {
    let command = hook["command"].as_str().unwrap_or_default();
    // Exec form: `command` is the program itself, never quoted.
    if hook.get("args").is_some() {
        return command;
    }
    match command.strip_prefix('\'') {
        Some(quoted) => quoted.split('\'').next().unwrap_or_default(),
        None => command.split_whitespace().next().unwrap_or_default(),
    }
}

/// Whether anything changed.
pub(super) fn add_hooks(settings: &mut Value, hook: &Value, spec: &Spec) -> anyhow::Result<bool> {
    let Some(settings) = settings.as_object_mut() else {
        bail!("the settings are not a JSON object");
    };
    let Some(hooks) = settings
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
    else {
        bail!("`hooks` is not a JSON object");
    };
    let mut changed = false;
    for (event, timeout) in spec.events {
        let event = *event;
        let Some(list) = hooks
            .entry(event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
        else {
            bail!("`hooks.{event}` is not a list");
        };
        let ours = list
            .iter_mut()
            .filter_map(|entry| entry.get_mut("hooks").and_then(Value::as_array_mut))
            .flatten()
            .find(|hook| is_ours(hook, spec));
        let mut wanted = hook.clone();
        if let Some(timeout) = timeout {
            wanted["timeout"] = json!(timeout);
        }
        match ours {
            Some(hook) if *hook == wanted => {}
            // Kept, but run by this isled (an older one may be gone), with the time it may take.
            Some(hook) => {
                *hook = wanted;
                changed = true;
            }
            None => {
                list.push(json!({"hooks": [wanted]}));
                changed = true;
            }
        }
    }
    Ok(changed)
}

/// Whether anything changed. Only lists Isle emptied go, with `hooks` if that empties it.
pub(super) fn remove_hooks(settings: &mut Value, spec: &Spec) -> bool {
    let Some(settings) = settings.as_object_mut() else {
        return false;
    };
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else {
        return false;
    };
    let mut removed = false;
    let mut emptied = false;
    // Every event Isle ever used, so one it no longer uses goes too.
    for event in spec
        .events
        .iter()
        .map(|(event, _)| *event)
        .chain(spec.retired.iter().copied())
    {
        let Some(list) = hooks.get_mut(event).and_then(Value::as_array_mut) else {
            continue;
        };
        let mut found = false;
        list.retain_mut(|entry| {
            let Some(inner) = entry.get_mut("hooks").and_then(Value::as_array_mut) else {
                return true;
            };
            let count = inner.len();
            inner.retain(|hook| !is_ours(hook, spec));
            found |= inner.len() < count;
            !(inner.is_empty() && count > 0)
        });
        removed |= found;
        if found && list.is_empty() {
            hooks.shift_remove(event);
            emptied = true;
        }
    }
    if emptied && hooks.is_empty() {
        settings.shift_remove("hooks");
    }
    removed
}
