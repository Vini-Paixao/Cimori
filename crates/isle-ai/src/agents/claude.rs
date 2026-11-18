// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 zickrian
// Adapted for isle-win from zickrian/isle @ a29a7c4 (crates/isle-daemon/src/agents/claude.rs): the hook is
// exec form (no shell), so the same entry runs under Git Bash, PowerShell or sh, spaces in the path or not.

//! Claude Code: hooks in its settings.json running `Cimori --island-hook claude`: `PermissionRequest` (its
//! permission prompt, answered from the island), `Notification` (it waits for the user) and `Stop` (done).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::json_hooks::{self, Spec};

/// What marks a hook as Isle's, whichever isle-win it runs.
/// Not upstream's `--agent-hook`: the GNOME isle (isled) may hook the same Claude, and neither may take the other's.
const MARK: &str = "--island-hook claude";
/// The hook events Isle listens to; a permission waits for the user's answer up to ten minutes.
pub const SPEC: Spec = Spec {
    events: &[
        ("PermissionRequest", Some(600)),
        ("Notification", None),
        ("Stop", None),
    ],
    mark: MARK,
    retired: &[],
};

/// Whether Isle's hooks are in the settings file.
pub fn hooked(path: &Path) -> bool {
    json_hooks::hooked(path, &SPEC)
}

/// Isle's hooks from before the token or the rename to Cimori, and the host they aim at.
pub fn outdated(path: &Path) -> Option<Option<String>> {
    json_hooks::outdated(path, &SPEC)
}

/// Adds Isle's hooks beside the user's own, keeping a backup of the file.
pub fn install(path: &Path, hook: &Value) -> anyhow::Result<()> {
    json_hooks::install(path, hook, &SPEC)
}

/// Takes Isle's hooks out again; false if none were there.
pub fn remove(path: &Path) -> anyhow::Result<bool> {
    json_hooks::remove(path, &SPEC)
}

/// `$CLAUDE_CONFIG_DIR/settings.json`, else `~/.claude/settings.json`.
pub fn settings_path() -> PathBuf {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .filter(|dir| !dir.is_empty())
        .map_or_else(
            || std::env::home_dir().unwrap_or_default().join(".claude"),
            PathBuf::from,
        )
        .join("settings.json")
}

/// The hook entry running `exe --island-hook claude`, aimed at the island on `host` (`None`: this machine).
pub fn hook(exe: &str, host: Option<&str>) -> Value {
    match crate::auth::configured_token() {
        Ok(token) => hook_with_token(exe, host, &token),
        Err(err) => { tracing::error!(%err, "Não foi possível preparar os hooks autenticados"); Value::Null }
    }
}

// Port of upstream's exec-form hook; the HTTP adaptation passes its shared token.
pub fn hook_with_token(exe: &str, host: Option<&str>, token: &str) -> Value {
    if !crate::auth::valid_token(token) { return Value::Null; }
    let mut args = vec!["--island-hook", "claude"];
    if let Some(host) = host {
        args.extend(["--host", host]);
    }
    args.extend(["--token", token]);
    json!({"type": "command", "command": exe, "args": args})
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::agents::file;
    use crate::agents::file::tests::scratch;
    use crate::agents::json_hooks::{add_hooks, remove_hooks};
    fn hook(exe: &str, host: Option<&str>) -> Value {
        super::hook_with_token(exe, host, "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
    }

    // A program that is there, as an installed isle-win would be (a hook to a missing one counts as out).
    fn ours() -> Value {
        hook(std::env::current_exe().unwrap().to_str().unwrap(), None)
    }

    #[test]
    fn hooks_go_in_beside_the_users_own() {
        let dir = scratch("claude-install");
        let path = dir.join("settings.json");
        let before = r#"{
  "model": "opus",
  "hooks": {
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "notify-send done"
          }
        ]
      }
    ],
    "PreToolUse": []
  },
  "env": {
    "B": "1",
    "A": "2"
  }
}
"#;
        fs::write(&path, before).unwrap();
        assert!(!hooked(&path));
        install(&path, &ours()).unwrap();
        assert!(hooked(&path));
        let text = fs::read_to_string(&path).unwrap();
        let settings: Value = serde_json::from_str(&text).unwrap();
        // Keys keep their order; the user's own hook stays first.
        let keys: Vec<&String> = settings.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["model", "hooks", "env"]);
        let env: Vec<&String> = settings["env"].as_object().unwrap().keys().collect();
        assert_eq!(env, ["B", "A"]);
        assert_eq!(
            settings["hooks"]["Stop"][0]["hooks"][0]["command"],
            "notify-send done"
        );
        assert_eq!(settings["hooks"]["Stop"][1], json!({"hooks": [ours()]}));
        let mut permission = ours();
        permission["timeout"] = json!(600);
        assert_eq!(
            settings["hooks"]["PermissionRequest"][0],
            json!({"hooks": [permission]})
        );
        assert!(text.contains("\n  \"model\": \"opus\",\n") && text.ends_with("}\n"));
        assert_eq!(
            fs::read_to_string(file::sibling(&path, ".isle-backup")).unwrap(),
            before
        );

        // Twice is once.
        install(&path, &ours()).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), text);

        // Off again: the file is as it was.
        assert!(remove(&path).unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
        assert!(!remove(&path).unwrap());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_missing_file_is_made_and_emptied_again() {
        let dir = scratch("claude-new");
        let path = dir.join("sub/settings.json");
        // An isle-win in a folder with a space; once it is gone, the hook counts as out.
        let exe = dir.join("my bin/isle-win");
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::write(&exe, "").unwrap();
        install(&path, &hook(&exe.to_string_lossy(), Some("100.64.0.1:47830"))).unwrap();
        assert!(hooked(&path));
        fs::remove_file(&exe).unwrap();
        assert!(!hooked(&path));
        assert!(!file::sibling(&path, ".isle-backup").exists());
        assert!(remove(&path).unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{}\n");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn hooks_from_before_the_token_are_found_with_their_host() {
        let dir = scratch("claude-tokenless");
        let path = dir.join("settings.json");
        let old = json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "C:/isle-win.exe",
            "args": ["--island-hook", "claude", "--host", "100.64.0.1:47830"]}]}]}});
        fs::write(&path, old.to_string()).unwrap();
        assert_eq!(outdated(&path), Some(Some("100.64.0.1:47830".into())));
        install(&path, &ours()).unwrap();
        assert_eq!(outdated(&path), None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn hooks_running_isle_win_are_found_and_moved_to_cimori() {
        let dir = scratch("claude-renamed");
        let path = dir.join("settings.json");
        let mut settings = json!({});
        add_hooks(&mut settings, &hook(r"C:\Users\a\isle-win\isle-win.exe", None), &SPEC).unwrap();
        fs::write(&path, settings.to_string()).unwrap();
        assert_eq!(outdated(&path), Some(None));
        install(&path, &ours()).unwrap();
        assert_eq!(outdated(&path), None);
        assert!(hooked(&path));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn bad_json_is_never_written() {
        let dir = scratch("claude-bad");
        let path = dir.join("settings.json");
        fs::write(&path, "{\"model\": ").unwrap();
        let err = install(&path, &ours()).unwrap_err();
        assert!(err.to_string().contains("not valid JSON"), "{err}");
        assert!(remove(&path).is_err());
        fs::write(&path, r#"{"hooks": []}"#).unwrap();
        assert!(install(&path, &ours()).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), r#"{"hooks": []}"#);
        assert!(!file::sibling(&path, ".isle-backup").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_moved_exe_or_new_host_is_followed() {
        let mut settings = json!({});
        assert!(add_hooks(&mut settings, &hook("/old/isle-win", None), &SPEC).unwrap());
        assert!(add_hooks(&mut settings, &ours(), &SPEC).unwrap());
        assert!(!add_hooks(&mut settings, &ours(), &SPEC).unwrap());
        let remote = hook(std::env::current_exe().unwrap().to_str().unwrap(), Some("100.64.0.1:47830"));
        assert!(add_hooks(&mut settings, &remote, &SPEC).unwrap());
        assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert_eq!(settings["hooks"]["Stop"][0]["hooks"][0], remote);
    }

    #[test]
    fn hooks_running_a_cimori_that_is_gone_are_outdated() {
        let dir = scratch("claude-gone");
        let path = dir.join("settings.json");
        let here = std::env::current_exe().unwrap();
        install(&path, &hook(here.to_str().unwrap(), None)).unwrap();
        assert_eq!(outdated(&path), None);
        let gone = dir.join("apagado").join("Cimori.exe");
        install(&path, &hook(gone.to_str().unwrap(), Some("100.64.0.1:47830"))).unwrap();
        assert_eq!(outdated(&path), Some(Some("100.64.0.1:47830".into())));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_gnome_isles_own_hook_is_never_touched() {
        // zickrian/isle's isled may hook the same Claude on Fedora; its mark is `--agent-hook claude`.
        let isled = json!({"type": "command", "command": "/home/a/.local/bin/isled --agent-hook claude"});
        let mut settings = json!({"hooks": {"Stop": [{"hooks": [isled.clone()]}]}});
        assert!(add_hooks(&mut settings, &ours(), &SPEC).unwrap());
        assert_eq!(settings["hooks"]["Stop"][0]["hooks"][0], isled);
        assert!(remove_hooks(&mut settings, &SPEC));
        assert_eq!(settings, json!({"hooks": {"Stop": [{"hooks": [isled]}]}}));
    }

    #[test]
    fn only_isles_hooks_are_removed() {
        let mut settings = json!({
            "hooks": {
                "Notification": [
                    {"hooks": [
                        {"type": "command", "command": "mine"},
                        ours()
                    ]}
                ],
                "Stop": [{"hooks": [ours()]}],
                "SessionStart": []
            }
        });
        assert!(remove_hooks(&mut settings, &SPEC));
        assert_eq!(
            settings,
            json!({
                "hooks": {
                    "Notification": [{"hooks": [{"type": "command", "command": "mine"}]}],
                    "SessionStart": []
                }
            })
        );
        assert!(!remove_hooks(&mut settings, &SPEC));
        // A list already empty is the user's to keep.
        let mut settings = json!({"hooks": {"Stop": []}});
        assert!(!remove_hooks(&mut settings, &SPEC));
        assert_eq!(settings, json!({"hooks": {"Stop": []}}));
    }
}
