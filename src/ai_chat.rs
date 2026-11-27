// SPDX-License-Identifier: GPL-3.0-or-later
// Ported from zickrian/isle @ a29a7c4 assistant/chat.rs and gnome-extension/src/chat.js.
// Copyright (C) 2026 zickrian. Windows input reuses Avisos Reply/WM_CHAR/keyboard capture.
use super::*;
use isle_ai::assistant::{Command, Update};
impl IsleApp {
    pub(super) fn close_ai_input(&mut self) {
        if self.chat_window.take().is_some() {
            self.overlay.release_keyboard();
        }
    }
    pub(super) fn sync_ai_input(&mut self) {
        if self.chat_window.is_some_and(|w| {
            !self.state.chat.open
                || self
                    .islands
                    .get(w)
                    .is_none_or(|i| i.mode != PillMode::Expanded(TabPage::AiHub))
        }) {
            self.close_ai_input();
        }
        // Return notification ownership as soon as its tab is left too.
        if self.state.reply.is_some()
            && self
                .islands
                .get(self.reply_window)
                .is_none_or(|i| i.mode != PillMode::Expanded(TabPage::Notifications))
        {
            self.close_reply();
        }
        for island in &mut self.islands {
            if island.mode == PillMode::Expanded(TabPage::AiHub) {
                island.morph(isle_render::layout::expanded_size(TabPage::AiHub));
            }
        }
    }
    fn focus_ai(&mut self, island: usize) {
        if self.state.chat.busy {
            return;
        }
        self.close_ai_input();
        self.close_reply();
        self.stop_typing_reminder();
        self.chat_window = Some(island);
        if !cfg!(test) {
            if let Err(err) = self.overlay.take_keyboard(island) {
                self.chat_window = None;
                self.state.chat.error =
                    Some(format!("Clique na caixa de texto para digitar: {err}"));
            }
        }
    }
    fn stop_ai(&mut self) {
        if let Some(tx) = &self.chat_tx {
            let _ = tx.send(Command::Cancel);
        }
        if self.state.chat.busy {
            self.state.chat.busy = false;
            self.state.chat.error = Some("Resposta interrompida.".into());
            self.state.chat.generation = self.state.chat.generation.wrapping_add(1);
        }
    }
    fn send_ai(&mut self) {
        let Some(tx) = &self.chat_tx else {
            self.state.chat.error = Some("Serviço de chat indisponível.".into());
            return;
        };
        if let Some((generation, items)) = self.state.chat.begin() {
            if tx
                .send(Command::Ask {
                    generation,
                    items,
                    provider: self.state.chat.provider,
                })
                .is_err()
            {
                self.state.chat.update(
                    generation,
                    "",
                    true,
                    Some("Serviço de chat encerrado.".into()),
                );
            }
        }
    }
    pub(super) fn apply_ai_update(&mut self, update: Update) {
        self.state
            .chat
            .update(update.generation, &update.text, update.done, update.error);
    }
    pub(super) fn ai_scroll(&mut self, delta: i32) {
        // Clamp using the same line calculation as drawing, so long upward scrolling can always return.
        let lines =
            isle_render::chat::lines(&isle_render::text::Text::new(), &self.state.chat, 460.0)
                .len();
        self.state.chat.scroll = self
            .state
            .chat
            .scroll
            .saturating_add_signed(delta as isize)
            .min(lines.saturating_sub(10));
    }
    pub(super) fn ai_action(&mut self, island: usize, action: UiAction) {
        match action {
            UiAction::AiOpen => {
                // The ✦ (isle's header sparkle): the assistant over the page it was pressed on.
                if let PillMode::Expanded(tab) = self.island(island).mode {
                    if !(tab == TabPage::AiHub && self.state.chat.open) { self.ai_return = Some(tab); }
                }
                self.state.chat.open = true;
                self.island(island).toggle(Some(TabPage::AiHub));
                self.focus_ai(island);
            }
            UiAction::AiInput => self.focus_ai(island),
            UiAction::AiBack => {
                self.close_ai_input();
                self.state.chat.open = false;
                if let Some(tab) = self.ai_return.take().filter(|t| isle_render::layout::visible_tabs(&self.config.modules).contains(t)) {
                    self.island(island).toggle(Some(tab));
                }
            }
            UiAction::AiIdea(i) => {
                // Port of the ideas' click (isle voice.js): the words in the field, asked at once or left to finish.
                let Some((_, words, send)) = isle_render::chat::IDEAS.get(i) else { return };
                self.state.chat.input.text = (*words).into();
                self.focus_ai(island);
                if *send { self.send_ai(); }
            }
            UiAction::AiSend => self.send_ai(),
            UiAction::AiStop => self.stop_ai(),
            UiAction::AiNew => {
                self.stop_ai();
                self.state.chat.reset(self.state.chat.provider);
                self.focus_ai(island);
            }
            UiAction::AiProvider(p) if p != self.state.chat.provider => {
                self.stop_ai();
                self.state.chat.reset(p);
                self.focus_ai(island);
            }
            UiAction::AiScroll(delta) => self.ai_scroll(delta),
            _ => {}
        }
    }
    pub(super) fn ai_char(&mut self, window: usize, ch: char) {
        if self.chat_window != Some(window) {
            return;
        }
        self.sync_ai_input();
        if self.chat_window.is_none() {
            return;
        }
        match ch {
            '\u{1b}' => self.close_ai_input(),
            _ if self.state.chat.busy => {}
            '\r' => self.send_ai(),
            '\u{16}' => self.state.chat.input.paste(&if cfg!(test) {
                String::new()
            } else {
                notifications::clipboard_text()
            }),
            c => self.state.chat.input.type_char(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ai_opens_types_and_yields_keyboard_across_tabs() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.apply_ui_action(UiAction::SwitchTab(TabPage::AiHub));
        app.apply_ui_action(UiAction::AiOpen);
        assert!(app.state.chat.open);
        assert_eq!(app.islands[0].geom.height.target, isle_render::layout::PANEL.1);
        for ch in "ação 😀!\u{8}".chars() {
            app.apply_input(UiInput::Char { window: 0, ch }).unwrap();
        }
        assert_eq!(app.state.chat.input.text, "ação 😀");
        app.apply_ui_action(UiAction::SwitchTab(TabPage::Media));
        app.apply_input(UiInput::Char { window: 0, ch: 'x' })
            .unwrap();
        assert_eq!(app.state.chat.input.text, "ação 😀");
        app.apply_ui_action(UiAction::SwitchTab(TabPage::AiHub));
        app.apply_ui_action(UiAction::AiBack);
        assert_eq!(app.islands[0].geom.height.target, isle_render::layout::PANEL.1);
    }
    #[test]
    fn missing_worker_is_visible_and_escape_preserves_draft() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.apply_ui_action(UiAction::AiOpen);
        app.apply_input(UiInput::Char { window: 0, ch: 'a' })
            .unwrap();
        app.apply_input(UiInput::Char {
            window: 0,
            ch: '\u{1b}',
        })
        .unwrap();
        app.apply_input(UiInput::Char { window: 0, ch: 'b' })
            .unwrap();
        assert_eq!(app.state.chat.input.text, "a");
        app.apply_ui_action(UiAction::AiSend);
        assert!(app.state.chat.error.is_some() && !app.state.chat.busy);
    }
    #[test]
    fn send_stop_provider_switch_and_stale_events() {
        let mut app = IsleApp::new(IsleConfig::default());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        app.chat_tx = Some(tx);
        app.apply_ui_action(UiAction::AiOpen);
        app.state.chat.input.text = "question".into();
        app.apply_input(UiInput::Char {
            window: 0,
            ch: '\r',
        })
        .unwrap();
        let Command::Ask {
            generation, items, ..
        } = rx.try_recv().unwrap()
        else {
            panic!("expected request")
        };
        assert_eq!(items[0].text, "question");
        assert!(app.state.chat.busy);
        app.apply_ui_action(UiAction::AiSend);
        assert!(rx.try_recv().is_err());
        app.apply_ai_update(Update {
            generation,
            text: "partial".into(),
            done: false,
            error: None,
        });
        app.apply_ui_action(UiAction::AiStop);
        assert!(matches!(rx.try_recv().unwrap(), Command::Cancel));
        app.apply_ai_update(Update {
            generation,
            text: "late final".into(),
            done: true,
            error: None,
        });
        assert_eq!(app.state.chat.items.last().unwrap().text, "partial");
        assert!(!app.state.chat.busy);
        app.apply_ui_action(UiAction::AiProvider(
            isle_core::state::AiProviderKind::Codex,
        ));
        assert!(app.state.chat.items.is_empty());
        app.apply_ai_update(Update {
            generation,
            text: "old backend".into(),
            done: true,
            error: None,
        });
        assert!(app.state.chat.items.is_empty());
    }
    #[test]
    fn only_the_input_owner_is_kept_open_and_keyboard_loss_releases_it() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.islands.push(Presence::new());
        app.apply_ui_action_on(1, UiAction::AiOpen);
        app.apply_input(UiInput::Char { window: 0, ch: 'x' })
            .unwrap();
        assert!(app.state.chat.input.text.is_empty());
        app.apply_input(UiInput::Char { window: 1, ch: 'a' })
            .unwrap();
        let now = Instant::now();
        app.on_deadlines(now);
        app.on_deadlines(now + Duration::from_secs(5));
        assert_eq!(app.islands[1].mode, PillMode::Expanded(TabPage::AiHub));
        app.apply_input(UiInput::KeyboardLost).unwrap();
        assert!(app.chat_window.is_none());
        app.apply_input(UiInput::Char { window: 1, ch: 'b' })
            .unwrap();
        assert_eq!(app.state.chat.input.text, "a");
    }
}
