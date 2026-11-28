// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 zickrian
// Adapted for isle-win from zickrian/isle @ a29a7c4 (crates/isle-daemon/src/usage.rs): async reqwest instead of
// ureq, no D-Bus (the reader hands its numbers to a callback), no chrono (a small RFC 3339 reader), labels and
// notes in Portuguese, no Grok. Plan names from crates/isle-daemon/src/assistant/models.rs (claude_plan, codex_plan).

//! AI plan usage: Claude's and Codex's limits, read where each CLI's /usage or /status reads them, with the
//! sign-ins the CLIs keep on this machine. With the `claude_renew_login` option, Claude's is renewed when it ran
//! out and no Claude Code renewed it (Claude Code used on another computer); Codex's is never renewed. Codex's
//! session logs fill in between asks.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use isle_core::i18n::trn;
use isle_core::state::{UsageProvider as Provider, UsageWindow as Window, FROM_CODEX};
use serde_json::Value;

/// Codex's local logs are read each minute; Claude and ChatGPT are asked every 10, Claude backing off to
/// an hour after it refused, so their servers are never pressed.
const LOCAL_EVERY: Duration = Duration::from_secs(60);
const ASK_EVERY: Duration = Duration::from_secs(10 * 60);
const CLAUDE_BACKOFF: Duration = Duration::from_secs(60 * 60);
/// Offline, or Claude Code about to renew its sign-in: soon again.
const RETRY_SOON: Duration = Duration::from_secs(2 * 60);
const TIMEOUT: Duration = Duration::from_secs(10);
/// Codex's logs: only their last part, and only the newest few files.
const CODEX_TAIL: u64 = 256 * 1024;
const CODEX_FILES: usize = 8;
const CODEX_USAGE: &str = "https://chatgpt.com/backend-api/wham/usage";

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn home() -> PathBuf {
    std::env::home_dir().unwrap_or_default()
}

/// `$CLAUDE_CONFIG_DIR`, else `~/.claude`.
fn claude_dir() -> PathBuf {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .filter(|d| !d.is_empty())
        .map_or_else(|| home().join(".claude"), PathBuf::from)
}

/// `$CODEX_HOME`, else `~/.codex`.
fn codex_dir() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .filter(|d| !d.is_empty())
        .map_or_else(|| home().join(".codex"), PathBuf::from)
}

/// Why Claude's limits could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClaudeError {
    /// The sign-in ran out a moment ago; a running Claude Code (or Cimori, soon) renews it.
    NeedsRefresh,
    /// The sign-in ran out and Cimori may not renew it: only Claude Code opened here does.
    Expired,
    /// No sign-in, or the server turned it down.
    SignIn,
    /// No network (yet).
    Offline,
    /// The server failed or answered in a way not understood.
    Busy,
}

impl ClaudeError {
    /// A key of locales/, shown in the language of the moment.
    fn note(self) -> &'static str {
        match self {
            ClaudeError::NeedsRefresh => "usage.claude_renewing",
            ClaudeError::Expired => "usage.claude_expired",
            ClaudeError::SignIn => "usage.claude_sign_in",
            ClaudeError::Offline => "usage.offline",
            ClaudeError::Busy => "usage.claude_busy",
        }
    }

    /// How long until Claude is asked again.
    fn wait(self) -> Duration {
        match self {
            ClaudeError::NeedsRefresh | ClaudeError::Expired | ClaudeError::Offline => RETRY_SOON,
            ClaudeError::SignIn | ClaudeError::Busy => CLAUDE_BACKOFF,
        }
    }
}

fn client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        // Said plainly who asks, never another app.
        .user_agent(concat!("Cimori/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// Where Claude Code renews its sign-in, and as which app (read from Claude Code itself).
const CLAUDE_TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
const CLAUDE_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
/// A running Claude Code renews its sign-in before it runs out; only one out this long has no Claude Code
/// to renew it, so Cimori does, never racing it.
const RENEW_AFTER_MS: i64 = 5 * 60 * 1000;

/// Claude Code's sign-in; with `renew`, renewed in its own file when it ran out a while ago (when Claude Code
/// is used only on another computer, nothing else renews it here).
async fn claude_token(renew_login: bool) -> Result<String, ClaudeError> {
    let path = claude_dir().join(".credentials.json");
    let text = std::fs::read_to_string(&path).map_err(|_| ClaudeError::SignIn)?;
    let sign_in: Value = serde_json::from_str(&text).map_err(|_| ClaudeError::SignIn)?;
    let oauth = &sign_in["claudeAiOauth"];
    let token = oauth["accessToken"].as_str().ok_or(ClaudeError::SignIn)?;
    let Some(expires) = oauth["expiresAt"].as_i64().filter(|&e| e <= now_ms()) else {
        return Ok(token.to_owned());
    };
    if !renew_login {
        return Err(ClaudeError::Expired);
    }
    let refresh = oauth["refreshToken"].as_str().ok_or(ClaudeError::SignIn)?;
    if now_ms() - expires < RENEW_AFTER_MS {
        return Err(ClaudeError::NeedsRefresh);
    }
    let scopes: Vec<&str> = oauth["scopes"].as_array().map_or(Vec::new(), |s| s.iter().filter_map(Value::as_str).collect());
    let reply = client()
        .map_err(|_| ClaudeError::Busy)?
        .post(CLAUDE_TOKEN_URL)
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh,
            "client_id": CLAUDE_CLIENT_ID,
            "scope": scopes.join(" "),
        }))
        .send()
        .await
        .map_err(|_| ClaudeError::Offline)?;
    match reply.status().as_u16() {
        400 | 401 | 403 => return Err(ClaudeError::SignIn),
        s if !(200..300).contains(&s) => return Err(ClaudeError::Busy),
        _ => {}
    }
    let renewed: Value = reply.json().await.map_err(|_| ClaudeError::Busy)?;
    // Claude Code may have renewed it meanwhile: its file wins, ours is dropped.
    let now_text = std::fs::read_to_string(&path).map_err(|_| ClaudeError::SignIn)?;
    let mut now_sign_in: Value = serde_json::from_str(&now_text).map_err(|_| ClaudeError::SignIn)?;
    let now_oauth = &mut now_sign_in["claudeAiOauth"];
    if now_oauth["refreshToken"].as_str() != Some(refresh) {
        return now_oauth["accessToken"].as_str().map(str::to_owned).ok_or(ClaudeError::SignIn);
    }
    let token = renew(now_oauth, &renewed, now_ms()).ok_or(ClaudeError::Busy)?;
    // Written whole and then swapped in, so Claude Code never reads half a file.
    let temp = path.with_extension("json.cimori");
    // Owner-only from its first byte (on Windows, the profile folder's permissions are inherited).
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let written = serde_json::to_vec(&now_sign_in).ok().and_then(|bytes| {
        use std::io::Write;
        let mut file = options.open(&temp).ok()?;
        file.write_all(&bytes).and_then(|()| file.sync_all()).ok()?;
        drop(file);
        std::fs::rename(&temp, &path).ok()
    });
    if written.is_none() {
        std::fs::remove_file(&temp).ok();
        tracing::warn!("uso: login do Claude renovado, mas não gravado");
    } else {
        tracing::info!("uso: login do Claude renovado");
    }
    Ok(token)
}

/// Claude Code's `claudeAiOauth` after the token endpoint answered `renewed` at `now`; the new access token.
fn renew(oauth: &mut Value, renewed: &Value, now: i64) -> Option<String> {
    let token = renewed["access_token"].as_str()?.to_owned();
    let expires_in = renewed["expires_in"].as_i64()?;
    oauth["accessToken"] = token.clone().into();
    if let Some(refresh) = renewed["refresh_token"].as_str() {
        oauth["refreshToken"] = refresh.into();
    }
    oauth["expiresAt"] = (now + expires_in * 1000).into();
    if let Some(seconds) = renewed["refresh_token_expires_in"].as_i64() {
        oauth["refreshTokenExpiresAt"] = (now + seconds * 1000).into();
    }
    Some(token)
}

/// Claude's limits now, with Claude Code's sign-in.
async fn read_claude(renew_login: bool) -> Result<Vec<Window>, ClaudeError> {
    let token = claude_token(renew_login).await?;
    let reply = client()
        .map_err(|_| ClaudeError::Busy)?
        .get("https://api.anthropic.com/api/oauth/usage")
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .send()
        .await
        .map_err(|_| ClaudeError::Offline)?;
    match reply.status().as_u16() {
        401 | 403 => return Err(ClaudeError::SignIn),
        s if !(200..300).contains(&s) => return Err(ClaudeError::Busy),
        _ => {}
    }
    let response: Value = reply.json().await.map_err(|_| ClaudeError::Busy)?;
    Ok(claude_windows(&response))
}

/// The limits Claude reports for this plan, in its order: "5 horas", "Semana", "Semana · Opus"…
/// (as keys of locales/, a model after " · ": `window_text` shows them).
fn claude_windows(response: &Value) -> Vec<Window> {
    if let Some(limits) = response["limits"].as_array() {
        return limits
            .iter()
            .filter_map(|limit| {
                let label = match limit["group"].as_str().or(limit["kind"].as_str())? {
                    "session" => "usage.five_hours".to_owned(),
                    "weekly" | "weekly_all" | "weekly_scoped" => "usage.week".to_owned(),
                    other => title_case(other),
                };
                let model = limit["scope"]["model"]["display_name"]
                    .as_str()
                    .filter(|m| !m.is_empty());
                Some(Window {
                    label: model.map_or(label.clone(), |model| format!("{label} · {model}")),
                    percent: limit["percent"].as_f64()?,
                    resets_at_ms: limit["resets_at"].as_str().map_or(0, rfc3339_ms),
                })
            })
            .collect();
    }
    // The older shape: one object per window.
    [
        ("five_hour", "usage.five_hours"),
        ("seven_day", "usage.week"),
        ("seven_day_opus", "usage.week · Opus"),
    ]
    .iter()
    .filter_map(|(key, label)| {
        let window = response.get(*key).filter(|w| !w.is_null())?;
        Some(Window {
            label: (*label).into(),
            percent: window["utilization"].as_f64()?,
            resets_at_ms: window["resets_at"].as_str().map_or(0, rfc3339_ms),
        })
    })
    .collect()
}

/// ChatGPT's limits now, with Codex's sign-in; `None` when signed out, offline or refused.
async fn read_codex_live() -> Option<Provider> {
    let text = std::fs::read_to_string(codex_dir().join("auth.json")).ok()?;
    let sign_in: Value = serde_json::from_str(&text).ok()?;
    let token = sign_in["tokens"]["access_token"].as_str()?;
    let mut request = client().ok()?.get(CODEX_USAGE).bearer_auth(token);
    if let Some(account) = sign_in["tokens"]["account_id"].as_str() {
        request = request.header("ChatGPT-Account-Id", account);
    }
    let response: Value = request.send().await.ok()?.error_for_status().ok()?.json().await.ok()?;
    Some(codex_live(&response, now_ms()))
}

/// The answer of ChatGPT's usage endpoint as a provider read at `now`.
fn codex_live(response: &Value, now: i64) -> Provider {
    let limits = &response["rate_limit"];
    let windows = ["primary_window", "secondary_window"]
        .iter()
        .filter_map(|key| {
            let window = limits.get(*key).filter(|w| !w.is_null())?;
            Some(Window {
                label: window_label(window["limit_window_seconds"].as_i64().unwrap_or(0) / 60),
                percent: window["used_percent"].as_f64()?,
                resets_at_ms: window["reset_at"].as_i64().map_or(0, |s| s * 1000),
            })
        })
        .collect();
    Provider {
        id: "codex".into(),
        name: "Codex".into(),
        plan: codex_plan(response["plan_type"].as_str().unwrap_or_default()),
        windows,
        updated_ms: now,
        ..Provider::default()
    }
}

/// The newest limits in one Codex log, from its last part.
fn codex_limits(file: &Path) -> Option<Provider> {
    use std::io::{Read, Seek, SeekFrom};
    let mut log = std::fs::File::open(file).ok()?;
    let length = log.metadata().ok()?.len();
    log.seek(SeekFrom::Start(length.saturating_sub(CODEX_TAIL))).ok()?;
    let mut tail = Vec::new();
    log.read_to_end(&mut tail).ok()?;
    let text = String::from_utf8_lossy(&tail);
    let (limits, at) = text.lines().rev().find_map(|line| {
        // Most lines are messages; only parse the ones that can hold limits.
        if !line.contains("rate_limits") {
            return None;
        }
        let event: Value = serde_json::from_str(line).ok()?;
        let limits = find_key(&event, "rate_limits")?.clone();
        Some((limits, event["timestamp"].as_str().map_or(0, rfc3339_ms)))
    })?;
    let modified = std::fs::metadata(file)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as i64);
    Some(Provider {
        id: "codex".into(),
        name: "Codex".into(),
        plan: codex_plan(limits["plan_type"].as_str().unwrap_or_default()),
        windows: codex_windows(&limits),
        updated_ms: if at > 0 { at } else { modified },
        note: FROM_CODEX.into(),
    })
}

/// Codex's session logs, newest first: sessions/<year>/<month>/<day>/*.jsonl.
fn newest_logs(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    entries.sort_by(|a, b| b.cmp(a));
    for path in entries {
        if files.len() >= CODEX_FILES {
            return;
        }
        if path.is_dir() {
            newest_logs(&path, files);
        } else if path.extension().is_some_and(|e| e == "jsonl") {
            files.push(path);
        }
    }
}

/// The primary and secondary limits ChatGPT reports.
fn codex_windows(limits: &Value) -> Vec<Window> {
    ["primary", "secondary"]
        .iter()
        .filter_map(|key| {
            let window = limits.get(*key).filter(|w| !w.is_null())?;
            Some(Window {
                label: window_label(window["window_minutes"].as_i64().unwrap_or(0)),
                percent: window["used_percent"].as_f64()?,
                resets_at_ms: window["resets_at"].as_i64().map_or(0, |s| s * 1000),
            })
        })
        .collect()
}

/// A window of `minutes` as people say it: "5 horas", "Semana", "30 dias".
fn window_label(minutes: i64) -> String {
    match minutes {
        300 => "usage.five_hours".to_owned(),
        10080 => "usage.week".to_owned(),
        m if m > 0 && m % 1440 == 0 => trn("usage.days", m / 1440, &[]),
        m if m > 0 => trn("usage.hours", (m + 30) / 60, &[]),
        _ => "usage.limit".to_owned(),
    }
}

/// The first value under `key`, at any depth.
fn find_key<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => map
            .get(key)
            .filter(|v| !v.is_null())
            .or_else(|| map.values().find_map(|v| find_key(v, key))),
        Value::Array(items) => items.iter().find_map(|v| find_key(v, key)),
        _ => None,
    }
}

/// "2026-10-21T16:10:00.123+00:00" or "…Z" as Unix milliseconds; 0 when it does not parse.
fn rfc3339_ms(text: &str) -> i64 {
    let parse = || -> Option<i64> {
        let num = |s: &str| s.parse::<i64>().ok();
        let (date, rest) = text.trim().split_once(['T', 't', ' '])?;
        let mut d = date.splitn(3, '-');
        let (year, month, day) = (num(d.next()?)?, num(d.next()?)?, num(d.next()?)?);
        let (h, m) = (num(rest.get(0..2)?)?, num(rest.get(3..5)?)?);
        let s = num(rest.get(6..8)?)?;
        let mut tail = &rest[8..];
        let mut ms = 0;
        if let Some(frac) = tail.strip_prefix('.') {
            let digits: String = frac.chars().take_while(char::is_ascii_digit).collect();
            ms = num(&format!("{:0<3}", &digits[..digits.len().min(3)]))?;
            tail = &frac[digits.len()..];
        }
        let offset_min = match tail {
            "Z" | "z" | "" => 0,
            _ => {
                let sign = if tail.starts_with('-') { -1 } else { 1 };
                sign * (num(tail.get(1..3)?)? * 60 + num(tail.get(4..6)?)?)
            }
        };
        // Days from the civil date (Howard Hinnant).
        let y = year - i64::from(month <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
        let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
        Some(((days * 86_400 + h * 3600 + m * 60 + s - offset_min * 60) * 1000) + ms)
    };
    parse().unwrap_or(0)
}

/// A limit's kind as a label: "weekly_opus" reads "Weekly Opus".
fn title_case(text: &str) -> String {
    text.split(['_', ' '])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or(String::new(), |first| first.to_uppercase().chain(chars).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Port of claude_plan (isle): the plan of Claude Code's sign-in and of its account in `.claude.json`.
fn claude_plan() -> String {
    let read = |path: PathBuf| std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).unwrap_or_default();
    let state = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()).map_or_else(home, PathBuf::from).join(".claude.json");
    claude_plan_of(&read(claude_dir().join(".credentials.json"))["claudeAiOauth"], &read(state)["oauthAccount"])
}

/// Port of claude_plan_of (isle): the account's own name for it, else the sign-in's kind, as Claude Code says it.
fn claude_plan_of(sign_in: &Value, account: &Value) -> String {
    let named = ["plan_display_name", "planDisplayName"].iter()
        .find_map(|key| account[key].as_str().or(account["organization"][key].as_str()));
    if let Some(name) = named.filter(|name| !name.is_empty()) {
        return name.to_owned();
    }
    let text = |value: &Value| value.as_str().unwrap_or_default().to_lowercase();
    let mut kind = text(&sign_in["subscriptionType"]);
    if kind.is_empty() {
        kind = text(&account["organizationType"]).trim_start_matches("claude_").to_owned();
    }
    match kind.as_str() {
        "" => String::new(),
        "pro" => "Pro".into(),
        "max" => "Max".into(),
        "team" => "Team".into(),
        "enterprise" => "Enterprise".into(),
        other => title_case(other),
    }
}

/// Port of codex_plan (isle): ChatGPT's `plan_type` as ChatGPT names it.
fn codex_plan(plan: &str) -> String {
    match plan.to_lowercase().as_str() {
        "" => String::new(),
        "free" => "Free".into(),
        "go" => "Go".into(),
        "plus" => "Plus".into(),
        "prolite" | "pro_lite" => "Pro".into(),
        "pro" => "Pro (More)".into(),
        "promax" | "pro_max" => "Pro (Max)".into(),
        "team" => "Team".into(),
        "business" => "Business".into(),
        "enterprise" => "Enterprise".into(),
        "edu" => "Edu".into(),
        other => title_case(other),
    }
}

/// A window whose reset has passed starts over at 0, until the numbers are read again.
fn settle(provider: &mut Provider, now: i64) {
    for window in &mut provider.windows {
        if window.resets_at_ms > 0 && window.resets_at_ms <= now {
            window.percent = 0.0;
        }
    }
}

/// Reads both AIs' limits for as long as it runs, handing them to `publish` whenever they change;
/// `renew_login`: Cimori may renew Claude Code's sign-in (the `claude_renew_login` option).
pub async fn run(renew_login: bool, publish: impl Fn(Vec<Provider>) + Send + 'static) {
    let mut claude: Option<Provider> = None;
    let mut claude_due = Instant::now();
    let mut codex_live: Option<Provider> = None;
    let mut codex_due = Instant::now();
    let mut codex_seen: Option<(PathBuf, SystemTime)> = None;
    let mut codex_logged: Option<Provider> = None;
    let mut published: Option<Vec<Provider>> = None;
    loop {
        let now = Instant::now();
        if now >= claude_due {
            let shell = claude.clone().unwrap_or_else(|| Provider {
                id: "claude".into(),
                name: "Claude".into(),
                ..Provider::default()
            });
            let shell = Provider { plan: claude_plan(), ..shell };
            claude = Some(match read_claude(renew_login).await {
                Ok(windows) => {
                    claude_due = now + ASK_EVERY;
                    Provider { windows, updated_ms: now_ms(), note: String::new(), ..shell }
                }
                Err(err) => {
                    claude_due = now + err.wait();
                    Provider { note: err.note().into(), ..shell }
                }
            });
        }
        if now >= codex_due {
            codex_due = now + ASK_EVERY;
            // Failed (offline, signed out), the last answer stands and the logs fill in.
            if let Some(fresh) = read_codex_live().await {
                codex_live = Some(fresh);
            }
        }
        let mut files = Vec::new();
        newest_logs(&codex_dir().join("sessions"), &mut files);
        let modified = |f: &PathBuf| std::fs::metadata(f).and_then(|m| m.modified()).ok();
        files.sort_by_key(|f| std::cmp::Reverse(modified(f)));
        let newest = files.first().and_then(|f| Some((f.clone(), modified(f)?)));
        if newest.is_some() && newest != codex_seen {
            codex_seen = newest;
            codex_logged = files.iter().take(CODEX_FILES).find_map(|f| codex_limits(f));
        }
        let codex = match (codex_live.clone(), codex_logged.clone()) {
            (Some(live), Some(logged)) if logged.updated_ms > live.updated_ms => Some(logged),
            (Some(live), _) => Some(live),
            (None, logged) => logged,
        };

        let now_ms = now_ms();
        // Claude always (without its sign-in, its note says why); Codex once it has numbers.
        let mut providers: Vec<Provider> = claude.iter().cloned().chain(codex).collect();
        for provider in &mut providers {
            settle(provider, now_ms);
        }
        if published.as_ref() != Some(&providers) {
            publish(providers.clone());
            published = Some(providers);
        }
        tokio::time::sleep(LOCAL_EVERY).await;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_rfc3339_times_as_claude_and_codex_write_them() {
        assert_eq!(rfc3339_ms("1970-01-01T00:00:00Z"), 0);
        assert_eq!(rfc3339_ms("2026-10-21T16:10:00+00:00"), 1_792_599_000_000);
        assert_eq!(rfc3339_ms("2026-10-21T13:10:00-03:00"), 1_792_599_000_000);
        assert_eq!(rfc3339_ms("2026-10-21T16:10:00.123456Z"), 1_792_599_000_123);
        assert_eq!(rfc3339_ms("2024-02-29T12:00:00Z"), 1_709_208_000_000);
        assert_eq!(rfc3339_ms("ontem"), 0);
    }

    #[test]
    fn reads_chatgpts_limits_as_status_does() {
        // A free account, as the endpoint answered on 2026-10-26: one 30-day window.
        let free = json!({"plan_type": "free", "rate_limit": {"allowed": true, "limit_reached": false,
            "primary_window": {"used_percent": 82, "limit_window_seconds": 2_592_000,
                "reset_after_seconds": 2_464_312, "reset_at": 1_795_444_424},
            "secondary_window": null}});
        let codex = codex_live(&free, 1_000);
        assert_eq!(codex.id, "codex");
        assert_eq!(codex.updated_ms, 1_000);
        assert_eq!(
            codex.windows,
            [Window { label: "30 dias".into(), percent: 82.0, resets_at_ms: 1_795_444_424_000 }]
        );
        // A paid one: 5 hours and a week.
        let pro = json!({"plan_type": "prolite", "rate_limit": {
            "primary_window": {"used_percent": 5, "limit_window_seconds": 18_000, "reset_at": 10},
            "secondary_window": {"used_percent": 23, "limit_window_seconds": 604_800, "reset_at": 20}}});
        let labels: Vec<String> = codex_live(&pro, 0).windows.into_iter().map(|w| w.text()).collect();
        assert_eq!(labels, ["5 horas", "Semana"]);
        assert_eq!((codex_live(&free, 0).plan, codex_live(&pro, 0).plan), ("Free".into(), "Pro".into()));
    }

    #[test]
    fn names_the_plan_as_the_clis_do() {
        let plan = |kind: &str, account: Value| claude_plan_of(&json!({"subscriptionType": kind}), &account);
        assert_eq!(plan("pro", Value::Null), "Pro");
        assert_eq!(plan("max", Value::Null), "Max");
        assert_eq!(plan("team", json!({"organization": {"plan_display_name": "Team Plus"}})), "Team Plus");
        assert_eq!(plan("", json!({"organizationType": "claude_team"})), "Team");
        assert_eq!(plan("", Value::Null), "");
        assert_eq!(codex_plan("pro"), "Pro (More)");
        assert_eq!(codex_plan(""), "");
    }

    #[test]
    fn reads_claudes_limits_whatever_the_plan_has() {
        let team = json!({"five_hour": {"utilization": 21.0}, "limits": [
            {"kind": "session", "group": "session", "percent": 21, "resets_at": "2026-10-21T16:10:00+00:00"},
            {"kind": "weekly_scoped", "group": "weekly", "percent": 0, "resets_at": "2026-10-24T21:00:00+00:00",
             "scope": {"model": {"display_name": "Fable"}}}]});
        let windows = claude_windows(&team);
        assert_eq!(windows.len(), 2);
        assert_eq!((windows[0].text().as_str(), windows[0].percent), ("5 horas", 21.0));
        assert_eq!(windows[0].resets_at_ms, rfc3339_ms("2026-10-21T16:10:00+00:00"));
        assert_eq!(windows[1].text(), "Semana · Fable");
        let older = json!({"five_hour": {"utilization": 4.0, "resets_at": null}, "seven_day": {"utilization": 31.5},
            "seven_day_opus": null});
        let labels: Vec<String> = claude_windows(&older).into_iter().map(|w| w.text()).collect();
        assert_eq!(labels, ["5 horas", "Semana"]);
    }

    #[test]
    fn reads_what_codex_saw_in_its_logs() {
        let pro = json!({"primary": {"used_percent": 9.0, "window_minutes": 10080, "resets_at": 1793084579},
            "secondary": null, "plan_type": "prolite"});
        let windows = codex_windows(&pro);
        assert_eq!(windows.len(), 1);
        assert_eq!((windows[0].text().as_str(), windows[0].percent), ("Semana", 9.0));
        assert_eq!(windows[0].resets_at_ms, 1_793_084_579_000);
        let line = json!({"timestamp": "2026-10-21T08:59:00Z", "payload": {"info": {}, "rate_limits": pro}});
        assert_eq!(find_key(&line, "rate_limits").unwrap()["plan_type"], "prolite");

        // A real session file: the newest line with limits wins.
        let dir = std::env::temp_dir().join(format!("isle-codex-{}", std::process::id()));
        let day = dir.join("sessions/2026/11/10");
        std::fs::create_dir_all(&day).unwrap();
        let older = json!({"timestamp": "2026-11-10T10:00:00Z", "payload": {"rate_limits":
            {"primary": {"used_percent": 1.0, "window_minutes": 300}}}});
        std::fs::write(day.join("a.jsonl"), format!("{older}\n{line}\n{{\"type\":\"message\"}}\n")).unwrap();
        let mut files = Vec::new();
        newest_logs(&dir.join("sessions"), &mut files);
        let logged = codex_limits(&files[0]).unwrap();
        assert_eq!(logged.windows[0].percent, 9.0);
        assert_eq!(logged.updated_ms, rfc3339_ms("2026-10-21T08:59:00Z"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_renewed_sign_in_keeps_what_claude_code_wrote() {
        let mut oauth = json!({"accessToken": "old", "refreshToken": "r1", "expiresAt": 1,
            "scopes": ["user:inference"], "subscriptionType": "pro"});
        let renewed = json!({"access_token": "new", "refresh_token": "r2", "expires_in": 3600,
            "refresh_token_expires_in": 7200});
        assert_eq!(renew(&mut oauth, &renewed, 1_000).as_deref(), Some("new"));
        assert_eq!(oauth["refreshToken"], "r2");
        assert_eq!(oauth["expiresAt"], 3_601_000);
        assert_eq!(oauth["refreshTokenExpiresAt"], 7_201_000);
        assert_eq!(oauth["subscriptionType"], "pro");
        // No new refresh token: the old one stays; an error answer changes nothing.
        let mut kept = json!({"refreshToken": "r1"});
        renew(&mut kept, &json!({"access_token": "a", "expires_in": 1}), 0).unwrap();
        assert_eq!(kept["refreshToken"], "r1");
        assert!(renew(&mut kept, &json!({"error": "invalid_grant"}), 0).is_none());
        assert_eq!(kept["accessToken"], "a");
    }

    #[test]
    fn a_window_past_its_reset_starts_over() {
        let mut provider = Provider {
            windows: vec![
                Window { label: "5 horas".into(), percent: 97.0, resets_at_ms: 1_000 },
                Window { label: "Semana".into(), percent: 40.0, resets_at_ms: 9_000 },
            ],
            ..Provider::default()
        };
        settle(&mut provider, 2_000);
        assert_eq!((provider.windows[0].percent, provider.windows[1].percent), (0.0, 40.0));
    }

    #[test]
    fn failures_wait_as_long_as_they_deserve() {
        assert_eq!(ClaudeError::Offline.wait(), RETRY_SOON);
        assert_eq!(ClaudeError::NeedsRefresh.wait(), RETRY_SOON);
        assert_eq!(ClaudeError::SignIn.wait(), CLAUDE_BACKOFF);
        assert_eq!(ClaudeError::Expired.wait(), RETRY_SOON);
        // Off, the note says how to get the numbers back, so it never looks like a bug.
        let note = isle_core::i18n::tr(ClaudeError::Expired.note());
        assert!(note.contains("abra o Claude Code"));
        assert!(note.contains("Renovar o login do Claude sozinha"));
    }
}
