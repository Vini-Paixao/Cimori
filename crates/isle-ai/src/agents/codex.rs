// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 zickrian
// Adapted for isle-win from zickrian/isle @ a29a7c4 (crates/isle-daemon/src/agents/codex.rs and claude.rs's
// quoting): hooks.json only. shortcut: no `notify` fallback for Codex < 0.124; port it if an older Codex shows up.

//! Codex: `Cimori --island-hook codex` hooks in hooks.json for approvals and done.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::json_hooks::{self, Spec};

/// The hook events Isle listens to; an approval waits for the user's answer up to ten minutes.
pub const SPEC: Spec = Spec {
    events: &[("PermissionRequest", Some(600)), ("Stop", None)],
    mark: "--island-hook codex",
    retired: &[],
};

/// `$CODEX_HOME/hooks.json`, else `~/.codex/hooks.json`.
pub fn hooks_path() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .filter(|dir| !dir.is_empty())
        .map_or_else(
            || std::env::home_dir().unwrap_or_default().join(".codex"),
            PathBuf::from,
        )
        .join("hooks.json")
}

pub fn hooked(path: &Path) -> bool {
    json_hooks::hooked(path, &SPEC)
}

pub fn outdated(path: &Path) -> Option<Option<String>> {
    json_hooks::outdated(path, &SPEC)
}

pub fn install(path: &Path, hook: &Value) -> anyhow::Result<()> {
    json_hooks::install(path, hook, &SPEC)
}

pub fn remove(path: &Path) -> anyhow::Result<bool> {
    json_hooks::remove(path, &SPEC)
}

/// The hook entry: `exe` quoted for the shell Codex runs it in, aimed at `host` (`None`: this machine).
pub fn hook(exe: &str, host: Option<&str>) -> Value {
    match crate::auth::configured_token() {
        Ok(token) => hook_with_token(exe, host, &token),
        Err(err) => { tracing::error!(%err, "Não foi possível preparar os hooks autenticados"); Value::Null }
    }
}

// Port of upstream's shell hook; the fixed hexadecimal token needs no shell escaping.
pub fn hook_with_token(exe: &str, host: Option<&str>, token: &str) -> Value {
    if !crate::auth::valid_token(token) { return Value::Null; }
    let mut command = format!("{} --island-hook codex", shell_quote(exe));
    if let Some(host) = host {
        command.push_str(&format!(" --host {}", shell_quote(host)));
    }
    command.push_str(&format!(" --token {token}"));
    json!({"type": "command", "command": command})
}

/// Single quotes unless the text is plain, so a folder with a space still runs.
fn shell_quote(text: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || "/._+-:".contains(c);
    if !text.is_empty() && text.chars().all(plain) {
        text.to_owned()
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::agents::file::tests::scratch;
    fn hook(exe: &str, host: Option<&str>) -> Value {
        super::hook_with_token(exe, host, "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
    }

    #[test]
    fn odd_paths_are_quoted() {
        assert_eq!(
            hook("/usr/bin/isle-win", Some("100.64.0.1:47830"))["command"],
            "/usr/bin/isle-win --island-hook codex --host 100.64.0.1:47830 --token 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
        assert_eq!(
            hook("/home/a b/it's/isle-win", None)["command"],
            r"'/home/a b/it'\''s/isle-win' --island-hook codex --token 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
    }

    #[test]
    fn a_hook_running_isle_win_is_outdated() {
        let dir = scratch("codex-renamed");
        let path = dir.join("hooks.json");
        let old = hook("/home/a b/isle-win", Some("100.64.0.1:47830"));
        fs::write(&path, json!({"hooks": {"Stop": [{"hooks": [old]}]}}).to_string()).unwrap();
        assert_eq!(outdated(&path), Some(Some("100.64.0.1:47830".into())));
        install(&path, &hook(std::env::current_exe().unwrap().to_str().unwrap(), None)).unwrap();
        assert_eq!(outdated(&path), None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn hooks_json_is_made_and_emptied_again() {
        let dir = scratch("codex-hooks");
        let path = dir.join("hooks.json");
        install(&path, &hook(std::env::current_exe().unwrap().to_str().unwrap(), None)).unwrap();
        assert!(hooked(&path));
        let settings: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(settings["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"], 600);
        assert!(remove(&path).unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{}\n");
        fs::remove_dir_all(dir).unwrap();
    }
}
