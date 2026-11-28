// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted from zickrian/isle @ a29a7c4 assistant/{claude,codex,opencode}.rs.
// Copyright (C) 2026 zickrian. Text-only stream adapters, without the voice/action schema.
use isle_core::i18n::{tr};
use anyhow::{Result, bail};
use isle_core::state::AiProviderKind;
use serde_json::{Value, json};

#[derive(Default)]
pub struct Stream {
    pub text: String,
    pub done: bool,
}
impl Stream {
    pub fn event(&mut self, provider: AiProviderKind, event: &Value) -> Result<bool> {
        let old = self.text.clone();
        match provider {
            AiProviderKind::ClaudeCode => match event["type"].as_str() {
                Some("stream_event") if event["event"]["delta"]["type"] == "text_delta" => self
                    .text
                    .push_str(event["event"]["delta"]["text"].as_str().unwrap_or_default()),
                Some("assistant") => {
                    if let Some(parts) = event["message"]["content"].as_array() {
                        let text: String = parts
                            .iter()
                            .filter(|p| p["type"] == "text")
                            .filter_map(|p| p["text"].as_str())
                            .collect();
                        if !text.is_empty() {
                            self.text = text;
                        }
                    }
                }
                Some("result") => {
                    if event["is_error"] == true || event["subtype"] != "success" {
                        bail!("{}", reason(event));
                    }
                    if let Some(text) = event["result"].as_str().filter(|s| !s.is_empty()) {
                        self.text = text.into();
                    }
                    self.done = true;
                }
                _ => {}
            },
            AiProviderKind::Codex => match event["method"].as_str() {
                Some("item/agentMessage/delta") => self
                    .text
                    .push_str(event["params"]["delta"].as_str().unwrap_or_default()),
                Some("item/completed") if event["params"]["item"]["type"] == "agentMessage" => {
                    if let Some(text) = event["params"]["item"]["text"].as_str() {
                        self.text = text.into();
                    }
                }
                Some("turn/completed") => {
                    let turn = &event["params"]["turn"];
                    if !turn["error"].is_null()
                        || turn["status"] == "failed"
                        || turn["status"] == "interrupted"
                    {
                        bail!("{}", reason(turn));
                    }
                    self.done = true;
                }
                Some("error") if event["params"]["willRetry"] != true => {
                    bail!("{}", reason(&event["params"]))
                }
                _ => {}
            },
            AiProviderKind::OpenCodeGo => match event["type"].as_str() {
                Some("step_start") => self.text.clear(),
                Some("text") => self
                    .text
                    .push_str(event["part"]["text"].as_str().unwrap_or_default()),
                Some("error") => bail!("{}", reason(event)),
                _ => {}
            },
        }
        anyhow::ensure!(
            self.text.len() <= isle_core::chat::ANSWER_LIMIT,
            "{}", tr("assistant.answer_too_long")
        );
        Ok(old != self.text)
    }
}
pub fn reason(event: &Value) -> &str {
    [
        &event["error"]["data"]["message"],
        &event["error"]["message"],
        &event["result"],
        &event["error"],
        &event["message"],
        &event["error"]["name"],
    ]
    .into_iter()
    .find_map(Value::as_str)
    .unwrap_or(tr("assistant.stopped"))
}
pub fn claude_request(prompt: &str) -> Value {
    json!({"type":"user","message":{"role":"user","content":prompt}})
}
pub fn codex_turn(thread: &str, prompt: &str) -> Value {
    json!({"threadId":thread,"input":[{"type":"text","text":prompt}],"effort":"low"})
}

#[cfg(test)]
mod tests {
    use super::*;
    use AiProviderKind::*;
    #[test]
    fn requests_keep_user_text_as_data() {
        let text = "--evil\n\"😀\"";
        assert_eq!(claude_request(text)["message"]["content"], text);
        assert_eq!(codex_turn("t1", text)["input"][0]["text"], text);
    }
    #[test]
    fn claude_deltas_final_fallback_and_error() {
        let mut s = Stream::default();
        s.event(
            ClaudeCode,
            &json!({"type":"stream_event","event":{"delta":{"type":"text_delta","text":"Olá"}}}),
        )
        .unwrap();
        assert_eq!(s.text, "Olá");
        s.event(
            ClaudeCode,
            &json!({"type":"result","subtype":"success","result":"Olá!"}),
        )
        .unwrap();
        assert_eq!(s.text, "Olá!");
        assert!(s.done);
        assert!(
            Stream::default()
                .event(
                    ClaudeCode,
                    &json!({"type":"result","is_error":true,"result":"sign in"})
                )
                .is_err()
        );
    }
    #[test]
    fn codex_delta_completion_and_failure() {
        let mut s = Stream::default();
        s.event(
            Codex,
            &json!({"method":"item/agentMessage/delta","params":{"delta":"Hi"}}),
        )
        .unwrap();
        s.event(Codex, &json!({"method":"item/completed","params":{"item":{"type":"agentMessage","text":"Hi!"}}})).unwrap();
        s.event(
            Codex,
            &json!({"method":"turn/completed","params":{"turn":{"status":"completed"}}}),
        )
        .unwrap();
        assert_eq!(s.text, "Hi!");
        assert!(s.done);
        assert!(Stream::default().event(Codex, &json!({"method":"turn/completed","params":{"turn":{"status":"failed","error":{"message":"quota"}}}})).is_err());
    }
    #[test]
    fn opencode_text_steps_and_nested_error() {
        let mut s = Stream::default();
        s.event(OpenCodeGo, &json!({"type":"text","part":{"text":"one"}}))
            .unwrap();
        s.event(OpenCodeGo, &json!({"type":"step_start"})).unwrap();
        s.event(OpenCodeGo, &json!({"type":"text","part":{"text":"two"}}))
            .unwrap();
        assert_eq!(s.text, "two");
        assert!(
            s.event(
                OpenCodeGo,
                &json!({"type":"error","error":{"data":{"message":"login"}}})
            )
            .is_err()
        );
    }
}
