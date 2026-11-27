// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted from zickrian/isle @ a29a7c4 gnome-extension/src/chat.js (Copyright 2026 zickrian).
// tiny-skia layout for text chat inside the Windows pill; shared draw/hitbox geometry.
use crate::{
    pages::{HitBox, UiAction},
    text::{Text, Weight},
};
use isle_core::{chat::Chat, state::AiProviderKind};

pub const TEXT_TOP: f32 = crate::layout::BODY + 50.0;
/// Port of IDEAS (isle voice.js): what to try under an empty field: the chip, the words it puts in, and
/// whether it asks at once.
pub const IDEAS: [(&str, &str, bool); 3] = [
    ("Plano de hoje", "O que tenho na agenda hoje?", true),
    ("Instalar um app…", "Instale ", false),
    ("Encontrar um arquivo…", "Encontre ", false),
];
/// The ideas show while nothing is said or typed yet (isle's `_syncIdeas`).
pub fn ideas_shown(chat: &Chat) -> bool {
    chat.items.is_empty() && !chat.busy && chat.error.is_none() && chat.input.text.is_empty()
}
pub const LINE_HEIGHT: f32 = 18.0;
/// The most lines of a command the card shows; longer, it is not offered to run (never run unseen).
pub const APPROVAL_MAX_LINES: usize = 8;
pub const APPROVAL_LINE: f32 = 17.0;
/// The card of a command waiting for Executar or Negar (isle's), over the field: its top and height
/// from the panel's top, and the command's lines, whole (None: too long to show, so not runnable).
pub struct ApprovalCard {
    pub top: f32,
    pub height: f32,
    pub lines: Option<Vec<String>>,
}
pub fn approval_card(w: f32, h: f32, chat: &Chat) -> Option<ApprovalCard> {
    let a = chat.approval.as_ref()?;
    let wrapped: Vec<String> = crate::pages::BAR_TEXT.with(|t| {
        format!("$ {}", a.command).split('\n').flat_map(|l| {
            let l = t.wrap_exact(l.trim_end_matches('\r'), w - 88., 12., Weight::Regular);
            if l.is_empty() { vec![String::new()] } else { l }
        }).collect()
    });
    let lines = (wrapped.len() <= APPROVAL_MAX_LINES && isle_core::chat::showable(&a.command)).then_some(wrapped);
    let shown = lines.as_ref().map_or(2, Vec::len);
    let height = 84. + shown as f32 * APPROVAL_LINE;
    Some(ApprovalCard { top: h - 66. - height, height, lines })
}
pub fn hitboxes(left: f32, top: f32, w: f32, h: f32, chat: &Chat) -> Vec<HitBox> {
    let hb = |x, y, w, h, action| HitBox {
        x: left + x,
        y: top + y,
        w,
        h,
        action,
    };
    if !chat.open {
        return vec![];
    }
    let mut boxes = vec![
        hb(20., crate::layout::BODY, 30., 30., UiAction::AiBack),
        hb(w - 50., crate::layout::BODY, 30., 30., UiAction::AiNew),
    ];
    let widths = [(w-134.)/3.;3];
    let mut x = 64.;
    for (i, p) in [
        AiProviderKind::ClaudeCode,
        AiProviderKind::Codex,
        AiProviderKind::OpenCodeGo,
    ]
    .into_iter()
    .enumerate()
    {
        boxes.push(hb(x, crate::layout::BODY, widths[i], 30., UiAction::AiProvider(p)));
        x += widths[i] + 5.;
    }
    if let Some(card) = approval_card(w, h, chat) {
        let y = card.top + card.height - 38.;
        if card.lines.is_some() {
            boxes.push(hb(w - 20. - 104., y, 92., 26., UiAction::AiRun));
        }
        boxes.push(hb(w - 20. - 104. - 92., y, 84., 26., UiAction::AiDeny));
    } else if ideas_shown(chat) {
        // .isle-voice-idea: 12 px semibold, 4 px by 11, 6 px apart.
        let mut x = 20.;
        for (i, (name, _, _)) in IDEAS.iter().enumerate() {
            let iw = crate::pages::BAR_TEXT.with(|t| t.width(name, 12., Weight::Semibold)) + 22.;
            boxes.push(hb(x, h - 90., iw, 24., UiAction::AiIdea(i)));
            x += iw + 6.;
        }
    } else {
        boxes.push(hb(20., h - 92., 30., 26., UiAction::AiScroll(8)));
        boxes.push(hb(58., h - 92., 30., 26., UiAction::AiScroll(-8)));
    }
    if !chat.busy {
        boxes.push(hb(20., h - 55., w - 86., 38., UiAction::AiInput));
    }
    boxes.push(hb(
        w - 56.,
        h - 55.,
        36.,
        38.,
        if chat.busy {
            UiAction::AiStop
        } else {
            UiAction::AiSend
        },
    ));
    boxes
}
pub fn lines(text: &Text, chat: &Chat, width: f32) -> Vec<(bool, String)> {
    let mut rows = Vec::new();
    for item in &chat.items {
        rows.push((
            item.user,
            if item.user {
                "Você"
            } else {
                isle_core::chat::provider_name(chat.provider)
            }
            .into(),
        ));
        for paragraph in item.text.split('\n') {
            let wrapped = text.wrap_exact(paragraph, width, 12.0, Weight::Regular);
            if wrapped.is_empty() {
                rows.push((item.user, String::new()));
            } else {
                rows.extend(wrapped.into_iter().map(|s| (item.user, s)));
            }
        }
        rows.push((item.user, String::new()));
    }
    if let Some(error) = &chat.error {
        rows.push((false, "Aviso".into()));
        for paragraph in error.split('\n') {
            rows.extend(
                text.wrap_exact(paragraph, width, 12.0, Weight::Regular)
                    .into_iter()
                    .map(|s| (false, s)),
            );
        }
    }
    rows
}
pub fn visible_range(total: usize, capacity: usize, scroll: usize) -> std::ops::Range<usize> {
    let end = total.saturating_sub(scroll.min(total.saturating_sub(capacity)));
    end.saturating_sub(capacity)..end
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_command_runs_only_when_the_card_shows_it_whole() {
        let mut chat = Chat { open: true, busy: true, ..Chat::default() };
        let (w, h) = (620., 432.);
        let run = |c: &Chat| hitboxes(0., 0., w, h, c).iter().any(|b| b.action == UiAction::AiRun);
        assert!(!run(&chat), "nothing asked");
        chat.approval = Some(isle_core::chat::Approval { title: "Ver o Node".into(), command: "node -v".into() });
        assert!(run(&chat));
        assert!(hitboxes(0., 0., w, h, &chat).iter().any(|b| b.action == UiAction::AiDeny));
        let long = "Get-ChildItem C:/ -Recurse | Where-Object { $_.Length -gt 1MB } ".repeat(40);
        chat.approval = Some(isle_core::chat::Approval { title: "Algo longo".into(), command: long });
        assert!(!run(&chat), "too long to see whole: only Negar");
        assert!(hitboxes(0., 0., w, h, &chat).iter().any(|b| b.action == UiAction::AiDeny));
        let card = approval_card(w, h, &chat).unwrap();
        assert!(card.top > crate::layout::BODY, "the card stays under the header");
    }
    #[test]
    fn chat_controls_fit_and_have_no_overlapping_targets() {
        let mut chat = Chat::default();
        assert!(hitboxes(30.0, 6.0, 520.0, 260.0, &chat).is_empty(), "closed, the ✦ in the header opens it");
        chat.open = true;
        let boxes = hitboxes(30.0, 6.0, 520.0, 360.0, &chat);
        assert_eq!(boxes.iter().filter(|b| matches!(b.action, UiAction::AiIdea(_))).count(), 3, "ideas under an empty field");
        chat.input.text = "oi".into();
        assert!(!hitboxes(30.0, 6.0, 520.0, 360.0, &chat).iter().any(|b| matches!(b.action, UiAction::AiIdea(_))));
        assert_eq!(
            boxes
                .iter()
                .filter(|b| matches!(b.action, UiAction::AiProvider(_)))
                .count(),
            3
        );
        assert!(boxes.iter().any(|b| b.action == UiAction::AiInput));
        for (i, b) in boxes.iter().enumerate() {
            assert!(b.x >= 48.0 && b.x + b.w <= 532.1 && b.y >= 44.0 && b.y + b.h <= 356.1);
            for other in boxes.iter().skip(i + 1) {
                assert!(
                    !(b.x < other.x + other.w
                        && other.x < b.x + b.w
                        && b.y < other.y + other.h
                        && other.y < b.y + b.h)
                );
            }
        }
        chat.busy = true;
        let boxes = hitboxes(30.0, 6.0, 520.0, 360.0, &chat);
        assert!(boxes.iter().any(|b| b.action == UiAction::AiStop));
        assert!(
            !boxes
                .iter()
                .any(|b| matches!(b.action, UiAction::AiSend | UiAction::AiInput))
        );
    }
    #[test]
    fn transcript_keeps_newlines_unicode_and_whole_long_answers() {
        let mut c = Chat::default();
        c.items.push(isle_core::chat::Message {
            user: false,
            text: "ação\n\nnext 😀".into(),
        });
        let rows = lines(&Text::new(), &c, 440.0);
        assert_eq!(
            rows.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>(),
            ["Claude", "ação", "", "next 😀", ""]
        );
    }
    #[test]
    fn scrolling_reaches_both_ends_and_errors_stay_readable() {
        assert_eq!(visible_range(100, 10, 0), 90..100);
        assert_eq!(visible_range(100, 10, 9999), 0..10);
        assert_eq!(visible_range(3, 10, 99), 0..3);
        let mut c = Chat::default();
        c.error = Some("Faça login no terminal".into());
        assert_eq!(
            lines(&Text::new(), &c, 440.0).last().unwrap().1,
            "Faça login no terminal"
        );
    }
}
