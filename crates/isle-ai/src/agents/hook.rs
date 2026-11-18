// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 zickrian
// Adapted for isle-win from zickrian/isle @ a29a7c4 (crates/isle-daemon/src/agents/hook.rs): D-Bus replaced by
// HTTP to the island (this machine or another one), no process tree (Linux-only, and useless across machines),
// no Grok, no Codex `notify` (Codex >= 0.124 has hooks).

//! `Cimori --island-hook claude|codex [--host H:P]`: run by the AI, it passes its hook events to the island
//! and asks it about permissions. With no answer (no island, no reply in time) it prints nothing: the AI
//! asks in its terminal.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use super::{Event, Request, folder_name, shows_whole};

/// The island on this machine, unless `--host` says otherwise.
pub const DEFAULT_HOST: &str = "127.0.0.1:47830";
/// All a report may take, connecting included; also the most connecting to the island may take.
const TIMEOUT: Duration = Duration::from_secs(1);
/// A permission waits this long for the island at most (the AIs give the hook ten minutes).
const ANSWER: Duration = Duration::from_secs(9 * 60 + 40);
/// The longest command or path the island asks about (it shows it whole, over a few lines).
const MAX_SHOWN: usize = 160;
/// Claude Code's payload is small; more than this is not read.
const MAX_INPUT: u64 = 1024 * 1024;
/// How often a waiting permission looks whether it was answered in the AI's own terminal instead.
const ANSWERED_EVERY: Duration = Duration::from_secs(1);
/// How long the tool's call may take to show in the transcript before that is no longer looked for.
const FIND_CALL_FOR: u32 = 6;
/// The end of the transcript searched for the tool's call, and the most read of it at a time.
const TRANSCRIPT_TAIL: u64 = 1024 * 1024;

/// Reads the AI's hook JSON from stdin and tells the island at `host`; prints Claude's answer if any.
pub async fn run(ai: &str, host: &str) {
    let args = std::env::args().collect::<Vec<_>>();
    let token = args.windows(2).find(|args| args[0] == "--token").map(|args| args[1].as_str());
    let Some(token) = token.filter(|token| crate::auth::valid_token(token)) else {
        // CLI hooks start before the main application's log subscriber.
        eprintln!("Cimori: hook antigo sem token válido; execute Cimori --install-hooks novamente.");
        return;
    };
    let ai = match ai {
        "claude" => "claude",
        "codex" => "codex",
        _ => return,
    };
    let mut input = Vec::new();
    let _ = std::io::stdin().take(MAX_INPUT).read_to_end(&mut input);
    let Ok(payload) = serde_json::from_slice::<Value>(&input) else {
        return;
    };
    if payload["hook_event_name"] == "PermissionRequest" {
        let Some(request) = request(ai, &payload) else {
            return;
        };
        // Answered in its terminal first, the AI leaves its hook waiting: it ends itself, and with it the
        // island's alert (the island sees the connection close).
        tokio::select! {
            answer = tokio::time::timeout(ANSWER, decide(host, &request, token)) => {
                if let Ok(Ok(answer)) = answer
                    && let Some(output) = decision(ai, &answer, &payload)
                {
                    println!("{output}");
                }
            }
            () = answered_elsewhere(&payload), if ai == "claude" => {}
        }
        return;
    }
    let event = match ai {
        "claude" => from_named(ai, &payload),
        _ => from_codex_hook(&payload),
    };
    if let Some(event) = event {
        let _ = tokio::time::timeout(TIMEOUT, send(host, &event, token)).await;
    }
}

fn client(total: Duration) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(TIMEOUT)
        .timeout(total)
        .build()
}

/// Asks the island and waits: "allow", "always", "deny", or "" for no answer.
async fn decide(host: &str, request: &Request, token: &str) -> reqwest::Result<String> {
    client(ANSWER)?
        .post(format!("http://{host}/agents/decide"))
        .bearer_auth(token)
        .json(request)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await
}

async fn send(host: &str, event: &Event, token: &str) -> reqwest::Result<()> {
    client(TIMEOUT)?
        .post(format!("http://{host}/agents/report"))
        .bearer_auth(token)
        .json(event)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

/// What the island shows of a permission: the tool, and what it would run or touch.
fn request(ai: &str, payload: &Value) -> Option<Request> {
    if !matches!(ai, "claude" | "codex") {
        return None;
    }
    let text = |key: &str| payload[key].as_str().unwrap_or_default();
    let input = &payload["tool_input"];
    let line = ["command", "file_path", "path", "url", "pattern", "query"]
        .iter()
        .find_map(|key| input[key].as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| readable(input));
    // Only what the island can show whole: more lines, hidden characters or more than fits, and the AI
    // asks in its terminal, where all of it shows.
    if !shows_whole(&line, MAX_SHOWN) {
        return None;
    }
    Some(Request {
        provider: ai.into(),
        cwd: cwd_or_here(text("cwd")),
        session: text("session_id").into(),
        tool: text("tool_name").into(),
        input: line,
        // Codex keeps no rule from its hook's answer: Always would only allow it this once.
        always: ai == "claude" && !rule_suggestions(payload).is_empty(),
        pids: ancestors(),
        ..Request::default()
    })
}

/// The hook's parent processes (the AI, its shell, the terminal or editor it runs in), nearest first.
// Port of isle's hook.rs `ancestors` (/proc there; Toolhelp's process list here).
#[cfg(windows)]
fn ancestors() -> Vec<u32> {
    use windows::Win32::{Foundation::CloseHandle, System::Diagnostics::ToolHelp::*};
    let mut parent = std::collections::HashMap::new();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return Vec::new() };
        let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut more = Process32FirstW(snapshot, &mut entry).is_ok();
        while more {
            parent.insert(entry.th32ProcessID, entry.th32ParentProcessID);
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
    }
    let mut pids = Vec::new();
    let mut pid = std::process::id();
    while let Some(&up) = parent.get(&pid) {
        if up == 0 || pids.contains(&up) || pids.len() >= 32 { break; }
        pids.push(up);
        pid = up;
    }
    pids
}
#[cfg(not(windows))]
fn ancestors() -> Vec<u32> { Vec::new() }

/// A tool's input with no command, path or link (an MCP tool's), as `key: value` pairs, all of them, so the
/// island shows what is asked without the JSON around it.
fn readable(input: &Value) -> String {
    match input {
        Value::Null => String::new(),
        Value::Object(fields) => fields
            .iter()
            .map(|(key, value)| match value {
                Value::String(text) => format!("{key}: {text}"),
                other => format!("{key}: {other}"),
            })
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string(),
    }
}

/// Resolves once the permission was answered in Claude Code's own terminal: the transcript has the call's
/// result. Codex waits for the hook instead. Never resolves without a transcript or the call in it.
async fn answered_elsewhere(payload: &Value) {
    let path = payload["transcript_path"].as_str().map(PathBuf::from);
    let mut found = None;
    for _ in 0..FIND_CALL_FOR {
        found = path
            .as_deref()
            .and_then(|path| pending_call(path, &payload["tool_name"], &payload["tool_input"]));
        if found.is_some() {
            break;
        }
        tokio::time::sleep(ANSWERED_EVERY).await;
    }
    let (Some(path), Some((id, mut read))) = (path, found) else {
        return std::future::pending().await;
    };
    loop {
        if read_on(&path, &mut read).is_some_and(|text| has_result(&text, &id)) {
            return;
        }
        tokio::time::sleep(ANSWERED_EVERY).await;
    }
}

/// As `find_call`, but only a call still waiting: an earlier, same call that has its result already means
/// this one is not written yet.
fn pending_call(path: &Path, tool: &Value, input: &Value) -> Option<(String, u64)> {
    let (id, read) = find_call(path, tool, input)?;
    let mut probe = read;
    let answered = read_on(path, &mut probe).is_some_and(|text| has_result(&text, &id));
    (!answered).then_some((id, read))
}

/// The id of the newest call of `tool` with `input` in the transcript at `path`, and where the lines
/// after it start.
fn find_call(path: &Path, tool: &Value, input: &Value) -> Option<(String, u64)> {
    let mut file = std::fs::File::open(path).ok()?;
    let start = file.metadata().ok()?.len().saturating_sub(TRANSCRIPT_TAIL);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.take(TRANSCRIPT_TAIL).read_to_end(&mut bytes).ok()?;
    // Whole lines only, newest first, each with where the next one starts.
    let mut line_end = bytes.iter().rposition(|&b| b == b'\n')? + 1;
    for line in bytes[..line_end].rsplit(|&b| b == b'\n').skip(1) {
        let next = start + line_end as u64;
        line_end = line_end.saturating_sub(line.len() + 1);
        let Ok(line) = std::str::from_utf8(line) else {
            continue;
        };
        if !line.contains("tool_use") {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let calls = entry["message"]["content"].as_array().into_iter().flatten();
        for call in calls.filter(|call| call["type"] == "tool_use" && &call["name"] == tool) {
            // The input as given, or at least its command: the AI may add a field of its own.
            let same = &call["input"] == input
                || (!input["command"].is_null() && call["input"]["command"] == input["command"]);
            if same && let Some(id) = call["id"].as_str() {
                return Some((id.to_owned(), next));
            }
        }
    }
    None
}

/// What was written to `path` since `read` (whole lines only), moving `read` on.
fn read_on(path: &Path, read: &mut u64) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    if len <= *read {
        return None;
    }
    file.seek(SeekFrom::Start(*read)).ok()?;
    let mut bytes = Vec::new();
    file.take(TRANSCRIPT_TAIL).read_to_end(&mut bytes).ok()?;
    let whole = bytes.iter().rposition(|&b| b == b'\n')? + 1;
    bytes.truncate(whole);
    *read += whole as u64;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Whether `lines` hold the result of the call `id`: run, or refused in the terminal.
fn has_result(lines: &str, id: &str) -> bool {
    lines
        .lines()
        .any(|line| line.contains(id) && line.contains("tool_result"))
}

/// The rules Claude suggests keeping for next time (this command, this folder); never a change of its
/// permission mode.
fn rule_suggestions(payload: &Value) -> Vec<Value> {
    payload["permission_suggestions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|rule| matches!(rule["type"].as_str(), Some("addRules" | "addDirectories")))
        .cloned()
        .collect()
}

/// The user's answer as the AI's hook output, or none (no answer: the AI asks itself). "always" keeps
/// the rule Claude itself suggested, where it suggested it.
fn decision(ai: &str, answer: &str, payload: &Value) -> Option<String> {
    let decision = match answer {
        "allow" => serde_json::json!({"behavior": "allow"}),
        "always" if ai == "claude" => {
            serde_json::json!({"behavior": "allow", "updatedPermissions": rule_suggestions(payload)})
        }
        "always" => serde_json::json!({"behavior": "allow"}),
        "deny" => {
            serde_json::json!({"behavior": "deny", "message": "The user denied this from the Cimori island."})
        }
        _ => return None,
    };
    Some(serde_json::json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": decision}}).to_string())
}

/// Codex's `Stop` hook: its turn is done.
fn from_codex_hook(payload: &Value) -> Option<Event> {
    let text = |key: &str| payload[key].as_str().unwrap_or_default();
    if text("hook_event_name") != "Stop" {
        return None;
    }
    let session = [text("session_id"), text("turn_id")]
        .into_iter()
        .find(|id| !id.is_empty())
        .unwrap_or_default();
    let message = said(text("last_assistant_message"));
    event("codex", "done", text("cwd"), &message, session)
}

/// Claude Code's `Notification` (needs a permission, or waits for the user) and `Stop` (done).
fn from_named(provider: &str, payload: &Value) -> Option<Event> {
    let text = |key: &str| payload[key].as_str().unwrap_or_default();
    let (kind, message) = match text("hook_event_name") {
        "Notification" => {
            // A permission too: the island skips it while it asks for that permission itself, and shows
            // it when it could not (a long command, the island busy).
            let kind = notification_kind(text("notification_type"), text("message"))?;
            (kind, text("message").to_owned())
        }
        "Stop" => ("done", said(text("last_assistant_message"))),
        _ => return None,
    };
    event(provider, kind, text("cwd"), &message, text("session_id"))
}

/// "permission" or "waiting" for a Claude Code notification; `None` for one the island ignores.
fn notification_kind(kind: &str, message: &str) -> Option<&'static str> {
    match kind {
        "permission_prompt" => Some("permission"),
        "idle_prompt" | "elicitation_dialog" => Some("waiting"),
        // Signed in again: nothing to do.
        "auth_success" => None,
        // Older Claude Codes send only the message.
        _ if message.to_lowercase().contains("permission") => Some("permission"),
        _ => Some("waiting"),
    }
}

/// What was said: the `say` of an answer written as the schema's JSON, else its first line.
fn said(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(say) = spoken(trimmed) {
        return first_line(&say).to_owned();
    }
    first_line(trimmed).to_owned()
}

/// The `say` of `{"say":"…"}`, also inside a ```json fence.
fn spoken(text: &str) -> Option<String> {
    let fenced = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .and_then(|rest| rest.trim_end().strip_suffix("```"))
        .map(str::trim);
    let value: Value = serde_json::from_str(fenced.unwrap_or(text)).ok()?;
    let say = value.get("say")?.as_str()?.trim();
    (!say.is_empty()).then(|| say.to_owned())
}

/// The first line with text on it, trimmed.
fn first_line(text: &str) -> &str {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
}

/// The folder the AI works in: the payload's, else the hook's own (the AI starts it there).
fn cwd_or_here(cwd: &str) -> String {
    if cwd.is_empty() {
        std::env::current_dir()
            .map(|dir| dir.to_string_lossy().into_owned())
            .unwrap_or_default()
    } else {
        cwd.to_owned()
    }
}

fn event(provider: &str, kind: &str, cwd: &str, message: &str, session: &str) -> Option<Event> {
    let cwd = cwd_or_here(cwd);
    Event {
        provider: provider.into(),
        kind: kind.into(),
        project: folder_name(&cwd).to_owned(),
        cwd,
        message: message.into(),
        session: session.into(),
    }
    .checked()
    .ok()
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn the_hook_knows_the_processes_it_runs_under() {
        let pids = super::ancestors();
        assert!(!pids.is_empty() && !pids.contains(&std::process::id()), "{pids:?}");
    }

    use serde_json::json;

    use super::*;

    fn from_claude(payload: &Value) -> Option<Event> {
        from_named("claude", payload)
    }

    #[test]
    fn shows_a_tools_input_readably() {
        let input = json!({"owner": "zickrian", "repo": "isle", "draft": false});
        assert_eq!(
            readable(&input),
            "owner: zickrian, repo: isle, draft: false"
        );
        assert_eq!(readable(&Value::Null), "");
    }

    #[test]
    fn sees_an_answer_given_in_the_terminal() {
        let path =
            std::env::temp_dir().join(format!("isle-transcript-{}.jsonl", std::process::id()));
        let input = json!({"command": "npm test", "description": "Run the tests"});
        let call = json!({"type": "assistant", "message": {"content": [
            {"type": "tool_use", "id": "toolu_older", "name": "Bash", "input": {"command": "ls"}},
            {"type": "tool_use", "id": "toolu_1", "name": "Bash", "input": input}]}});
        std::fs::write(
            &path,
            format!("{{\"type\":\"user\",\"note\":\"é\"}}\n{call}\n{{\"partial"),
        )
        .unwrap();
        let (id, mut read) = find_call(&path, &json!("Bash"), &input).unwrap();
        assert_eq!(id, "toolu_1");
        // Another call's result is not this one's; this one's is.
        let other = json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "toolu_older"}]}});
        let mine = json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "toolu_1"}]}});
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        std::io::Write::write_all(&mut file, format!("\"}}\n{other}\n").as_bytes()).unwrap();
        assert!(!has_result(&read_on(&path, &mut read).unwrap(), &id));
        std::io::Write::write_all(&mut file, format!("{mine}\n").as_bytes()).unwrap();
        assert!(has_result(&read_on(&path, &mut read).unwrap(), &id));
        assert!(read_on(&path, &mut read).is_none());
        assert!(find_call(&path, &json!("Bash"), &json!({"command": "rm -rf /"})).is_none());
        // The same command again: until its own call is written, the old one (answered) is not taken.
        assert!(pending_call(&path, &json!("Bash"), &input).is_none());
        let again = json!({"type": "assistant", "message": {"content": [
            {"type": "tool_use", "id": "toolu_2", "name": "Bash", "input": input}]}});
        std::io::Write::write_all(&mut file, format!("{again}\n").as_bytes()).unwrap();
        assert_eq!(
            pending_call(&path, &json!("Bash"), &input).unwrap().0,
            "toolu_2"
        );

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn claude_permission_and_waiting() {
        let permission = from_claude(&json!({
            "session_id": "8a7c",
            "transcript_path": "/home/a/.claude/projects/x.jsonl",
            "cwd": "/home/a/code/isle",
            "hook_event_name": "Notification",
            "message": "Claude needs your permission to use Bash"
        }));
        assert_eq!(permission.unwrap().kind, "permission");
        let waiting = from_claude(&json!({
            "cwd": "/home/a/code/isle",
            "hook_event_name": "Notification",
            "message": "Claude is waiting for your input"
        }))
        .unwrap();
        assert_eq!(waiting.kind, "waiting");
        let typed = from_claude(&json!({
            "cwd": "/w",
            "hook_event_name": "Notification",
            "notification_type": "permission_prompt",
            "message": "Claude needs your approval"
        }));
        assert_eq!(typed.unwrap().kind, "permission");
        assert!(
            from_claude(
                &json!({"hook_event_name": "Notification", "notification_type": "auth_success"})
            )
            .is_none()
        );
    }

    #[test]
    fn claude_stopping_is_done() {
        let done = from_claude(&json!({
            "session_id": "8a7c",
            "cwd": "/home/a/code/isle",
            "hook_event_name": "Stop",
            "stop_hook_active": false
        }))
        .unwrap();
        assert_eq!((done.kind.as_str(), done.message.as_str()), ("done", ""));
        let summed_up = from_claude(&json!({
            "cwd": "C:\\Users\\a\\isle",
            "hook_event_name": "Stop",
            "last_assistant_message": "\nFixed the build.\n\nDetails follow…"
        }))
        .unwrap();
        assert_eq!(summed_up.message, "Fixed the build.");
        assert_eq!(summed_up.project, "isle");
        assert!(from_claude(&json!({"hook_event_name": "PreToolUse"})).is_none());
        assert!(from_claude(&json!([1, 2])).is_none());
    }

    #[test]
    fn a_permission_is_asked_and_answered_as_each_ai_wants() {
        let claude = json!({
            "hook_event_name": "PermissionRequest", "session_id": "s1", "cwd": "/home/a/korus",
            "tool_name": "Bash", "tool_input": {"command": "git push origin main", "description": "Push"},
            "permission_suggestions": [{"type": "addRules", "rules": [{"toolName": "Bash", "ruleContent": "git push:*"}],
                "behavior": "allow", "destination": "localSettings"}]
        });
        // What the island could not show whole asks in the terminal.
        let mut hidden = claude.clone();
        hidden["tool_input"]["command"] = json!("echo ok\nrm -rf ~");
        assert!(request("claude", &hidden).is_none());
        hidden["tool_input"]["command"] = json!("x".repeat(MAX_SHOWN + 1));
        assert!(request("claude", &hidden).is_none());
        let asked = request("claude", &claude).unwrap();
        assert_eq!(
            (asked.tool.as_str(), asked.input.as_str(), asked.always),
            ("Bash", "git push origin main", true)
        );
        let allow: Value =
            serde_json::from_str(&decision("claude", "allow", &claude).unwrap()).unwrap();
        assert_eq!(
            allow["hookSpecificOutput"]["hookEventName"],
            "PermissionRequest"
        );
        assert_eq!(
            allow["hookSpecificOutput"]["decision"],
            json!({"behavior": "allow"})
        );
        let always: Value =
            serde_json::from_str(&decision("claude", "always", &claude).unwrap()).unwrap();
        assert_eq!(
            always["hookSpecificOutput"]["decision"]["updatedPermissions"],
            claude["permission_suggestions"]
        );
        // A suggested change of mode is never kept by Always, nor offers it alone.
        let mut moded = claude.clone();
        moded["permission_suggestions"]
            .as_array_mut()
            .unwrap()
            .push(
                json!({"type": "setMode", "mode": "bypassPermissions", "destination": "session"}),
            );
        let always: Value =
            serde_json::from_str(&decision("claude", "always", &moded).unwrap()).unwrap();
        assert_eq!(
            always["hookSpecificOutput"]["decision"]["updatedPermissions"],
            claude["permission_suggestions"]
        );
        moded["permission_suggestions"] = json!([{"type": "setMode", "mode": "acceptEdits"}]);
        assert!(!request("claude", &moded).unwrap().always);
        let deny: Value =
            serde_json::from_str(&decision("codex", "deny", &claude).unwrap()).unwrap();
        assert_eq!(deny["hookSpecificOutput"]["decision"]["behavior"], "deny");
        // No answer: nothing printed, the AI asks in its terminal.
        assert!(decision("claude", "", &claude).is_none());
        let edit = json!({"hook_event_name": "PermissionRequest", "tool_name": "apply_patch",
            "tool_input": {"command": "*** Begin Patch"}, "cwd": "/w"});
        let asked = request("codex", &edit).unwrap();
        assert!(!asked.always && asked.input == "*** Begin Patch");
        assert!(request("agy", &edit).is_none());
        // Always only where the AI keeps the rule.
        assert!(!request("codex", &claude).unwrap().always);
        // The command stays as it runs, its spaces too.
        let mut spaced = claude.clone();
        spaced["tool_input"]["command"] = json!("grep \"a  b\" f");
        assert_eq!(request("claude", &spaced).unwrap().input, "grep \"a  b\" f");
    }

    #[test]
    fn codex_stop_hook_is_done() {
        let done = from_codex_hook(
            &json!({"hook_event_name": "Stop", "session_id": "t1", "cwd": "/home/a/site",
            "last_assistant_message": "All tests pass.\nMore."}),
        )
        .unwrap();
        assert_eq!(
            (
                done.kind.as_str(),
                done.message.as_str(),
                done.project.as_str()
            ),
            ("done", "All tests pass.", "site")
        );
    }
}
