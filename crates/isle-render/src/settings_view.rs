//! The settings window drawn in the Cimori look (mockup: config-janela.html), on the same renderer
//! as the island: the preview is the real bar.
use super::*;
use crate::settings::{self, Layout, RowBox, SettingsAction, SettingsView};
use isle_core::prefs::{PrefKind, PrefValue, SettingsTab};

fn win_bg() -> Color {
    Color::from_rgba8(27, 30, 36, 255)
}
fn side_bg() -> Color {
    Color::from_rgba8(23, 25, 30, 255)
}
fn card_bg() -> Color {
    Color::from_rgba8(34, 38, 45, 255)
}
fn card2() -> Color {
    Color::from_rgba8(42, 46, 54, 255)
}
fn well() -> Color {
    Color::from_rgba8(26, 29, 34, 255)
}
fn off_track() -> Color {
    Color::from_rgba8(58, 63, 72, 255)
}
fn faint() -> Color {
    Color::from_rgba8(109, 115, 124, 255)
}

pub(crate) fn tab_icon(tab: SettingsTab) -> Icon {
    match tab {
        SettingsTab::General => Icon::Gear,
        SettingsTab::Bar => Icon::Pill,
        SettingsTab::Look => Icon::Palette,
        SettingsTab::Icons => Icon::Tray,
        SettingsTab::Monitors => Icon::Monitor,
        SettingsTab::Hiding => Icon::EyeOff,
        SettingsTab::Animations => Icon::Wave,
        SettingsTab::Ai => Icon::Spark,
        SettingsTab::Control => Icon::Controls,
        SettingsTab::Agenda => Icon::Calendar,
        SettingsTab::Weather => Icon::Sun,
        SettingsTab::Media => Icon::Music,
        SettingsTab::Notifications => Icon::Bell,
        SettingsTab::Focus => Icon::Timer,
        SettingsTab::Tray => Icon::Tray,
        SettingsTab::Clipboard => Icon::Clipboard,
        SettingsTab::Shelf => Icon::Folder,
        SettingsTab::Usb => Icon::Usb,
        SettingsTab::Downloads => Icon::Download,
        SettingsTab::Shortcuts => Icon::Keyboard,
        SettingsTab::Backup => Icon::Download,
        SettingsTab::Updates => Icon::Restart,
        SettingsTab::About => Icon::Info,
        SettingsTab::Welcome => Icon::Smile,
    }
}

impl PillRenderer {
    /// The settings window, the size this renderer was made for (logical pixels).
    pub fn render_settings(&mut self, view: &SettingsView, cfg: &IsleConfig, state: &IslandState) -> &PillFrameBuffer {
        self.icon_cache.prepare(cfg);
        self.glass_active = false;
        let s = self.scale;
        let (w, h) = (self.framebuffer.width as f32 / s, self.framebuffer.height as f32 / s);
        self.layer.fill(win_bg());
        let l = settings::layout(view, cfg, &self.text, w, h);

        if let Some(rect) = l.preview {
            self.settings_preview(rect, view, cfg, state, &l);
        }
        for card in &l.cards {
            self.round(card.x, card.y, card.w, card.h, 12., card_bg());
            let title = card.title.to_uppercase();
            self.label(&title, card.x + 18., card.y + 12., 10.5, Weight::Semibold, faint());
        }
        for row in &l.rows {
            self.settings_row(row, view, cfg);
        }
        self.settings_hover(&l, view);

        // The header over what scrolled under it, then the side bar.
        self.round(l.nav_w, 0., w - l.nav_w, settings::HEADER, 0., win_bg());
        let pad = if l.narrow { 16. } else { 32. };
        let searching = !view.search.trim().is_empty();
        let title = if searching { "Busca" } else { view.tab.title() };
        self.label(title, l.nav_w + pad, 18., 22., Weight::Semibold, white());
        let sub = view.status.clone().unwrap_or_else(|| if searching { String::new() } else { view.tab.subtitle().to_string() });
        if !l.narrow && !sub.is_empty() {
            let color = if view.status.is_some() { accent() } else { muted() };
            self.label_right(&sub, w - pad, 28., Weight::Regular, color);
        }
        self.settings_nav(&l, view);
        self.settings_search(&l, view);

        self.pixmap.fill(win_bg());
        self.pixmap.draw_pixmap(0, 0, self.layer.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
        self.finish()
    }

    fn settings_nav(&mut self, l: &Layout, view: &SettingsView) {
        self.round(0., 0., l.nav_w, l.h, 0., side_bg());
        self.round(l.nav_w - 1., 0., 1., l.h, 0., line());
        if !l.narrow {
            self.round(20., 18., 26., 26., 7., ink());
            self.round(26., 26., 14., 4., 2., accent());
            self.round(27., 33., 12., 3., 1.5, accent_alpha(115));
            self.label("Cimori", 56., 21., 15., Weight::Semibold, white());
        }
        for entry in &l.nav_entries {
            if entry.y < 96. && !l.narrow || entry.y > l.h {
                continue;
            }
            match entry.tab {
                None => {
                    let g = entry.label.to_uppercase();
                    self.label(&g, 22., entry.y + 10., 10., Weight::Semibold, faint());
                }
                Some(tab) => {
                    let Some(hb) = l.nav.iter().find(|n| n.action == SettingsAction::Tab(tab)) else { continue };
                    let on = tab == view.tab && view.search.trim().is_empty();
                    if on {
                        self.round(hb.x, hb.y, hb.w, hb.h, 7., tint());
                    }
                    let color = if on { accent() } else { Color::from_rgba8(207, 210, 215, 255) };
                    if l.narrow {
                        self.ui_icon(tab_icon(tab), hb.x + hb.w / 2., hb.y + hb.h / 2., 18., color);
                    } else {
                        self.ui_icon(tab_icon(tab), hb.x + 16., hb.y + hb.h / 2., 16., color);
                        self.label(tab.title(), hb.x + 34., centered_top(hb.y, hb.h, 13.), 13., Weight::Regular, color);
                    }
                    if view.focus == Some(settings::Focus::Nav(l.nav.iter().position(|n| n.y == hb.y).unwrap_or(0))) {
                        self.focus_ring(hb.x, hb.y, hb.w, hb.h);
                    }
                }
            }
        }
    }

    fn settings_search(&mut self, l: &Layout, view: &SettingsView) {
        let (x, y, w, h) = l.search;
        if w < 60. {
            return;
        }
        self.round(x, y, w, h, 8., card_bg());
        self.ui_icon(Icon::Search, x + 16., y + h / 2., 14., muted());
        let typing = view.focus == Some(settings::Focus::Search);
        let (text, color) = if view.search.is_empty() && !typing {
            ("Buscar configuração".to_string(), muted())
        } else {
            (view.search.clone(), white())
        };
        let shown = self.text.ellipsize(&text, w - 60., 12.5, Weight::Regular);
        let end = self.label(&shown, x + 30., centered_top(y, h, 12.5), 12.5, Weight::Regular, color);
        if typing && view.caret {
            // Drawn as the chat input's: at the end of the text, blinking at the system's rate.
            self.round(x + 30. + if view.search.is_empty() { 0. } else { end + 1. }, centered_top(y, h, 12.5), 1.5, 16., 0.75, white());
        }
        if !view.search.trim().is_empty() {
            self.ui_icon(Icon::Close, x + w - 14., y + h / 2., 11., muted());
        }
        if typing {
            self.focus_ring(x, y, w, h);
        }
    }

    fn focus_ring(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let s = self.scale;
        if let Some(path) = build_rounded_rect((x - 2.) * s, (y - 2.) * s, (w + 4.) * s, (h + 4.) * s, 9. * s) {
            let mut paint = Paint { anti_alias: true, ..Default::default() };
            paint.set_color(accent());
            self.layer.stroke_path(&path, &paint, &Stroke { width: 2. * s, ..Default::default() }, Transform::identity(), None);
        }
    }

    fn settings_hover(&mut self, l: &Layout, view: &SettingsView) {
        let Some((px, py)) = view.hover else { return };
        let Some(hb) = l.hits.iter().find(|b| b.contains(px, py)) else { return };
        if matches!(hb.action, SettingsAction::Tab(_) | SettingsAction::FocusSearch | SettingsAction::ClearSearch) {
            return;
        }
        let color = Color::from_rgba8(245, 241, 235, if self.pressed { 22 } else { 10 });
        self.round(hb.x - 4., hb.y - 2., hb.w + 8., hb.h + 4., 8., color);
    }

    fn settings_preview(&mut self, (x, y, w, h): (f32, f32, f32, f32), view: &SettingsView, cfg: &IsleConfig, state: &IslandState, l: &Layout) {
        self.round(x, y, w, h, 12., Color::from_rgba8(44, 49, 58, 255));
        self.label("PRÉVIA", x + 14., y + 10., 10., Weight::Semibold, faint());
        let mut st = state.clone();
        st.mode = PillMode::CompactBar;
        st.bar = cfg.bar_shown();
        st.bar_quota_strip = cfg.bar_quota_strip;
        st.modules = cfg.modules;
        st.ui = cfg.ui;
        st.phase8.download_meter = cfg.modules.downloads && cfg.download_meter;
        let pw = crate::pages::compact_tray_width(&st).min(w - 40.);
        let (px, py) = (x + (w - pw) / 2., y + 30.);
        let (r, g, b) = cfg.background_rgb;
        let ph = cfg.ui.pill_height_px();
        let radius = (ph * 0.5 * cfg.notch_curve as f32 / 75.).min(ph * 0.5);
        if cfg.ui.pill_style == isle_core::config::PillStyle::Notch {
            // The notch hangs from the preview's top edge, as from the screen's.
            let nr = super::notch_radius(ph, cfg.notch_curve);
            let s = self.scale;
            if let Some(path) = super::build_notch(px * s, pw * s, ph * s, nr * s, super::notch_shoulder(nr, cfg.notch_curve) * s, true) {
                let mut paint = Paint { anti_alias: true, ..Default::default() };
                paint.set_color(Color::from_rgba8(r, g, b, 255));
                self.layer.fill_path(&path, &paint, FillRule::Winding, Transform::from_translate(0., y * s), None);
            }
            self.draw_compact(px, y, pw, ph, &st);
        } else {
        self.round(px, py + 3., pw, ph, radius, Color::from_rgba8(0, 0, 0, 60));
        self.round(px, py, pw, ph, radius, Color::from_rgba8(r, g, b, 255));
        self.draw_compact(px, py, pw, ph, &st);
        }
        if view.tab == SettingsTab::Bar {
            let strip = crate::settings::strip_pref(cfg);
            let items = settings::chip_items(strip, cfg, view);
            let chips: Vec<_> = l.hits.iter().filter(|b| matches!(&b.action, SettingsAction::ChipGrab(p, _) | SettingsAction::ChipToggle(p, _) if *p == strip))
                .cloned().collect();
            self.chips_from_hits(&items, &chips, strip, view, true);
        }
    }

    /// Draws chips at the places their hits hold (a grab plus its × count as one chip).
    fn chips_from_hits(&mut self, items: &[(String, String, bool)], hits: &[settings::Hit], id: &'static str, view: &SettingsView, ordered: bool) {
        let mut i = 0;
        let mut item = 0;
        while i < hits.len() && item < items.len() {
            let (_, label, on) = &items[item];
            let (x, y, w) = if *on && ordered {
                let (a, b) = (&hits[i], hits.get(i + 1));
                i += 2;
                (a.x, a.y, a.w + b.map_or(0., |b| b.w))
            } else {
                let a = &hits[i];
                i += 1;
                (a.x, a.y, a.w)
            };
            let dragged = view.drag.is_some_and(|d| d.pref == id && *on && items[..item].iter().filter(|it| it.2).count() == d.from);
            let (x, y) = match view.drag {
                Some(d) if dragged => (d.x - w / 2., d.y - settings_chip_h() / 2.),
                _ => (x, y),
            };
            self.chip(x, y, w, label, *on, *on && ordered);
            item += 1;
        }
    }

    fn chip(&mut self, x: f32, y: f32, w: f32, label: &str, on: bool, removable: bool) {
        let h = settings_chip_h();
        if on {
            self.round(x, y, w, h, 7., card2());
            self.label(label, x + 12., centered_top(y, h, 12.), 12., Weight::Medium, white());
            if removable {
                self.ui_icon(Icon::Close, x + w - 12., y + h / 2., 9., muted());
            }
        } else {
            let s = self.scale;
            if let Some(path) = build_rounded_rect(x * s + 0.5, y * s + 0.5, w * s - 1., h * s - 1., 7. * s) {
                let mut paint = Paint { anti_alias: true, ..Default::default() };
                paint.set_color(off_track());
                self.layer.stroke_path(&path, &paint, &Stroke { width: 1. * s, dash: tiny_skia::StrokeDash::new(vec![3. * s, 3. * s], 0.), ..Default::default() }, Transform::identity(), None);
            }
            self.label(&format!("+ {label}"), x + 10., centered_top(y, h, 12.), 12., Weight::Medium, muted());
        }
    }

    fn settings_row(&mut self, row: &RowBox, view: &SettingsView, cfg: &IsleConfig) {
        let p = row.pref;
        // The name and its description.
        let desc = settings::desc_lines(row, &self.text, view);
        let (cx, cy, cw, ch) = settings::control_rect(row, &self.text);
        let name = self.text.ellipsize(p.name, settings::name_room(row, &self.text, view), 13., Weight::Regular);
        if desc.is_empty() {
            self.label(&name, row.x, centered_top(row.y, row.head, 13.), 13., Weight::Regular, white());
        } else {
            self.label(&name, row.x, row.y + 9., 13., Weight::Regular, white());
            for (i, line) in desc.iter().enumerate() {
                self.label(line, row.x, row.y + 28. + i as f32 * settings::DESC, 11.5, Weight::Regular, muted());
            }
        }
        let value = (p.get)(cfg);
        let editing = view.editing.as_ref().filter(|(id, _)| *id == p.id).map(|(_, t)| t.clone());
        match p.kind {
            PrefKind::Switch => {
                let on = value == PrefValue::Bool(true);
                self.round(cx, cy, cw, ch, ch / 2., if on { accent() } else { off_track() });
                let knob = if on { cx + cw - ch / 2. } else { cx + ch / 2. };
                self.circle(knob, cy + ch / 2., ch / 2. - 3., if on { white() } else { Color::from_rgba8(207, 210, 215, 255) });
            }
            PrefKind::Choice(options) => {
                if settings::listed(p.id) {
                    let chosen = if let PrefValue::Index(i) = value { i } else { 0 };
                    for (i,o) in options.iter().enumerate() {
                        let yy = cy + i as f32 * 34.;
                        if yy < settings::HEADER || yy+32. > self.framebuffer.height as f32/self.scale { continue; }
                        self.round(cx,yy,cw,32.,7.,if i==chosen { tint() } else { well() });
                        self.label(o,cx+12.,yy+8.,12.,Weight::Medium,if i==chosen {accent()} else {muted()});
                    }
                    return;
                }
                self.round(cx, cy, cw, ch, 8., well());
                let chosen = if let PrefValue::Index(i) = value { i } else { 0 };
                let mut x = cx + 3.;
                for (i, o) in options.iter().enumerate() {
                    let ow = self.text.width(o, 12., Weight::Medium) + 20.;
                    if i == chosen {
                        self.round(x, cy + 3., ow, ch - 6., 6., card2());
                    }
                    self.label(o, x + 10., centered_top(cy, ch, 12.), 12., Weight::Medium, if i == chosen { white() } else { muted() });
                    x += ow;
                }
            }
            PrefKind::Slider { min, max, unit, step } => {
                let v = if let PrefValue::Number(n) = value { n } else { min };
                let t = ((v - min) / (max - min)).clamp(0., 1.);
                let mid = cy + ch / 2.;
                self.round(cx, mid - 2., cw, 4., 2., off_track());
                self.round(cx, mid - 2., cw * t, 4., 2., accent());
                self.circle(cx + cw * t, mid, 7., white());
                let precision = usize::from(step < 1.);
                let shown = if unit.is_empty() { format!("{v:.precision$}") } else { format!("{v:.precision$} {unit}") };
                self.label_right(&shown, cx - 12., centered_top(cy, ch, 12.), Weight::Regular, muted());
            }
            PrefKind::Color(colors) => {
                let chosen = if let PrefValue::Rgb(r, g, b) = value { (r, g, b) } else { (0, 0, 0) };
                for (i, (r, g, b)) in colors.iter().enumerate() {
                    let (ccx, ccy) = (cx + i as f32 * 28. + 12., cy + 12.);
                    if (*r, *g, *b) == chosen {
                        self.circle(ccx, ccy, 12., accent());
                        self.circle(ccx, ccy, 10., card_bg());
                    }
                    self.circle(ccx, ccy, 8.5, Color::from_rgba8(*r, *g, *b, 255));
                }
            }
            PrefKind::Text { secret, placeholder } => {
                self.round(cx, cy, cw, ch, 7., well());
                let current = if let PrefValue::Text(t) = value { t } else { String::new() };
                let (shown, color) = match &editing {
                    Some(t) => (t.clone(), white()),
                    None if current.is_empty() => (placeholder.to_string(), faint()),
                    None if secret => ("••••••••".to_string(), white()),
                    None => (current, white()),
                };
                let shown = if editing.is_some() {
                    // While typing, the end stays in view.
                    let mut t = shown.clone();
                    while self.text.width(&t, 12.5, Weight::Regular) > cw - 24. && !t.is_empty() {
                        t.remove(0);
                    }
                    t
                } else {
                    self.text.ellipsize(&shown, cw - 20., 12.5, Weight::Regular)
                };
                let end = self.label(&shown, cx + 10., centered_top(cy, ch, 12.5), 12.5, Weight::Regular, color);
                if editing.is_some() && view.caret {
                    self.round(cx + 10. + if shown.is_empty() { 0. } else { end + 1. }, centered_top(cy, ch, 12.5), 1.5, 16., 0.75, white());
                }
                if editing.is_some() {
                    self.focus_ring(cx, cy, cw, ch);
                }
            }
            PrefKind::Action(a) => {
                let confirm = view.confirm == Some(a);
                self.round(cx, cy, cw, ch, 8., if confirm { accent() } else { card2() });
                let label = if confirm { "Confirmar?" } else { settings::action_label(p) };
                self.centered_label(label, cx + cw / 2., centered_top(cy, ch, 12.), 12., Weight::Medium, if confirm { ink() } else { white() });
            }
            PrefKind::Chips => {
                let items = settings::chip_items(p.id, cfg, view);
                let ordered = settings::closable(p.id);
                // The chips are flowed the same way the hits are.
                let mut x = 0.;
                let mut line = 0;
                for (n, (_, label, on)) in items.iter().enumerate() {
                    let w = self.text.width(label, 12., Weight::Medium) + 24. + if *on && ordered { 16. } else { 0. };
                    if x > 0. && x + w > cw {
                        x = 0.;
                        line += 1;
                    }
                    let dragged = view.drag.is_some_and(|d| d.pref == p.id && *on && items[..n].iter().filter(|it| it.2).count() == d.from);
                    let (dx, dy) = match view.drag {
                        Some(d) if dragged => (d.x - w / 2., d.y - settings_chip_h() / 2.),
                        _ => (cx + x, cy + line as f32 * 32.),
                    };
                    self.chip(dx, dy, w, label, *on, *on && ordered);
                    x += w + 6.;
                }
            }
            PrefKind::Monitors => {
                let chosen: Vec<String> = match &value {
                    PrefValue::List(l) => l.clone(),
                    PrefValue::Text(t) => vec![t.clone()],
                    _ => vec![],
                };
                for (device, mx, my, mw, mh) in settings::monitor_rects(view, (cx, cy, cw, ch)) {
                    let on = chosen.iter().any(|c| c.eq_ignore_ascii_case(&device));
                    self.round(mx, my, mw, mh, 6., if on { accent() } else { off_track() });
                    self.round(mx + 2., my + 2., mw - 4., mh - 4., 5., well());
                    let short = device.trim_start_matches(r"\\.\").to_string();
                    let shown = self.text.ellipsize(&short, mw - 12., 11.5, Weight::Medium);
                    self.centered_label(&shown, mx + mw / 2., centered_top(my, mh, 11.5), 11.5, Weight::Medium, if on { white() } else { muted() });
                }
            }
            PrefKind::CitySearch => {
                self.round(cx, cy, cw, ch, 7., well());
                let label = match &editing {
                    Some(t) => format!("{t}|"),
                    None => cfg.weather.label.clone(),
                };
                let shown = self.text.ellipsize(&label, cw - 40., 12.5, Weight::Regular);
                self.ui_icon(Icon::Search, cx + 14., cy + ch / 2., 13., muted());
                self.label(&shown, cx + 28., centered_top(cy, ch, 12.5), 12.5, Weight::Regular, white());
                for (i, (name, _, _)) in view.city_results.iter().enumerate() {
                    let ry = cy + 38. + i as f32 * 30.;
                    self.round(cx, ry, cw, 28., 6., card2());
                    let n = self.text.ellipsize(name, cw - 20., 12., Weight::Regular);
                    self.label(&n, cx + 10., centered_top(ry, 28., 12.), 12., Weight::Regular, white());
                }
            }
            PrefKind::Shortcut => {
                // Port of the mod's recorded shortcut row: the keys, or waiting for them.
                let recording = view.recording == Some(p.id);
                let code = if let PrefValue::Index(i) = value { i as u32 } else { 0 };
                self.round(cx, cy, cw, ch, 8., if recording { tint() } else { well() });
                let (text, color) = if recording {
                    ("Pressione as teclas…".to_string(), accent())
                } else {
                    (isle_core::shortcut::text(code), if code == 0 { faint() } else { white() })
                };
                let shown = self.text.ellipsize(&text, cw - 44., 12.5, Weight::Medium);
                self.label(&shown, cx + 12., centered_top(cy, ch, 12.5), 12.5, Weight::Medium, color);
                if code != 0 && !recording {
                    self.ui_icon(Icon::Close, cx + cw - 14., cy + ch / 2., 9., muted());
                }
                if recording {
                    self.focus_ring(cx, cy, cw, ch);
                }
            }
            PrefKind::Info => {
                let text = view.info(p.id).to_string();
                let shown = self.text.ellipsize(&text, cw, 12., Weight::Regular);
                self.label_right(&shown, cx + cw, centered_top(row.y, row.head, 12.), Weight::Regular, muted());
            }
        }
        if !row.enabled {
            self.round(row.x - 6., row.y, row.w + 12., row.h, 6., Color::from_rgba8(34, 38, 45, 150));
        }
        if view.focus == Some(settings::Focus::Row(p.id)) && editing.is_none() {
            self.focus_ring(cx, cy, cw, ch);
        }
    }
}

fn settings_chip_h() -> f32 {
    26.
}
