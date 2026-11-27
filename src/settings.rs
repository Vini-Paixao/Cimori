// The settings window wired to the island: open, edit, apply live, save
// (docs/superpowers/specs/2026-11-11-configuracoes-design.md).
use super::*;
use isle_core::prefs::{self, PrefKind, PrefValue, SettingsTab};
use isle_render::settings::{self as view, ChipDrag, Focus, SettingsAction, SettingsView};
use isle_render::renderer::PillRenderer;
use isle_render::settings_window::{DEFAULT_SIZE, SettingsInput, SettingsWindow};

/// The longest text a settings field takes.
pub(super) const FIELD_MAX: usize = 2048;

/// Pasted text into a one-line field: line breaks become spaces, the whole stays within FIELD_MAX.
pub(super) fn paste_line(field: &mut String, text: &str) {
    for c in text.chars() {
        if field.chars().count() >= FIELD_MAX {
            break;
        }
        match c {
            '\r' => {}
            '\n' | '\t' => field.push(' '),
            c if c.is_control() => {}
            c => field.push(c),
        }
    }
}

/// The settings window's part of the app: kept apart from the island's own state.
#[derive(Default)]
pub(super) struct Settings {
    pub(super) window: SettingsWindow,
    pub(super) view: SettingsView,
    pub(super) open: bool,
    renderer: Option<(PillRenderer, (u32, u32, u32))>,
    text: Option<isle_render::text::Text>,
    pub(super) dirty: bool,
    /// The slider being dragged; its value is applied live and saved on release.
    slider: Option<&'static str>,
    /// Where a chip was grabbed (it moves only past a few pixels).
    grabbed: Option<(f32, f32, bool)>,
    /// `Cimori --settings` with no Cimori running: open once the islands exist.
    pub(super) at_start: bool,
    /// The first run: the window opens on the welcome.
    pub(super) welcome: bool,
}

impl IsleApp {
    pub fn show_settings_at_start(&mut self) {
        self.settings.at_start = true;
    }

    /// No settings file yet (a first run, not an isle-win user moved over): the welcome opens.
    pub fn show_welcome_at_start(&mut self) {
        self.settings.at_start = true;
        self.settings.welcome = true;
    }

    /// Shows the settings window (or brings it to the front), on the tab it was left on.
    pub(crate) fn open_settings(&mut self) {
        if !self.settings.open {
            let tab = self.config.settings_window.as_ref().and_then(|p| SettingsTab::from_key(&p.tab));
            self.settings.view = SettingsView { tab: tab.unwrap_or(SettingsTab::General), ..Default::default() };
            if std::mem::take(&mut self.settings.welcome) {
                self.settings.view.tab = SettingsTab::Welcome;
            }
        }
        self.ask_models();
        self.refresh_settings_view();
        if !cfg!(test) {
            if let Err(err) = self.settings.window.open(self.config.settings_window.as_ref()) {
                tracing::warn!("configurações: {err:#}");
                return;
            }
        }
        self.settings.open = true;
        let layout = self.settings_layout();
        let tab = self.settings.view.tab;
        if let Some(entry) = layout.nav_entries.iter().find(|entry| entry.tab == Some(tab)) {
            let extra = (entry.y + 42. - layout.h).max(0.);
            self.settings.view.nav_scroll = (self.settings.view.nav_scroll + extra).min(layout.nav_max_scroll);
        }
        self.settings.dirty = true;
    }

    /// What the window shows that comes from the island: monitors, apps seen, the hooks' state.
    pub(super) fn refresh_settings_view(&mut self) {
        #[cfg(windows)]
        if !cfg!(test) {
            self.settings.view.monitors = isle_render::window::list_monitors();
        }
        let mut seen: Vec<(String, String)> = Vec::new();
        for n in self.state.notifications.ordered() {
            if !seen.iter().any(|(k, _)| k.eq_ignore_ascii_case(&n.app_id)) {
                seen.push((n.app_id.clone(), n.app_name.clone()));
            }
        }
        let tray: Vec<(String, String)> = self.state.tray.iter()
            .filter(|i| !i.exe_path.is_empty())
            .map(|i| (i.exe_path.clone(), i.display_name.clone()))
            .collect();
        // The apps with a window open now, for "hide_apps": keyed by exe name, shown by their own name.
        #[cfg(windows)]
        let windowed: Vec<(String, String)> = if cfg!(test) { Vec::new() } else {
            isle_render::window::window_apps().iter()
                .map(|path| (isle_render::window::app_key(path), isle_platform_win::tray::program_name(path)))
                .collect()
        };
        #[cfg(not(windows))]
        let windowed = Vec::new();
        // The chat's models, as each CLI listed them.
        let models = |p| self.ai_models.iter().find(|(q, _)| *q == p)
            .map(|(_, list)| list.iter().map(|m| (m.id.clone(), m.name.clone())).collect::<Vec<_>>()).unwrap_or_default();
        let (codex, opencode) = (models(isle_core::state::AiProviderKind::Codex), models(isle_core::state::AiProviderKind::OpenCodeGo));
        self.settings.view.dynamic = vec![("muted_apps", seen), ("pinned_tray_apps", tray), ("hide_apps", windowed),
            ("ai_codex_model", codex), ("ai_opencode_model", opencode)];
        let hooks = if isle_ai::agents::claude::hooked(&isle_ai::agents::claude::settings_path()) { "Instalados" } else { "Não instalados" };
        // The agents' folders (~/.claude, ~/.codex) say they are installed.
        let agent = |path: std::path::PathBuf, hooked: fn(&std::path::Path) -> bool| {
            if hooked(&path) { "Hooks instalados" } else if path.parent().is_some_and(|d| d.is_dir()) { "Encontrado, sem hooks" } else { "Não encontrado" }
        };
        let bridge = if self.state.tray_bridge { "Ligada" } else if !self.config.modules.tray { "Ligue a Bandeja para detectar" } else { "Não encontrada" };
        self.settings.view.info = vec![
            ("info.icons", self.icons_work.status.clone()),
            ("info.icon_credits", "Microsoft, Ionic, Tabler, Phosphor, Bootstrap, Iconoir, Tailwind Labs: MIT · Lucide: ISC · Google: Apache 2.0".into()),
            ("info.version", format!("Versão {}", env!("CARGO_PKG_VERSION"))),
            ("info.hooks", hooks.into()),
            ("info.bridge", bridge.into()),
            ("welcome.claude", agent(isle_ai::agents::claude::settings_path(), isle_ai::agents::claude::hooked).into()),
            ("welcome.codex", agent(isle_ai::agents::codex::hooks_path(), isle_ai::agents::codex::hooked).into()),
            ("info.update", if self.release.status.is_empty() { "Ainda não verificado".into() } else { self.release.status.clone() }),
        ];
        self.settings.view.update = self.release.newer.as_ref().map(|l| l.version.clone());
        self.settings.view.update_installable = self.release.newer.as_ref().is_some_and(|l| l.installer.is_some());
    }

    fn settings_size(&self) -> (f32, f32, f32) {
        if self.settings.open && !cfg!(test) {
            self.settings.window.size()
        } else {
            (DEFAULT_SIZE.0 as f32, DEFAULT_SIZE.1 as f32, 1.0)
        }
    }

    fn settings_layout(&mut self) -> view::Layout {
        let (w, h, _) = self.settings_size();
        let text = self.settings.text.get_or_insert_with(isle_render::text::Text::new);
        view::layout(&self.settings.view, &self.config, text, w, h)
    }

    /// A field of the settings window is being typed in while the window is on screen: its caret blinks.
    pub(super) fn settings_typing(&self) -> bool {
        let v = &self.settings.view;
        self.settings.open && (v.editing.is_some() || v.focus == Some(Focus::Search)) && (cfg!(test) || self.settings.window.is_visible())
    }

    /// The caret's phase this frame (`blink` ms each way; `u64::MAX` steady): a frame only when it changes.
    pub(super) fn blink_settings_caret(&mut self, now_ms: u64, blink: u64) {
        let caret = !self.settings_typing() || blink == u64::MAX || now_ms / blink % 2 == 0;
        if caret != self.settings.view.caret {
            self.settings.view.caret = caret;
            self.settings.dirty = true;
        }
    }

    /// Window messages, then a frame when something changed. Called on every wake.
    pub(super) fn pump_settings(&mut self) {
        for input in self.settings.window.take_inputs() {
            self.settings_input(input);
        }
        if !self.settings.open || !self.settings.dirty || cfg!(test) {
            return;
        }
        self.settings.dirty = false;
        let (w, h, scale) = self.settings_size();
        let key = (w.round() as u32, h.round() as u32, (scale * 100.0).round() as u32);
        if self.settings.renderer.as_ref().is_none_or(|(_, k)| *k != key) {
            self.settings.renderer = Some((PillRenderer::with_scale(key.0, key.1, scale), key));
        }
        let max = self.settings_layout().max_scroll();
        self.settings.view.scroll = self.settings.view.scroll.clamp(0.0, max);
        if let Some((renderer, _)) = self.settings.renderer.as_mut() {
            let fb = renderer.render_settings(&self.settings.view, &self.config, &self.state);
            self.settings.window.present(fb);
        }
    }

    /// The island's state changed: a window showing the bar's preview draws again.
    pub(super) fn settings_follow_island(&mut self) {
        if self.settings.open && matches!(self.settings.view.tab, SettingsTab::Bar | SettingsTab::Look | SettingsTab::Hiding) {
            self.settings.dirty = true;
        }
        // The bridge's state, as it comes.
        if self.settings.open && self.settings.view.tab == SettingsTab::Welcome && self.settings.view.welcome_step == 3 {
            self.refresh_settings_view();
            self.settings.dirty = true;
        }
    }

    pub(super) fn settings_input(&mut self, input: SettingsInput) {
        self.settings.dirty = true;
        match input {
            SettingsInput::Down { x, y } => {
                let l = self.settings_layout();
                match view::hit(&l, x, y) {
                    Some(SettingsAction::ChipGrab(pref, from)) => {
                        self.settings.view.drag = Some(ChipDrag { pref, from, x, y });
                        self.settings.grabbed = Some((x, y, false));
                    }
                    Some(SettingsAction::Slide(id, v)) => {
                        self.settings.slider = Some(id);
                        self.set_pref(id, PrefValue::Number(v), false);
                    }
                    _ => {}
                }
            }
            SettingsInput::Move { x, y } => {
                self.settings.view.hover = Some((x, y));
                if let Some(id) = self.settings.slider {
                    let l = self.settings_layout();
                    if let Some(b) = l.hits.iter().find(|b| matches!(b.action, SettingsAction::Slide(s, _) if s == id)) {
                        let v = view::slider_value(id, (x - b.x) / b.w);
                        self.set_pref(id, PrefValue::Number(v), false);
                    }
                }
                if let Some(drag) = self.settings.view.drag.as_mut() {
                    drag.x = x;
                    drag.y = y;
                    if let Some((gx, gy, moved)) = self.settings.grabbed.as_mut() {
                        *moved |= (x - *gx).abs() + (y - *gy).abs() > 4.0;
                    }
                }
                let hand = view::hit(&self.settings_layout(), x, y).is_some();
                self.settings.window.set_cursor(hand);
            }
            SettingsInput::Up { x, y } => {
                if let Some(id) = self.settings.slider.take() {
                    // Where it was let go counts, and only now is it written.
                    let l = self.settings_layout();
                    if let Some(b) = l.hits.iter().find(|b| matches!(b.action, SettingsAction::Slide(s, _) if s == id)) {
                        self.set_pref(id, PrefValue::Number(view::slider_value(id, (x - b.x) / b.w)), false);
                    }
                    let _ = y;
                    self.save_config();
                } else if let Some(drag) = self.settings.view.drag.take() {
                    let moved = self.settings.grabbed.take().is_some_and(|g| g.2);
                    if moved {
                        self.drop_chip(drag, x, y);
                    }
                } else {
                    let l = self.settings_layout();
                    match view::hit(&l, x, y) {
                        Some(action) => self.settings_action(action),
                        None => self.commit_edit(),
                    }
                }
            }
            SettingsInput::Leave => self.settings.view.hover = None,
            SettingsInput::Wheel { delta, x, .. } => {
                let layout = self.settings_layout();
                let step = delta as f32 / 120. * 48.;
                let v = &mut self.settings.view;
                if x < layout.nav_w {
                    v.nav_scroll = (v.nav_scroll - step).clamp(0., layout.nav_max_scroll);
                } else {
                    v.scroll = (v.scroll - step).clamp(0., layout.max_scroll());
                }
            }
            SettingsInput::Char(c) => self.settings_char(c),
            SettingsInput::Chord(code) => self.record_shortcut(code),
            SettingsInput::Key(_, _) if self.settings.view.recording.is_some() => {}
            SettingsInput::Key(vk, shift) => self.settings_key(vk, shift),
            SettingsInput::Resized => {}
            SettingsInput::CaptureLost => {
                if self.settings.slider.take().is_some() {
                    self.save_config();
                }
                self.settings.view.drag = None;
                self.settings.grabbed = None;
            }
            SettingsInput::Closed => self.close_settings(),
        }
    }

    fn close_settings(&mut self) {
        self.commit_edit();
        if let Some(mut p) = self.settings.window.placement() {
            p.tab = self.settings.view.tab.key().to_string();
            self.config.settings_window = Some(p);
            self.save_config();
        }
        if !cfg!(test) {
            self.settings.window.close();
        }
        self.settings.open = false;
        self.settings.renderer = None;
        self.settings.slider = None;
        self.settings.view.drag = None;
    }

    /// A value set from the window: applied now, saved unless a slider is still moving.
    fn set_pref(&mut self, id: &str, value: PrefValue, save: bool) {
        let Some(p) = prefs::by_id(id) else { return };
        if !(p.enabled)(&self.config) {
            return;
        }
        let old = self.config.clone();
        (p.set)(&mut self.config, value);
        if self.config != old {
            self.apply_config(&old, save);
        }
        self.settings.dirty = true;
    }

    /// Port of ControlPanel::OnShortcutRecorded (mac-island 3.1.1): Esc stops waiting; keys without
    /// Win, Ctrl or Alt wait for others; the rest become the row's shortcut.
    fn record_shortcut(&mut self, code: u32) {
        self.settings.view.just_recorded = false;
        let Some(id) = self.settings.view.recording else { return };
        self.settings.dirty = true;
        if code == 0x1B {
            self.settings.view.recording = None;
            self.settings.view.status = None;
            self.settings.view.just_recorded = true;
            return;
        }
        if !isle_core::shortcut::accepts(code) {
            self.settings.view.status = Some("Use Win, Ctrl ou Alt com uma tecla".into());
            return;
        }
        self.settings.view.recording = None;
        self.settings.view.status = None;
        self.settings.view.just_recorded = true;
        self.set_pref(id, PrefValue::Index(code as usize), true);
    }

    pub(super) fn settings_action(&mut self, action: SettingsAction) {
        if !matches!(action, SettingsAction::Edit(_)) {
            self.commit_edit();
        }
        if !matches!(action, SettingsAction::Record(_)) && self.settings.view.recording.take().is_some() {
            self.settings.view.status = None;
        }
        if !matches!(action, SettingsAction::Run(_)) {
            self.settings.view.confirm = None;
        }
        let v = &mut self.settings.view;
        match action {
            SettingsAction::Tab(tab) => {
                v.tab = tab;
                v.scroll = 0.0;
                v.search.clear();
                v.focus = None;
            }
            SettingsAction::FocusSearch => v.focus = Some(Focus::Search),
            SettingsAction::ClearSearch => {
                v.search.clear();
                v.scroll = 0.0;
            }
            SettingsAction::Toggle(id) => {
                if let Some(PrefValue::Bool(b)) = prefs::by_id(id).map(|p| (p.get)(&self.config)) {
                    self.set_pref(id, PrefValue::Bool(!b), true);
                }
            }
            SettingsAction::Choose(id, i) => self.set_pref(id, PrefValue::Index(i), true),
            SettingsAction::Slide(id, value) => self.set_pref(id, PrefValue::Number(value), true),
            SettingsAction::Pick(id, (r, g, b)) => self.set_pref(id, PrefValue::Rgb(r, g, b), true),
            SettingsAction::Edit(id) => {
                let current = match prefs::by_id(id).map(|p| ((p.get)(&self.config), p.kind)) {
                    Some((_, PrefKind::CitySearch)) => String::new(),
                    Some((PrefValue::Text(t), _)) => t,
                    _ => String::new(),
                };
                self.settings.view.editing = Some((id, current));
                self.settings.view.focus = Some(Focus::Row(id));
            }
            SettingsAction::ChipToggle(id, key) => {
                if let Some(PrefValue::List(mut list)) = prefs::by_id(id).map(|p| (p.get)(&self.config)) {
                    match list.iter().position(|k| *k == key) {
                        Some(i) => {
                            list.remove(i);
                        }
                        None => list.push(key),
                    }
                    self.set_pref(id, PrefValue::List(list), true);
                }
            }
            SettingsAction::ChipGrab(..) => {}
            SettingsAction::Run(action) => self.run_settings_action(action),
            SettingsAction::PickCity(i) => self.pick_city(i),
            SettingsAction::Monitor(device) => {
                let mut list = self.config.monitors.clone();
                match list.iter().position(|m| m.eq_ignore_ascii_case(&device)) {
                    Some(i) if list.len() > 1 => {
                        list.remove(i);
                    }
                    Some(_) => {}
                    None => list.push(device),
                }
                self.set_pref("monitors", PrefValue::List(list), true);
            }
            SettingsAction::MainMonitor(device) => self.set_pref("main_monitor", PrefValue::Text(device), true),
            SettingsAction::Record(id) => {
                let enabled = prefs::by_id(id).is_some_and(|p| (p.enabled)(&self.config));
                v.recording = (v.recording != Some(id) && enabled).then_some(id);
                v.focus = Some(Focus::Row(id));
            }
            SettingsAction::ClearShortcut(id) => self.set_pref(id, PrefValue::Index(0), true),
        }
    }

    /// A dragged chip let go: on another one, it takes that place; elsewhere in its strip, the end;
    /// outside, it leaves (never the last).
    fn drop_chip(&mut self, drag: ChipDrag, x: f32, y: f32) {
        let Some(PrefValue::List(mut list)) = prefs::by_id(drag.pref).map(|p| (p.get)(&self.config)) else { return };
        if drag.from >= list.len() {
            return;
        }
        let l = self.settings_layout();
        let target = l.hits.iter().find_map(|b| match b.action {
            SettingsAction::ChipGrab(p, i) if p == drag.pref && b.contains(x, y) => Some(i),
            _ => None,
        });
        let area = if drag.pref == isle_render::settings::strip_pref(&self.config) {
            l.preview
        } else {
            l.rows.iter().find(|r| r.pref.id == drag.pref).map(|r| (r.x, r.y, r.w, r.h))
        };
        let inside = area.is_some_and(|(ax, ay, aw, ah)| x >= ax && x <= ax + aw && y >= ay && y <= ay + ah);
        let item = list.remove(drag.from);
        match (target, inside) {
            (Some(i), _) => list.insert(i.min(list.len()), item),
            (None, true) => list.push(item),
            (None, false) if list.is_empty() => list.push(item),
            (None, false) => {}
        }
        self.set_pref(drag.pref, PrefValue::List(list), true);
    }

    fn commit_edit(&mut self) {
        let Some((id, text)) = self.settings.view.editing.take() else { return };
        if prefs::by_id(id).is_some_and(|p| matches!(p.kind, PrefKind::CitySearch)) {
            self.search_city(text);
        } else {
            self.set_pref(id, PrefValue::Text(text), true);
        }
    }

    fn settings_char(&mut self, c: char) {
        let v = &mut self.settings.view;
        // The keys go to the shortcut being recorded (as a Chord), not to the search.
        if v.recording.is_some() || std::mem::take(&mut v.just_recorded) {
            return;
        }
        if let Some((_, text)) = v.editing.as_mut() {
            match c {
                '\r' => self.commit_edit(),
                '\u{1b}' => v.editing = None,
                '\u{8}' => {
                    text.pop();
                }
                '\u{16}' => paste_line(text, &if cfg!(test) { String::new() } else { notifications::clipboard_text() }),
                c if !c.is_control() && text.chars().count() < FIELD_MAX => text.push(c),
                _ => {}
            }
            return;
        }
        match c {
            '\u{1b}' if !v.search.is_empty() => {
                v.search.clear();
                v.scroll = 0.0;
            }
            '\u{1b}' => self.close_settings(),
            ' ' if v.focus == Some(Focus::Search) || !v.search.is_empty() => {
                v.search.push(' ');
                v.focus = Some(Focus::Search);
            }
            '\r' | ' ' => {
                if let Some(Focus::Row(id)) = v.focus {
                    self.activate_row(id);
                } else if let Some(Focus::Nav(i)) = v.focus {
                    if let Some(tab) = self.settings_layout().nav.get(i).map(|n| n.action.clone()) {
                        self.settings_action(tab);
                    }
                }
            }
            '\u{8}' => {
                v.search.pop();
                v.focus = Some(Focus::Search);
            }
            '\u{16}' => {
                paste_line(&mut v.search, &if cfg!(test) { String::new() } else { notifications::clipboard_text() });
                v.focus = Some(Focus::Search);
            }
            // Typing anywhere searches (the mod's window does the same).
            c if !c.is_control() && v.search.chars().count() < 80 => {
                v.search.push(c);
                v.focus = Some(Focus::Search);
                v.scroll = 0.0;
            }
            _ => {}
        }
    }

    /// Space or Enter on a focused row: what a click on its control does.
    fn activate_row(&mut self, id: &'static str) {
        let Some(p) = prefs::by_id(id) else { return };
        match p.kind {
            PrefKind::Switch => self.settings_action(SettingsAction::Toggle(id)),
            PrefKind::Text { .. } | PrefKind::CitySearch => self.settings_action(SettingsAction::Edit(id)),
            PrefKind::Action(a) => self.settings_action(SettingsAction::Run(a)),
            PrefKind::Shortcut => self.settings_action(SettingsAction::Record(id)),
            _ => {}
        }
    }

    /// Tab and Shift+Tab walk the side bar, the search and the rows; arrows change a choice or a slider.
    fn settings_key(&mut self, vk: u16, shift: bool) {
        const TAB: u16 = 0x09;
        const LEFT: u16 = 0x25;
        const UP: u16 = 0x26;
        const RIGHT: u16 = 0x27;
        const DOWN: u16 = 0x28;
        const HOME: u16 = 0x24;
        const END: u16 = 0x23;
        const PAGE_UP: u16 = 0x21;
        const PAGE_DOWN: u16 = 0x22;
        let l = self.settings_layout();
        let mut stops: Vec<Focus> = (0..l.nav.len()).map(Focus::Nav).collect();
        stops.push(Focus::Search);
        stops.extend(l.rows.iter().filter(|r| r.enabled && !matches!(r.pref.kind, PrefKind::Info)).map(|r| Focus::Row(r.pref.id)));
        let at = self.settings.view.focus.and_then(|f| stops.iter().position(|s| *s == f));
        match vk {
            TAB => {
                self.commit_edit();
                let n = stops.len();
                let next = match at {
                    None => if shift { n - 1 } else { 0 },
                    Some(i) if shift => (i + n - 1) % n,
                    Some(i) => (i + 1) % n,
                };
                self.settings.view.focus = stops.get(next).copied();
                self.scroll_focus_into_view(&l);
            }
            LEFT | RIGHT | UP | DOWN => {
                let step: i32 = if matches!(vk, LEFT | UP) { -1 } else { 1 };
                match self.settings.view.focus {
                    Some(Focus::Nav(i)) if matches!(vk, UP | DOWN) => {
                        let n = l.nav.len() as i32;
                        let i = (i as i32 + step).rem_euclid(n.max(1)) as usize;
                        self.settings.view.focus = Some(Focus::Nav(i));
                        if let Some(tab) = l.nav.get(i).map(|n| n.action.clone()) {
                            self.settings_action(tab);
                            self.settings.view.focus = Some(Focus::Nav(i));
                        }
                    }
                    Some(Focus::Row(id)) => {
                        let Some(p) = prefs::by_id(id) else { return };
                        match (p.kind, (p.get)(&self.config)) {
                            (PrefKind::Choice(options), PrefValue::Index(i)) => {
                                let i = (i as i32 + step).clamp(0, options.len() as i32 - 1) as usize;
                                self.set_pref(id, PrefValue::Index(i), true);
                            }
                            (PrefKind::Slider { min, max, step: s, .. }, PrefValue::Number(n)) => {
                                self.set_pref(id, PrefValue::Number((n + s * step as f32).clamp(min, max)), true);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            HOME => self.settings.view.scroll = 0.0,
            END => self.settings.view.scroll = l.max_scroll(),
            PAGE_UP | PAGE_DOWN => {
                let page = (l.h - view::HEADER) * 0.8 * if vk == PAGE_UP { -1.0 } else { 1.0 };
                self.settings.view.scroll = (self.settings.view.scroll + page).clamp(0.0, l.max_scroll());
            }
            _ => {}
        }
    }

    fn scroll_focus_into_view(&mut self, l: &view::Layout) {
        let Some(Focus::Row(id)) = self.settings.view.focus else { return };
        let Some(r) = l.rows.iter().find(|r| r.pref.id == id) else { return };
        let v = &mut self.settings.view;
        if r.y < view::HEADER {
            v.scroll = (v.scroll - (view::HEADER + 8.0 - r.y)).max(0.0);
        } else if r.y + r.h > l.h {
            v.scroll = (v.scroll + (r.y + r.h - l.h + 8.0)).min(l.max_scroll());
        }
    }

    /// What an Action row's button does.
    pub(super) fn run_settings_action(&mut self, action: isle_core::prefs::ActionId) {
        use isle_core::prefs::ActionId::*;
        use isle_platform_win::files::shell_open;
        let status = |app: &mut Self, text: &str| {
            app.settings.view.status = Some(text.to_string());
            app.settings.dirty = true;
        };
        match action {
            PickIconFolder => self.pick_icons(),
            DeleteIconPack => self.delete_icons(),
            ReloadIcons => self.start_icons(),
            Restore if self.settings.view.confirm != Some(Restore) => self.settings.view.confirm = Some(Restore),
            Restore => {
                self.settings.view.confirm = None;
                let old = self.config.clone();
                self.config = IsleConfig { settings_window: old.settings_window.clone(), ..IsleConfig::default() };
                self.apply_config(&old, true);
                status(self, "Configurações restauradas");
            }
            OpenFolder => {
                if let Some(dir) = self.settings_path.as_ref().and_then(|p| p.parent()) {
                    if !cfg!(test) {
                        shell_open(&dir.to_string_lossy());
                    }
                }
            }
            Quit => {
                self.close_settings();
                self.save_config();
                #[cfg(windows)]
                if !cfg!(test) {
                    unsafe { windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0) };
                }
            }
            InstallHooks | RemoveHooks => {
                if cfg!(test) {
                    return;
                }
                let done = if action == InstallHooks { crate::install_hooks(None) } else { crate::uninstall_hooks() };
                match done {
                    Ok(()) => status(self, if action == InstallHooks { "Hooks instalados" } else { "Hooks removidos" }),
                    Err(err) => {
                        tracing::warn!("hooks: {err:#}");
                        status(self, "Não deu para mexer nos hooks");
                    }
                }
                self.refresh_settings_view();
            }
            Export | Import => {
                let save = action == Export;
                let Some(Services { tx, waker, .. }) = self.services.clone() else { return };
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64 + local_offset();
                let (y, m, d) = civil_from_days(now.div_euclid(86_400));
                let name = format!("cimori-backup-{y:04}-{m:02}-{d:02}.json");
                // The dialog is modal: on a thread of its own, the island keeps going.
                std::thread::spawn(move || {
                    let path = isle_platform_win::files::pick_backup_file(save, &name);
                    send(&tx, &waker, AppEvent::BackupFile { save, path });
                });
            }
            WelcomeBack | WelcomeNext => {
                let v = &mut self.settings.view;
                v.welcome_step = if action == WelcomeBack { v.welcome_step.saturating_sub(1) } else { (v.welcome_step + 1).min(prefs::WELCOME.len() - 1) };
                v.scroll = 0.0;
                v.status = None;
                v.focus = None;
                self.refresh_settings_view();
            }
            WelcomeFinish => self.settings_action(SettingsAction::Tab(SettingsTab::General)),
            ShowWelcome => {
                self.settings.view.welcome_step = 0;
                self.settings_action(SettingsAction::Tab(SettingsTab::Welcome));
            }
            CopyBridge => self.copy_bridge(),
            CheckUpdates => {
                self.check_updates_now();
                self.refresh_settings_view();
            }
            DownloadUpdate => self.open_update_page(),
            InstallUpdate => self.install_update(),
            OpenLink(url) => {
                if !cfg!(test) {
                    shell_open(url);
                }
            }
            OpenWindowsBanners => {
                if !cfg!(test) {
                    shell_open("ms-settings:notifications");
                }
            }
            OpenBridgeReadme => {
                // The bridge's instructions and its mod file, beside the settings, where Windhawk can take it from.
                let Some(dir) = self.settings_path.as_ref().and_then(|p| p.parent()).map(|d| d.join("windhawk")) else { return };
                let written = std::fs::create_dir_all(&dir)
                    .and_then(|_| std::fs::write(dir.join("README.md"), include_str!("../windhawk/README.md")))
                    .and_then(|_| std::fs::write(dir.join("cimori-tray-bridge.wh.cpp"), include_str!("../windhawk/cimori-tray-bridge.wh.cpp")));
                match written {
                    Ok(()) if !cfg!(test) => shell_open(&dir.to_string_lossy()),
                    Ok(()) => {}
                    Err(err) => tracing::warn!("ponte do Windhawk: {err}"),
                }
            }
        }
    }

    pub(super) fn backup_picked(&mut self, save: bool, path: Option<std::path::PathBuf>) {
        let Some(path) = path else { return };
        let done = if save { self.export_backup(&path) } else { self.import_backup(&path) };
        self.settings.view.status = Some(match (save, done) {
            (true, Ok(())) => "Backup exportado".into(),
            (false, Ok(())) => "Backup importado".into(),
            (_, Err(err)) => {
                tracing::warn!("backup: {err:#}");
                if save { "Não deu para exportar".into() } else { "Esse arquivo não é um backup da Cimori".into() }
            }
        });
        self.settings.dirty = true;
    }

    /// The settings, the typed reminders and the "cleared" mark, in one file.
    pub(super) fn export_backup(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let backup = serde_json::json!({
            "cimori_backup": 1,
            "settings": self.config,
            "reminders": self.reminders,
            "cleared_until": self.state.notifications.cleared_until,
        });
        isle_core::config::write_atomic(path, &serde_json::to_string_pretty(&backup)?)?;
        Ok(())
    }

    /// Reads a backup made by `export_backup`; anything else changes nothing.
    pub(super) fn import_backup(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        let text = std::fs::read_to_string(path)?;
        let backup: serde_json::Value = serde_json::from_str(&text)?;
        let settings = backup.get("settings").filter(|s| s.is_object()).ok_or_else(|| anyhow::anyhow!("sem configurações"))?;
        let reminders = backup.get("reminders").cloned().map(serde_json::from_value).transpose()?;
        let old = self.config.clone();
        self.config = IsleConfig { settings_window: old.settings_window.clone(), ..IsleConfig::load_lenient(&settings.to_string()) };
        self.apply_config(&old, true);
        if let Some(reminders) = reminders {
            self.reminders = reminders;
            self.save_reminders();
        }
        if let Some(cleared) = backup.get("cleared_until").and_then(|c| c.as_i64()) {
            self.state.notifications.cleared_until = self.state.notifications.cleared_until.max(cleared);
        }
        Ok(())
    }
    /// Asks Open-Meteo for places with this name; the answer comes back as `AppEvent::Places`.
    fn search_city(&mut self, name: String) {
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        self.settings.view.status = Some("Buscando…".into());
        let Some(Services { rt, tx, waker }) = self.services.clone() else { return };
        rt.spawn(async move {
            let url = isle_core::weather::geocoding_url(&name);
            let answer = isle_platform_win::weather::fetch_url_text(&url)
                .await
                .map(|json| isle_core::weather::parse_places(&json))
                .map_err(|e| e.to_string());
            send(&tx, &waker, AppEvent::Places(answer));
        });
    }

    pub(super) fn apply_places(&mut self, answer: Result<Vec<(String, f64, f64)>, String>) {
        let v = &mut self.settings.view;
        match answer {
            Ok(places) if places.is_empty() => {
                v.city_results.clear();
                v.status = Some("Nenhuma cidade com esse nome".into());
            }
            Ok(places) => {
                v.city_results = places;
                v.status = None;
            }
            Err(err) => {
                tracing::warn!("busca de cidade: {err}");
                v.city_results.clear();
                v.status = Some("Sem conexão para buscar a cidade".into());
            }
        }
        self.settings.dirty = true;
    }

    fn pick_city(&mut self, i: usize) {
        let Some((label, lat, lon)) = self.settings.view.city_results.get(i).cloned() else { return };
        self.settings.view.city_results.clear();
        self.settings.view.status = None;
        self.set_pref("weather", PrefValue::List(vec![label, lat.to_string(), lon.to_string()]), true);
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use isle_core::config::BarItem;

    fn app_saving_to(dir: &str) -> (IsleApp, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("cimori-settings-{dir}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let mut app = IsleApp::new(IsleConfig::default());
        app.settings_path = Some(path.clone());
        (app, path)
    }

    fn hit_where(app: &mut IsleApp, wanted: impl Fn(&SettingsAction) -> bool) -> view::Hit {
        app.settings_layout().hits.into_iter().find(|b| wanted(&b.action)).expect("hit on screen")
    }

    #[test]
    fn toggling_a_switch_applies_and_saves() {
        let (mut app, path) = app_saving_to("toggle");
        app.open_settings();
        app.settings_action(SettingsAction::Toggle("clock_seconds"));
        assert!(app.config.clock_seconds);
        assert!(std::fs::read_to_string(&path).unwrap().contains("\"clock_seconds\": true"));
    }

    #[test]
    fn dragging_a_slider_saves_only_on_release() {
        let (mut app, path) = app_saving_to("slider");
        app.open_settings();
        app.settings_action(SettingsAction::Tab(SettingsTab::Notifications));
        let b = hit_where(&mut app, |a| matches!(a, SettingsAction::Slide("banner_secs", _)));
        let y = b.y + b.h / 2.;
        app.settings_input(SettingsInput::Down { x: b.x + 1., y });
        for i in 0..20 {
            app.settings_input(SettingsInput::Move { x: b.x + b.w * i as f32 / 20., y });
        }
        assert!(!path.exists(), "nothing written while dragging");
        assert!(app.config.banner_secs > 20, "applied live");
        app.settings_input(SettingsInput::Up { x: b.x + b.w, y });
        assert!(std::fs::read_to_string(&path).unwrap().contains("\"banner_secs\": 30"));
    }

    #[test]
    fn the_last_bar_item_cannot_leave() {
        let mut cfg = IsleConfig::default();
        cfg.bar_items = vec![BarItem::Clock];
        let mut app = IsleApp::new(cfg);
        app.open_settings();
        app.settings_action(SettingsAction::Tab(SettingsTab::Bar));
        app.settings_action(SettingsAction::ChipToggle("bar_items", "clock".into()));
        assert_eq!(app.config.bar_items, vec![BarItem::Clock]);
        let grab = hit_where(&mut app, |a| matches!(a, SettingsAction::ChipGrab("bar_items", 0)));
        app.settings_input(SettingsInput::Down { x: grab.x + 4., y: grab.y + 4. });
        app.settings_input(SettingsInput::Move { x: 700., y: 700. });
        app.settings_input(SettingsInput::Up { x: 700., y: 700. });
        assert_eq!(app.config.bar_items, vec![BarItem::Clock]);
    }

    #[test]
    fn a_chip_dropped_on_another_takes_its_place() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.open_settings();
        app.settings_action(SettingsAction::Tab(SettingsTab::Bar));
        let clock = hit_where(&mut app, |a| matches!(a, SettingsAction::ChipGrab("bar_items", 0)));
        let weather = hit_where(&mut app, |a| matches!(a, SettingsAction::ChipGrab("bar_items", 2)));
        app.settings_input(SettingsInput::Down { x: clock.x + 4., y: clock.y + 4. });
        app.settings_input(SettingsInput::Move { x: weather.x + 4., y: weather.y + 4. });
        app.settings_input(SettingsInput::Up { x: weather.x + 4., y: weather.y + 4. });
        assert_eq!(app.config.bar_items, vec![BarItem::Next, BarItem::Weather, BarItem::Clock, BarItem::Tray]);
        assert_eq!(app.state.bar, app.config.bar_items, "the pill follows at once");
    }

    #[test]
    fn a_chip_dragged_out_leaves_the_bar() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.open_settings();
        app.settings_action(SettingsAction::Tab(SettingsTab::Bar));
        let tray = hit_where(&mut app, |a| matches!(a, SettingsAction::ChipGrab("bar_items", 3)));
        app.settings_input(SettingsInput::Down { x: tray.x + 4., y: tray.y + 4. });
        app.settings_input(SettingsInput::Move { x: 700., y: 700. });
        app.settings_input(SettingsInput::Up { x: 700., y: 700. });
        assert_eq!(app.config.bar_items, vec![BarItem::Clock, BarItem::Next, BarItem::Weather]);
    }

    #[test]
    fn pasted_text_becomes_one_bounded_line() {
        let mut field = String::new();
        paste_line(&mut field, "a\r\nb");
        assert_eq!(field, "a b");
        paste_line(&mut field, &"x".repeat(3000));
        assert_eq!(field.chars().count(), FIELD_MAX);
        let mut emoji = String::new();
        paste_line(&mut emoji, "ok 😀");
        assert_eq!(emoji, "ok 😀");
    }

    #[test]
    fn escape_discards_an_edit_and_enter_keeps_it() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.open_settings();
        app.settings_action(SettingsAction::Edit("holidays_country"));
        for c in "\u{8}\u{8}PT\u{1b}".chars() {
            app.settings_input(SettingsInput::Char(c));
        }
        assert_eq!(app.config.holidays_country, "", "not chosen: Windows' region");
        assert!(app.settings.view.editing.is_none());
        app.settings_action(SettingsAction::Edit("holidays_country"));
        for c in "\u{8}\u{8}pt\r".chars() {
            app.settings_input(SettingsInput::Char(c));
        }
        assert_eq!(app.config.holidays_country, "PT");
    }

    #[test]
    fn typing_outside_a_field_searches_and_escape_clears_then_closes() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.open_settings();
        for c in "volume".chars() {
            app.settings_input(SettingsInput::Char(c));
        }
        assert_eq!(app.settings.view.search, "volume");
        app.settings_input(SettingsInput::Char('\u{1b}'));
        assert!(app.settings.view.search.is_empty());
        assert!(app.settings.open);
        app.settings_input(SettingsInput::Char('\u{1b}'));
        assert!(!app.settings.open);
    }

    #[test]
    fn a_typed_field_blinks_its_caret_and_nothing_else_redraws() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.open_settings();
        app.settings.dirty = false;
        app.blink_settings_caret(530, 530);
        assert!(!app.settings.dirty, "no field typed in: no frames");
        app.settings_input(SettingsInput::Char('v'));
        assert!(app.settings_typing());
        app.settings.dirty = false;
        app.blink_settings_caret(530, 530);
        assert!(!app.settings.view.caret && app.settings.dirty, "the second half of the blink hides it");
        app.settings.dirty = false;
        app.blink_settings_caret(1060, 530);
        assert!(app.settings.view.caret && app.settings.dirty);
        app.settings.dirty = false;
        app.blink_settings_caret(1061, 530);
        assert!(!app.settings.dirty, "a frame only when the phase turns");
    }

    #[test]
    fn tab_walks_to_a_switch_and_space_flips_it() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.open_settings();
        app.settings_action(SettingsAction::Tab(SettingsTab::Hiding));
        let l = app.settings_layout();
        let stops = l.nav.len() + 1;
        for _ in 0..=stops {
            app.settings_input(SettingsInput::Key(0x09, false));
        }
        assert_eq!(app.settings.view.focus, Some(Focus::Row("auto_hide")));
        app.settings_input(SettingsInput::Char(' '));
        assert!(!app.config.auto_hide);
    }

    #[test]
    fn right_click_on_the_pill_opens_settings_but_on_a_tray_icon_does_not() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.state.tray = vec![isle_core::tray::TrayItem { key: "k".into(), pinned: true, ..Default::default() }];
        let (w, h) = (app.islands[0].geom.width.value, app.islands[0].geom.height.value);
        let icon = compute_hitboxes(CANVAS_MAX_WIDTH as f32, w, h, app.config.gap_px as f32, &app.state)
            .into_iter()
            .find(|b| matches!(b.action, UiAction::TrayClick(..)))
            .unwrap();
        app.apply_input(UiInput::RightClick { window: 0, x: icon.x + 4., y: icon.y + 4. }).unwrap();
        assert!(!app.settings.open);
        let center = CANVAS_MAX_WIDTH as f32 / 2.0;
        app.apply_input(UiInput::RightClick { window: 0, x: center - w / 2. + 30., y: app.config.gap_px as f32 + h / 2. }).unwrap();
        assert!(app.settings.open);
    }

    #[test]
    fn picking_a_city_sets_the_weather() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.open_settings();
        app.settings.view.city_results = vec![("Lisboa, Portugal".into(), 38.7, -9.1)];
        app.settings_action(SettingsAction::PickCity(0));
        assert_eq!(app.config.weather.label, "Lisboa, Portugal");
        assert_eq!((app.config.weather.latitude, app.config.weather.longitude), (38.7, -9.1));
        assert!(app.settings.view.city_results.is_empty());
    }

    #[test]
    fn the_search_answer_lists_places_or_says_why_not() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.open_settings();
        app.apply_places(Ok(vec![("Bom Jesus, São Paulo, Brasil".into(), -23.5, -46.7)]));
        assert_eq!(app.settings.view.city_results.len(), 1);
        app.apply_places(Ok(vec![]));
        assert_eq!(app.settings.view.status.as_deref(), Some("Nenhuma cidade com esse nome"));
        app.apply_places(Err("offline".into()));
        assert!(app.settings.view.city_results.is_empty());
        assert_eq!(app.settings.view.status.as_deref(), Some("Sem conexão para buscar a cidade"));
        let before = app.config.weather.clone();
        assert_eq!(app.config.weather, before, "nothing changes");
    }

    fn notice_from(app_id: &str, id: i64) -> isle_core::notifications::Notification {
        isle_core::notifications::Notification {
            id,
            arrival_ms: id,
            app_id: app_id.into(),
            app_name: "WhatsApp".into(),
            title: "Oi".into(),
            ..Default::default()
        }
    }

    #[test]
    fn banners_off_counts_but_shows_no_banner() {
        let mut cfg = IsleConfig::default();
        cfg.banners = false;
        let mut app = IsleApp::new(cfg);
        app.apply_notifications(vec![notice_from("WhatsApp", 1)], false);
        assert_eq!(app.islands[0].mode, PillMode::CompactBar);
        assert_eq!(app.state.notifications.unread_count(), 1);
    }

    #[test]
    fn banner_secs_sets_how_long_a_notification_shows() {
        let mut cfg = IsleConfig::default();
        cfg.banner_secs = 15;
        let mut app = IsleApp::new(cfg);
        let before = Instant::now();
        app.apply_notifications(vec![notice_from("WhatsApp", 1)], false);
        let left = app.banner_until.unwrap() - before;
        assert!(left > Duration::from_secs(14) && left <= Duration::from_secs(16), "{left:?}");
    }

    #[test]
    fn notifications_per_page_pages_the_list() {
        let mut cfg = IsleConfig::default();
        cfg.notifications_per_page = 2;
        let mut app = IsleApp::new(cfg);
        app.apply_notifications((1..=5).map(|i| notice_from("WhatsApp", i)).collect(), true);
        assert_eq!(app.state.notifications.last_page(), 2);
    }

    #[test]
    fn muted_apps_never_reach_the_island() {
        let mut cfg = IsleConfig::default();
        cfg.muted_apps = vec!["whatsapp".into()];
        let mut app = IsleApp::new(cfg);
        app.apply_notifications(vec![notice_from("WhatsApp", 1), notice_from("Outlook", 2)], false);
        let apps: Vec<_> = app.state.notifications.ordered().iter().map(|n| n.app_id.clone()).collect();
        assert_eq!(apps, vec!["Outlook".to_string()]);
    }

    #[test]
    fn wheel_volume_off_leaves_the_wheel_alone() {
        let mut cfg = IsleConfig::default();
        cfg.wheel_volume = false;
        let mut app = IsleApp::new(cfg);
        let (tx, rx) = std::sync::mpsc::channel();
        app.audio_tx = Some(tx);
        app.apply_input(UiInput::Wheel { window: 0, delta: -120, x: 0., y: 0. }).unwrap();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn lyrics_off_asks_for_none() {
        let mut cfg = IsleConfig::default();
        cfg.lyrics = false;
        let mut app = IsleApp::new(cfg);
        let song = isle_platform_win::media::MediaStatus { title: "Song".into(), playing: true, duration_ms: 1000, ..Default::default() };
        assert!(app.apply_media(Some(song)).is_none());
        assert_eq!(app.state.media.as_ref().map(|m| m.title.as_str()), Some("Song"));
    }

    #[test]
    fn quota_alerts_off_shows_no_banner() {
        use isle_core::state::{UsageProvider, UsageWindow};
        let mut cfg = IsleConfig::default();
        cfg.quota_alerts = false;
        let mut app = IsleApp::new(cfg);
        let claude = UsageProvider {
            id: "claude".into(),
            name: "Claude Code".into(),
            windows: vec![UsageWindow { label: "5 horas".into(), percent: 83.0, resets_at_ms: 2_000_000 }],
            ..UsageProvider::default()
        };
        app.apply_usage(vec![claude], 1_000);
        assert_eq!(app.state.claude_quota.used_pct, 83.0, "the reading still counts");
        assert_eq!(app.islands[0].mode, PillMode::CompactBar);
    }

    #[test]
    fn restore_asks_first() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.open_settings();
        app.settings_action(SettingsAction::Toggle("clock_seconds"));
        app.settings_action(SettingsAction::Run(isle_core::prefs::ActionId::Restore));
        assert!(app.config.clock_seconds, "only asked");
        assert_eq!(app.settings.view.confirm, Some(isle_core::prefs::ActionId::Restore));
        app.settings_action(SettingsAction::Run(isle_core::prefs::ActionId::Restore));
        assert!(!app.config.clock_seconds);
        assert_eq!(app.settings.view.confirm, None);
    }

    #[test]
    fn a_backup_brings_back_settings_and_reminders() {
        let dir = std::env::temp_dir().join(format!("cimori-backup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("backup.json");
        let mut cfg = IsleConfig::default();
        cfg.clock_seconds = true;
        cfg.bar_items = vec![BarItem::Weather, BarItem::Clock];
        let mut app = IsleApp::new(cfg.clone());
        app.reminders.add("ligar para a mãe", 5_000);
        app.export_backup(&file).unwrap();

        let mut other = IsleApp::new(IsleConfig::default());
        other.import_backup(&file).unwrap();
        assert!(other.config.clock_seconds);
        assert_eq!(other.state.bar, vec![BarItem::Weather, BarItem::Clock]);
        assert_eq!(other.reminders.items().len(), 1);
        assert!(other.import_backup(&dir.join("missing.json")).is_err());
        std::fs::write(dir.join("bad.json"), "{nope").unwrap();
        assert!(other.import_backup(&dir.join("bad.json")).is_err(), "not a backup: nothing changes");
        assert!(other.config.clock_seconds);
    }

    #[test]
    fn holidays_turned_off_leave_the_agenda_at_once() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.state.today = isle_core::agenda::days_from_civil(2026, 11, 10);
        app.load_holidays(r#"[{"date":"2026-11-15","localName":"Proclamação da República"}]"#);
        assert!(app.state.agenda.iter().any(|i| i.holiday));
        app.open_settings();
        app.settings_action(SettingsAction::Toggle("holidays"));
        assert!(!app.state.agenda.iter().any(|i| i.holiday));
    }

    #[test]
    fn clearing_the_calendar_address_drops_its_events() {
        let mut cfg = IsleConfig::default();
        cfg.calendar_ics_url = Some("https://example.com/a.ics".into());
        let mut app = IsleApp::new(cfg);
        app.state.today = isle_core::agenda::days_from_civil(2026, 11, 10);
        app.load_ics(concat!("BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Deploy\r\n", "DTSTART:20261110T220000Z\r\nEND:VEVENT\r\nEND:VCALENDAR"));
        assert!(app.state.agenda.iter().any(|i| i.title == "Deploy"));
        app.open_settings();
        app.settings_action(SettingsAction::Edit("calendar_ics_url"));
        app.settings.view.editing = Some(("calendar_ics_url", String::new()));
        app.settings_input(SettingsInput::Char('\r'));
        assert_eq!(app.config.calendar_ics_url, None);
        assert!(!app.state.agenda.iter().any(|i| i.title == "Deploy"));
    }

    #[test]
    fn a_module_turned_off_stops_its_service() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.started = vec!["weather", "agenda", "usage", "media"];
        app.open_settings();
        for id in ["modules.weather", "modules.agenda", "modules.ai"] {
            app.settings_action(SettingsAction::Toggle(id));
        }
        assert_eq!(app.started, vec!["media"]);
    }

    #[test]
    fn settings_read_only_at_start_say_so() {
        for id in ["notification_icons", "hook_port"] {
            assert!(isle_core::prefs::by_id(id).unwrap().restart, "{id}");
        }
    }

    #[test]
    fn a_space_reaches_the_search() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.open_settings();
        for c in "modo foco".chars() {
            app.settings_input(SettingsInput::Char(c));
        }
        assert_eq!(app.settings.view.search, "modo foco");
    }

    #[test]
    fn losing_the_mouse_mid_drag_saves_and_lets_go() {
        let (mut app, path) = app_saving_to("capture");
        app.open_settings();
        app.settings_action(SettingsAction::Tab(SettingsTab::Notifications));
        let b = hit_where(&mut app, |a| matches!(a, SettingsAction::Slide("banner_secs", _)));
        let y = b.y + b.h / 2.;
        app.settings_input(SettingsInput::Down { x: b.x + b.w, y });
        app.settings_input(SettingsInput::CaptureLost);
        assert!(std::fs::read_to_string(&path).unwrap().contains("\"banner_secs\": 30"));
        app.settings_input(SettingsInput::Move { x: b.x, y });
        assert_eq!(app.config.banner_secs, 30, "the slider no longer follows the mouse");
    }

    #[test]
    fn the_monitors_follow_the_settings() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.open_settings();
        app.settings_action(SettingsAction::Toggle("all_monitors"));
        assert!(!app.overlay.all_monitors);
        app.settings_action(SettingsAction::Monitor(r"\\.\DISPLAY6".into()));
        assert_eq!(app.overlay.monitors, vec![r"\\.\DISPLAY1".to_string(), r"\\.\DISPLAY6".to_string()]);
        app.settings_action(SettingsAction::Monitor(r"\\.\DISPLAY1".into()));
        app.settings_action(SettingsAction::Monitor(r"\\.\DISPLAY6".into()));
        assert_eq!(app.overlay.monitors, vec![r"\\.\DISPLAY6".to_string()], "the last one stays");
    }
}
