use super::*;
use isle_core::chat::provider_name;
/// Port of the usage page's `resets` (isle): "renova em 3h 34m".
fn usage_resets(ms: i64, now_ms: i64) -> String {
    let minutes = ((ms - now_ms) as f64 / 60000.).round().max(0.) as i64;
    let (d, h, m) = (minutes / 1440, minutes % 1440 / 60, minutes % 60);
    match (minutes, d, h, m) {
        (0, ..) => "renova logo".into(),
        (_, 1.., 0, _) => format!("renova em {d}d"),
        (_, 1.., ..) => format!("renova em {d}d {h}h"),
        (_, 0, 1.., 0) => format!("renova em {h}h"),
        (_, 0, 1.., _) => format!("renova em {h}h {m}m"),
        _ => format!("renova em {m}m"),
    }
}

/// Port of the usage page's `updated` (isle): "Atualizado 16:54", "Do Codex, 16:54" for numbers from Codex's
/// last use; on another day with its weekday.
fn usage_updated(ms: i64, from_codex: bool, s: &IslandState) -> String {
    if ms <= 0 {
        return String::new();
    }
    let local = ms.div_euclid(1000) + s.utc_offset_s;
    let (day, secs) = (local.div_euclid(86_400), local.rem_euclid(86_400));
    let (h, m) = (secs / 3600, secs % 3600 / 60);
    // The island's own 12/24-hour setting, as its clock shows it.
    let time = if s.clock_text.ends_with('M') { format!("{}:{m:02} {}", (h + 11) % 12 + 1, if h < 12 { "AM" } else { "PM" }) } else { format!("{h:02}:{m:02}") };
    let when = if day == s.today { time } else { format!("{} {time}", isle_core::clock::WEEKDAYS[day.rem_euclid(7) as usize]) };
    if from_codex { format!("Do Codex, {when}") } else { format!("Atualizado {when}") }
}

impl PillRenderer {
    /// Port of UsagePage._content and _limit (isle), every provider in turn: its logo, name, plan and how
    /// fresh the numbers are, then a line and a bar per limit.
    fn usage_view(&mut self, x: f32, y: f32, w: f32, state: &IslandState) {
        use crate::layout::{USAGE_GAP, USAGE_HEAD, USAGE_LIMIT};
        let fallback = [("claude", "Claude", state.claude_quota.used_pct), ("codex", "Codex", state.codex_quota.used_pct)]
            .map(|(id, name, pct)| isle_core::state::UsageProvider {
                id: id.into(), name: name.into(),
                windows: vec![isle_core::state::UsageWindow { label: "Plano".into(), percent: pct as f64, resets_at_ms: 0 }],
                ..Default::default()
            });
        let providers = if state.usage.is_empty() { &fallback[..] } else { &state.usage[..] };
        let (left, right) = (x + 20., x + w - 20.);
        let mut ty = y + CONTENT + 14.;
        for p in providers {
            // ACCENTS: Claude's orange, Codex's green; red once nearly used up.
            let accent_color = match p.id.as_str() { "claude" => Color::from_rgba8(217, 120, 87, 255), "codex" => Color::from_rgba8(15, 163, 128, 255), _ => accent() };
            self.provider_logo(&p.id, left, ty + 1., 18.);
            let mut hx = left + 26.;
            hx += self.label(&p.name, hx, ty, 15., Weight::Semibold, white()) + 8.;
            if !p.plan.is_empty() {
                let pw = self.text.width(&p.plan, 11., Weight::Semibold) + 14.;
                self.round(hx, ty + 2., pw, 17., 8.5, Color::from_rgba8(255, 255, 255, 26));
                self.centered_label(&p.plan, hx + pw / 2., centered_top(ty + 2., 17., 11.), 11., Weight::Semibold, Color::from_rgba8(255, 255, 255, 179));
                hx += pw + 8.;
            }
            let from_codex = p.note == "Do último uso do Codex";
            let warning = !p.note.is_empty() && !from_codex;
            let stamp = if warning { p.note.clone() } else { usage_updated(p.updated_ms, from_codex, state) };
            let stamp = self.text.ellipsize(&stamp, (right - hx).max(0.), 11., Weight::Regular);
            let sw = self.text.width(&stamp, 11., Weight::Regular);
            self.label(&stamp, right - sw, centered_top(ty, USAGE_HEAD, 11.), 11., Weight::Regular, Color::from_rgba8(255, 255, 255, if warning { 204 } else { 140 }));
            ty += USAGE_HEAD + 14.;
            if p.windows.is_empty() {
                self.label("Nenhum limite informado ainda", left, ty, 12., Weight::Regular, Color::from_rgba8(255, 255, 255, 140));
                ty += USAGE_LIMIT;
            }
            for limit in &p.windows {
                let used = limit.percent.clamp(0., 100.) as f32;
                let shown = used.round();
                let critical = shown >= 95.;
                let pct = format!("{shown:.0}%");
                let tw = self.text.width(&pct, 13., Weight::Semibold);
                let pw = tw.max(40.);
                self.label(&pct, right - tw, ty, 13., Weight::Semibold, if critical { red() } else { white() });
                let mut name_room = right - pw - 10. - left;
                if limit.resets_at_ms > 0 {
                    let reset = usage_resets(limit.resets_at_ms, state.now_ms);
                    let rw = self.text.width(&reset, 11., Weight::Regular);
                    self.label(&reset, right - pw - 10. - rw, ty + 2., 11., Weight::Regular, Color::from_rgba8(255, 255, 255, 140));
                    name_room -= rw + 10.;
                }
                self.clipped_label(&limit.label, left, ty, name_room.max(0.), 13., Weight::Semibold, Color::from_rgba8(255, 255, 255, 230));
                // A sliver shows even at 1%.
                let (bar_y, bw) = (ty + 22., right - left);
                self.round(left, bar_y, bw, 5., 2.5, Color::from_rgba8(255, 255, 255, 26));
                if used > 0. {
                    self.round(left, bar_y, (bw * used / 100.).max(5.), 5., 2.5, if critical { red() } else { accent_color });
                }
                ty += USAGE_LIMIT;
            }
            ty += USAGE_GAP - 11.;
        }
    }

    pub(super) fn ai_view(&mut self, x: f32, y: f32, w: f32, h: f32, state: &IslandState) {
        let c = &state.chat;
        if !c.open {
            self.usage_view(x, y, w, state);
            return;
        }
        for hb in crate::chat::hitboxes(x, y, w, h, c) {
            match hb.action {
                UiAction::AiBack => self.action_icon(&hb, Icon::Left, false),
                UiAction::AiNew => self.action_icon(&hb, Icon::Plus, false),
                UiAction::AiProvider(p) => {
                    let on = p == c.provider;
                    self.round(
                        hb.x,
                        hb.y,
                        hb.w,
                        hb.h,
                        9.,
                        if on { surface() } else { Color::TRANSPARENT },
                    );
                    let name = provider_name(p);
                    if !matches!(p, isle_core::state::AiProviderKind::OpenCodeGo) {
                        self.provider_logo(name, hb.x + 9., hb.y + 8., 14.);
                    } else {
                        self.ui_icon(Icon::Spark, hb.x + 16., hb.y + 15., 14., muted());
                    }
                    let label = if on && !c.model_label.is_empty() { format!("{name} · {}", c.model_label) } else { name.to_owned() };
                    self.clipped_label(
                        &label,
                        hb.x + 30.,
                        centered_top(hb.y, hb.h, 11.),
                        hb.w - 35.,
                        11.,
                        Weight::Medium,
                        if on { white() } else { muted() },
                    );
                }
                UiAction::AiIdea(i) => {
                    // .isle-voice-idea: a pill, white 0.09 under 0.8 text.
                    self.round(hb.x, hb.y, hb.w, hb.h, hb.h / 2., Color::from_rgba8(255, 255, 255, 23));
                    self.centered_label(crate::chat::IDEAS[i].0, hb.x + hb.w / 2., centered_top(hb.y, hb.h, 12.), 12., Weight::Semibold, Color::from_rgba8(255, 255, 255, 204));
                }
                UiAction::AiScroll(n) => {
                    self.action_icon(&hb, if n > 0 { Icon::Up } else { Icon::Down }, false)
                }
                UiAction::AiSend | UiAction::AiStop => {
                    self.round(hb.x, hb.y, hb.w, hb.h, 11., accent());
                    self.ui_icon(
                        if c.busy { Icon::Stop } else { Icon::Send },
                        hb.x + hb.w / 2.,
                        hb.y + hb.h / 2.,
                        18.,
                        ink(),
                    );
                }
                UiAction::AiInput => {
                    self.round(hb.x, hb.y, hb.w, hb.h, 12., surface());
                    let hint = c.input.text.is_empty();
                    let shown = if hint {
                        "Pergunte qualquer coisa…".to_owned()
                    } else {
                        let mut t = c.input.text.as_str();
                        while self.text.width(t, 13., Weight::Regular) > hb.w - 30. && !t.is_empty()
                        {
                            t = &t[t.chars().next().unwrap().len_utf8()..];
                        }
                        t.to_owned()
                    };
                    let (lx, ly) = (hb.x + 13., centered_top(hb.y, hb.h, 13.));
                    self.clipped_label(&shown, lx, ly, hb.w - 25., 13., Weight::Regular, if hint { muted() } else { white() });
                    if c.caret == Some(true) {
                        // The caret sits at the insertion point: the start of the hint, else after the text.
                        let cx = if hint { lx } else { lx + self.text.width(&shown, 13., Weight::Regular) + 1. };
                        self.round(cx.min(hb.x + hb.w - 14.), ly, 1.5, 16., 0.75, white());
                    }
                }
                _ => {}
            }
        }
        let mood = if c.error.is_some() {
            Mood::Error
        } else if c.busy && c.items.last().is_some_and(|m| !m.text.is_empty()) {
            Mood::Speaking
        } else if c.busy {
            Mood::Thinking
        } else if !c.items.is_empty() {
            Mood::Happy
        } else {
            Mood::Ready
        };
        if c.items.is_empty() {
            let top = y + CONTENT + if h<400. {36.}else{52.};
            let avatar_size=if h<400.{48.}else{72.};
            self.avatar(x + w / 2. - avatar_size/2., top, avatar_size, mood);
            self.centered_label(
                "O que vamos fazer hoje?",
                x + w / 2.,
                top + avatar_size+16.,
                23.,
                Weight::Medium,
                white(),
            );
            self.centered_label(
                "Uma ideia, uma pergunta, o próximo passo.",
                x + w / 2.,
                top + avatar_size+50.,
                12.,
                Weight::Regular,
                muted(),
            );
        } else {
            self.avatar(x + 20., y + CONTENT + 47., 26., mood);
            let rows = crate::chat::lines(&self.text, c, w - 100.);
            let top = y + crate::chat::TEXT_TOP;
            let card = crate::chat::approval_card(w, h, c).map_or(0., |card| card.height);
            let capacity = ((h - 106. - card - crate::chat::TEXT_TOP) / crate::chat::LINE_HEIGHT)
                .floor()
                .max(1.) as usize;
            let range = crate::chat::visible_range(rows.len(), capacity, c.scroll);
            for (i, (user, line)) in rows[range].iter().enumerate() {
                self.label(
                    line,
                    x + 64.,
                    top + i as f32 * crate::chat::LINE_HEIGHT,
                    12.,
                    Weight::Regular,
                    if *user {
                        accent_light(0.4)
                    } else {
                        white()
                    },
                );
            }
        }
        if let (Some(a), Some(card)) = (&c.approval, crate::chat::approval_card(w, h, c)) {
            // Port of isle's command card: what it does, the command whole, Negar (Esc) and Executar (Enter).
            let top = y + card.top;
            self.round(x + 20., top, w - 40., card.height - 6., 14., surface());
            self.clipped_label(&a.title, x + 34., top + 12., w - 68., 13., Weight::Semibold, white());
            let rows = card.lines.as_ref().map_or(2, Vec::len) as f32;
            self.round(x + 34., top + 34., w - 68., 10. + rows * crate::chat::APPROVAL_LINE, 8., Color::from_rgba8(0, 0, 0, 90));
            match &card.lines {
                Some(lines) => for (i, line) in lines.iter().enumerate() {
                    self.label(line, x + 44., top + 39. + i as f32 * crate::chat::APPROVAL_LINE, 12., Weight::Regular, accent_light(0.4));
                },
                None => {
                    self.clipped_label("Comando longo demais ou com caracteres escondidos.", x + 44., top + 39., w - 88., 12., Weight::Regular, red());
                    self.clipped_label("Negue e peça um comando menor.", x + 44., top + 39. + crate::chat::APPROVAL_LINE, w - 88., 12., Weight::Regular, muted());
                }
            }
            for hb in crate::chat::hitboxes(x, y, w, h, c) {
                match hb.action {
                    UiAction::AiDeny => {
                        self.round(hb.x, hb.y, hb.w, hb.h, hb.h / 2., Color::from_rgba8(255, 255, 255, 23));
                        self.label("Negar", hb.x + 12., centered_top(hb.y, hb.h, 12.), 12., Weight::Semibold, white());
                        self.label_right("esc", hb.x + hb.w - 11., centered_top(hb.y, hb.h, 10.), Weight::Regular, muted());
                    }
                    UiAction::AiRun => {
                        self.round(hb.x, hb.y, hb.w, hb.h, hb.h / 2., accent());
                        self.label("Executar", hb.x + 12., centered_top(hb.y, hb.h, 12.), 12., Weight::Semibold, ink());
                        self.label_right("↵", hb.x + hb.w - 11., centered_top(hb.y, hb.h, 12.), Weight::Semibold, ink());
                    }
                    _ => {}
                }
            }
        }
        if crate::chat::ideas_shown(c) {
            return;
        }
        let waiting = c.approval.is_some();
        let status = if let Some(e) = &c.error {
            e.as_str()
        } else if waiting {
            ""
        } else if c.busy {
            "Respondendo…"
        } else {
            "Enter envia · Esc libera o teclado"
        };
        self.clipped_label(
            status,
            x + 103.,
            y + h - 86.,
            w - 128.,
            11.,
            Weight::Regular,
            if c.error.is_some() { red() } else { muted() },
        );
        if c.busy {
            self.round(x + 20., y + h - 55., w - 86., 38., 12., surface());
            self.label(
                if waiting { "Aguardando sua aprovação · Enter executa · Esc nega" } else { "Sua IA está trabalhando…" },
                x + 33.,
                y + h - 44.,
                12.,
                Weight::Regular,
                muted(),
            );
        }
    }
}
