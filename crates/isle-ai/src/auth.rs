//! HTTP adaptation of Isle's local D-Bus hooks (agents/hook.rs). The upstream
//! bus authenticates the session; Cimori's network transport uses a saved bearer.
use std::path::PathBuf;

pub fn settings_path() -> PathBuf {
    if let Some(path) = std::env::var_os("ISLE_SETTINGS_PATH") { PathBuf::from(path) }
    else if let Some(path) = std::env::var_os("APPDATA") { PathBuf::from(path).join(isle_core::data_dir::NAME).join("settings.json") }
    else { PathBuf::from("dist/settings.json") }
}

pub fn configured_token() -> anyhow::Result<String> {
    let path = settings_path();
    let mut cfg = isle_core::config::IsleConfig::load_or_import(&path, None)?;
    if cfg.ensure_hooks_token()? {
        isle_core::config::write_atomic(&path, &serde_json::to_string_pretty(&cfg)?)?;
    }
    cfg.hooks_token.ok_or_else(|| anyhow::anyhow!("Token dos hooks ausente nas configurações."))
}

pub fn valid_token(token: &str) -> bool { token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()) }

pub fn matches(expected: &str, supplied: Option<&str>) -> bool {
    let Some(value) = supplied.filter(|value| valid_token(value)) else { return false; };
    if !valid_token(expected) { return false; }
    // Fixed-size comparison, like share/session.rs::ct_eq. black_box keeps the
    // compiler from replacing the full scan with an early-exit string comparison.
    let mut difference = 0u8;
    for i in 0..64 { difference |= std::hint::black_box(expected.as_bytes()[i] ^ value.as_bytes()[i]); }
    std::hint::black_box(difference) == 0
}

pub fn has_token(hook: &serde_json::Value) -> bool {
    if let Some(args) = hook["args"].as_array() {
        return args.windows(2).any(|args| args[0] == "--token" && args[1].as_str().is_some_and(valid_token));
    }
    let words = hook["command"].as_str().unwrap_or_default().split_whitespace().collect::<Vec<_>>();
    words.windows(2).any(|words| words[0] == "--token" && valid_token(words[1]))
}
