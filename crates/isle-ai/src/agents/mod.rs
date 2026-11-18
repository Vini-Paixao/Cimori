// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 zickrian
// Adapted for isle-win from zickrian/isle @ a29a7c4 (crates/isle-daemon/src/agents/mod.rs):
// D-Bus replaced by HTTP, no process ids (the island may run on another machine), Windows paths accepted.

//! AI agent alerts: Claude Code and Codex run `Cimori --island-hook` when they need the user or finish,
//! and the island shows it. The hook may run on this machine or another one (over Tailscale/LAN).

pub mod claude;
pub mod codex;
pub mod file;
pub mod hook;
mod json_hooks;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Longest texts the island gets, in characters.
const MAX_MESSAGE: usize = 160;
const MAX_PROJECT: usize = 60;
const MAX_SESSION: usize = 100;
const MAX_CWD: usize = 1024;
/// Events of one kind from one AI session closer together than this are dropped.
const MIN_GAP: Duration = Duration::from_millis(300);
/// A permission waits this long for an answer on the island; then the AI asks in its own terminal
/// (its hook may wait ten minutes: this ends first).
pub const ANSWER_WITHIN: Duration = Duration::from_secs(9 * 60 + 30);
const MAX_INPUT: usize = 300;
const MAX_TOOL: usize = 60;

const KINDS: [&str; 3] = ["permission", "waiting", "done"];
const PROVIDERS: [&str; 2] = ["claude", "codex"];

/// What an AI hook reports, and the island shows.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Event {
    pub provider: String,
    pub kind: String,
    pub project: String,
    pub cwd: String,
    pub message: String,
    pub session: String,
}

impl Event {
    /// Checked, on one line each and cut to size: the project is the folder's name unless given.
    pub fn checked(mut self) -> Result<Self, String> {
        if !PROVIDERS.contains(&self.provider.as_str()) {
            return Err(format!("unknown provider `{}`", cut(&self.provider, 40)));
        }
        if !KINDS.contains(&self.kind.as_str()) {
            return Err(format!("unknown kind `{}`", cut(&self.kind, 40)));
        }
        let cwd = self.cwd.trim();
        self.cwd = if looks_absolute(cwd)
            && cwd.chars().count() <= MAX_CWD
            && !cwd.contains(char::is_control)
        {
            cwd.to_owned()
        } else {
            String::new()
        };
        if self.project.trim().is_empty() {
            self.project = folder_name(&self.cwd).to_owned();
        }
        self.project = cut(&self.project, MAX_PROJECT);
        self.message = cut(&self.message, MAX_MESSAGE);
        self.session = cut(&self.session, MAX_SESSION);
        Ok(self)
    }
}

/// An absolute path from Linux (`/home/…`) or Windows (`C:\…`), whichever machine the AI runs on.
fn looks_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    path.starts_with('/')
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'\\' | b'/'))
}

/// The last folder of a Linux or Windows path, whatever this machine's separator is.
pub fn folder_name(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or_default()
}

/// `text` on one line, its spaces collapsed, at most `max` characters (an ellipsis ends a cut one).
fn cut(text: &str, max: usize) -> String {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|word| word.chars().filter(|c| !c.is_control()).collect())
        .filter(|word: &String| !word.is_empty())
        .collect();
    clip(&words.join(" "), max)
}

/// `text` as it is, at most `max` characters (an ellipsis ends a cut one).
fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut short: String = text.chars().take(max.saturating_sub(1)).collect();
    short.truncate(short.trim_end().len());
    short.push('…');
    short
}

/// Whether `text` fits in `max` characters with nothing hidden: no line breaks, no control or
/// direction-changing characters that would make what is shown differ from what runs.
/// From zickrian/isle crates/isle-daemon/src/assistant/tasks.rs.
pub fn shows_whole(text: &str, max: usize) -> bool {
    let hidden = |c: char| {
        c.is_control()
            || matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2069}'
                | '\u{061C}' | '\u{FEFF}' | '\u{00AD}')
    };
    text.chars().count() <= max && !text.chars().any(hidden)
}

/// A permission an AI agent asks for, as its hook sends it and the island shows it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Request {
    pub id: String,
    pub provider: String,
    pub project: String,
    pub cwd: String,
    pub session: String,
    /// The tool (Bash, Edit…) and what it would run or touch, on one line.
    pub tool: String,
    pub input: String,
    /// Whether the AI offers to keep it for next time (Claude's "always").
    pub always: bool,
    /// The hook's parent processes, nearest first: the island finds the agent's window among them
    /// (isle's hook `pids`).
    pub pids: Vec<u32>,
}

impl Request {
    /// Checked like an `Event`, with the tool cut to size; the input stays exactly as the AI would run it.
    pub fn checked(mut self) -> Result<Self, String> {
        // A hook sends only what shows whole (see hook.rs `request`): anything else asks in the terminal.
        // Too long is refused too, never clipped: what is approved must be all that was shown.
        if !shows_whole(&self.input, MAX_INPUT) {
            return Err("the input is too long, or has line breaks or hidden characters".into());
        }
        let event = Event {
            provider: self.provider.clone(),
            kind: "permission".into(),
            project: self.project.clone(),
            cwd: self.cwd.clone(),
            message: String::new(),
            session: self.session.clone(),
        }
        .checked()?;
        self.project = event.project;
        self.cwd = event.cwd;
        self.session = event.session;
        self.tool = cut(&self.tool, MAX_TOOL);
        self.pids.truncate(32);
        Ok(self)
    }
}

/// Drops an event that follows the last of its kind from the same AI session too closely.
#[derive(Default)]
pub struct Limiter {
    last: HashMap<(String, String, String), Instant>,
}

impl Limiter {
    /// False for an event too soon after the last of its kind.
    pub fn allow(&mut self, event: &Event, now: Instant) -> bool {
        let key = (
            event.provider.clone(),
            event.kind.clone(),
            event.session.clone(),
        );
        if self
            .last
            .get(&key)
            .is_some_and(|at| now.duration_since(*at) < MIN_GAP)
        {
            return false;
        }
        // Only the recent are kept: each AI session adds its own, which over weeks would pile up.
        self.last.retain(|_, at| now.duration_since(*at) < MIN_GAP);
        self.last.insert(key, now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(json: &str) -> Result<Event, String> {
        serde_json::from_str::<Event>(json).unwrap().checked()
    }

    #[test]
    fn a_command_asked_about_stays_as_it_runs() {
        let request = |input: &str| {
            Request {
                provider: "claude".into(),
                tool: "Bash".into(),
                input: input.into(),
                ..Request::default()
            }
            .checked()
        };
        assert_eq!(request("grep \"a  b\" f").unwrap().input, "grep \"a  b\" f");
        assert!(request("ls\nrm -rf ~").is_err());
        assert!(request("ls \u{202E}fdp.exe").is_err());
        assert!(request(&"x ".repeat(MAX_INPUT)).is_err(), "too long: refused, not clipped");
    }

    #[test]
    fn events_are_checked_and_cut_to_size() {
        let checked = event(
            r#"{"provider": "claude", "kind": "permission", "cwd": "/home/a/isle",
                "message": "  Claude needs\n your permission\tto use Bash  ", "session": "abc"}"#,
        )
        .unwrap();
        assert_eq!(checked.project, "isle");
        assert_eq!(checked.message, "Claude needs your permission to use Bash");
        assert_eq!(checked.session, "abc");

        let long = event(&format!(
            r#"{{"provider": "codex", "kind": "done", "cwd": "relative/path",
                 "project": "{}", "message": "{}\u0007"}}"#,
            "p".repeat(100),
            "word ".repeat(100)
        ))
        .unwrap();
        assert_eq!(long.cwd, "");
        assert_eq!(long.project.chars().count(), MAX_PROJECT);
        assert!(long.project.ends_with('…'));
        assert!(long.message.chars().count() <= MAX_MESSAGE);
        assert!(long.message.ends_with("word…"));

        assert!(event(r#"{"provider": "agy", "kind": "done"}"#).is_err());
        assert!(event(r#"{"provider": "claude", "kind": "exploded"}"#).is_err());
        assert!(event(r#"{"provider": "claude", "kind": "done"}"#).is_ok());
    }

    #[test]
    fn windows_and_linux_folders_both_name_the_project() {
        let win = event(r#"{"provider": "claude", "kind": "done", "cwd": "C:\\Users\\marcus\\isle-win"}"#)
            .unwrap();
        assert_eq!((win.cwd.as_str(), win.project.as_str()), ("C:\\Users\\marcus\\isle-win", "isle-win"));
        let linux = event(r#"{"provider": "claude", "kind": "done", "cwd": "/mnt/dados/projetos/isle-win/"}"#)
            .unwrap();
        assert_eq!(linux.project, "isle-win");
    }

    #[test]
    fn events_too_close_together_are_dropped() {
        let mut limiter = Limiter::default();
        let done = event(r#"{"provider": "claude", "kind": "done"}"#).unwrap();
        let waiting = event(r#"{"provider": "claude", "kind": "waiting"}"#).unwrap();
        let codex = event(r#"{"provider": "codex", "kind": "done"}"#).unwrap();
        let start = Instant::now();
        assert!(limiter.allow(&done, start));
        assert!(!limiter.allow(&done, start + Duration::from_millis(100)));
        assert!(limiter.allow(&waiting, start + Duration::from_millis(100)));
        assert!(limiter.allow(&codex, start + Duration::from_millis(100)));
        assert!(limiter.allow(&done, start + Duration::from_millis(400)));
        // Sessions long over are forgotten.
        assert_eq!(limiter.last.len(), 1);
    }

    #[test]
    fn an_input_too_long_to_show_is_refused_not_clipped() {
        let request = |input: String| {
            Request { provider: "claude".into(), tool: "Bash".into(), input, ..Request::default() }.checked()
        };
        assert!(request("x ".repeat(MAX_INPUT)).is_err());
        assert_eq!(request("ls".into()).unwrap().input, "ls");
    }
}
