// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted from zickrian/isle @ a29a7c4, assistant/chat.rs (Copyright 2026 zickrian).
// Windows: bounded in-memory text sessions, generation IDs and Reply editing; no voice/actions.
use crate::{notifications::Reply, state::AiProviderKind};
use serde::{Deserialize, Serialize};

pub const HISTORY: usize = 50;
pub const ANSWER_LIMIT: usize = 32_768;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub user: bool,
    pub text: String,
}

/// A command the assistant asks to run on this computer: shown with Executar and Negar (isle's Run/Deny).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Approval {
    /// What it does, in the user's words ("Instalar o Node pelo nvm").
    pub title: String,
    /// The PowerShell command, shown whole.
    pub command: String,
}

/// A model a provider's CLI offers: the id it takes and the name people see.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelChoice {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chat {
    pub open: bool,
    pub provider: AiProviderKind,
    pub input: Reply,
    pub items: Vec<Message>,
    pub busy: bool,
    pub generation: u64,
    pub error: Option<String>,
    /// Visual lines before the bottom, zero follows streaming text.
    pub scroll: usize,
    /// The input's caret while it has the keyboard: whether this frame draws it. None when unfocused.
    #[serde(skip)]
    pub caret: Option<bool>,
    /// A command waiting for Executar or Negar.
    pub approval: Option<Approval>,
    /// The model the open provider answers with ("Opus"); empty: the CLI's own default.
    pub model_label: String,
}
impl Default for Chat {
    fn default() -> Self {
        Self {
            open: false,
            provider: AiProviderKind::ClaudeCode,
            input: Reply::default(),
            items: vec![],
            busy: false,
            generation: 0,
            error: None,
            scroll: 0,
            caret: None,
            approval: None,
            model_label: String::new(),
        }
    }
}
impl Chat {
    pub fn begin(&mut self) -> Option<(u64, Vec<Message>)> {
        if self.busy || self.input.text.trim().is_empty() {
            return None;
        }
        self.generation = self.generation.wrapping_add(1);
        self.items.push(Message {
            user: true,
            text: std::mem::take(&mut self.input.text).trim().to_owned(),
        });
        // Evict complete turns, keeping the same 50 shown messages as the original.
        if self.items.len() >= HISTORY {
            self.items.drain(..2);
        }
        let request = self.items.clone();
        self.items.push(Message {
            user: false,
            text: String::new(),
        });
        self.busy = true;
        self.error = None;
        self.scroll = 0;
        Some((self.generation, request))
    }
    pub fn update(&mut self, generation: u64, text: &str, done: bool, error: Option<String>) {
        if generation != self.generation || !self.busy {
            return;
        }
        if let Some(item) = self.items.last_mut() {
            let mut end = text.len().min(ANSWER_LIMIT);
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            item.text = text[..end].to_owned();
        }
        self.error = error;
        if done {
            self.busy = false;
            self.approval = None;
        }
    }
    pub fn reset(&mut self, provider: AiProviderKind) {
        *self = Self {
            open: self.open,
            generation: self.generation.wrapping_add(1),
            provider,
            model_label: std::mem::take(&mut self.model_label),
            ..Self::default()
        };
    }
}

pub fn provider_name(provider: AiProviderKind) -> &'static str {
    match provider {
        AiProviderKind::ClaudeCode => "Claude",
        AiProviderKind::Codex => "Codex",
        AiProviderKind::OpenCodeGo => "OpenCode Go",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blank_busy_and_history_are_bounded() {
        let mut c = Chat::default();
        assert!(c.begin().is_none());
        for i in 0..30 {
            c.input.text = format!("question {i}");
            let (id, request) = c.begin().unwrap();
            assert!(request.last().unwrap().user);
            assert!(c.begin().is_none());
            c.update(id, "answer", true, None);
        }
        assert_eq!(c.items.len(), HISTORY);
        assert_eq!(c.items[0].text, "question 5");
    }
    #[test]
    fn snapshots_unicode_limits_and_late_events() {
        let mut c = Chat::default();
        c.input.paste("oi\r\n😀");
        c.input.type_char('\u{8}');
        assert_eq!(c.input.text, "oi ");
        let (id, _) = c.begin().unwrap();
        c.update(id, "á😀", false, None);
        c.update(id, "á😀 sim", false, None);
        assert_eq!(c.items.last().unwrap().text, "á😀 sim");
        c.update(id, &"😀".repeat(ANSWER_LIMIT), true, None);
        assert!(c.items.last().unwrap().text.len() <= ANSWER_LIMIT);
        c.reset(AiProviderKind::Codex);
        c.update(id, "late", true, Some("late error".into()));
        assert!(c.items.is_empty() && c.error.is_none() && !c.busy);
    }
}
