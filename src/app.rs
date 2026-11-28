// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Adapted for isle-win from Mac Island & Dock by caliberda v3.1.0 (Island::OnVolumeChanged/ShowStatusNotice):
// worker events, per-island input, volume OSD and Island::ShowToasts notifications.
// Tray events and pinned/overflow interaction adapted from Mac Island & Dock v3.1.1.

use anyhow::Result;
#[path = "ai_chat.rs"]
mod ai_chat;
#[path = "settings.rs"]
mod settings;
#[path = "app_icons.rs"]
mod icon_packs;
#[path = "phase8.rs"]
mod phase8;
#[cfg(test)]
#[path = "app_phase9_tests.rs"]
mod phase9_tests;
#[path = "app_shortcuts.rs"]
mod shortcuts;
#[path = "app_release.rs"]
mod release;
use isle_ai::hooks::{HookEvent, HookServer};
use isle_core::agenda::{AgendaItem, IcsEvent, civil_from_days, days_from_civil, expand, month_len, parse_holidays, parse_ics,
};
use isle_core::config::IsleConfig;
use isle_core::i18n::{tr, trf, trn};
use isle_core::pomodoro::{Phase, Pomodoro, PomodoroConfig};
use isle_core::spring::PillGeometry;
use isle_core::state::MediaInfo;
use isle_core::state::{
    IslandState, PermissionCard, PermissionDecision, PillMode, QuotaAlertEvent, TabPage,
};
use isle_platform_win::audio_osd::{self, AudioCommand, AudioEvent, AudioStatus};
use isle_platform_win::lyrics::{self, Lyrics, LyricsStatus, TrackQuery};
use isle_platform_win::media::{self, MediaCommand, MediaStatus};
use isle_platform_win::notifications::{self, NotificationEvent};
use isle_platform_win::system_controls::{self, ControlCommand, ControlEvent};
use isle_platform_win::tray::{self, TrayCommand, TrayService, TraySnapshot};
use isle_render::pages::{MediaKey, compute_hitboxes, slider_percent};
use isle_render::renderer::{CARD_WIDTH, card_height, command_fits};
use isle_render::window::{
    CANVAS_MAX_WIDTH, HitArea, MultiMonitorOverlay, UiAction, UiInput, Waker, hit_test_action, init_dpi,
};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::oneshot;

/// The mouse away this long closes an open panel, then hides the pill to its line (auto-hide).
#[cfg(test)]
const LEAVE_AFTER: Duration = Duration::from_millis(1500);
/// How long a banner shows, unless the mouse is on it.
const BANNER_FOR: Duration = Duration::from_secs(6);
/// (width, height, radius) the spring heads to.
const COMPACT: (f32, f32, f32) = (220.0, 34.0, 17.0);
const LINE: (f32, f32, f32) = (120.0, 5.0, 2.5);
const EXPANDED: (f32, f32, f32) = (560.0, 450.0, 28.0);
/// An animation frame (60 a second).
const FRAME: Duration = Duration::from_micros(16_667);
/// São Paulo, UTC-3 (no daylight saving since 2019): the zone when Windows' is not used.
const FALLBACK_OFFSET: i64 = -3 * 3600;
static SYSTEM_ZONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Seconds east of UTC now: Windows' zone (the config's `timezone_system`), else São Paulo's.
/// Tests always run on São Paulo's so they do not depend on the PC.
pub(crate) fn local_offset() -> i64 {
    if cfg!(test) || !SYSTEM_ZONE.load(std::sync::atomic::Ordering::Relaxed) { return FALLBACK_OFFSET; }
    isle_platform_win::locale::utc_offset_secs().unwrap_or(FALLBACK_OFFSET)
}

/// What each island does on its own (one per monitor; 0 is the main one): its spring, what it shows,
/// whether the mouse is on it, and when it closes or hides once the mouse left.
pub struct Presence {
    pinned: bool,
    pub geom: PillGeometry,
    pub mode: PillMode,
    hovered: bool,
    scroll:f32,
    leave_at: Option<Instant>,
    enter_at: Option<Instant>,
    line_since: Option<Instant>,
    line_hidden: bool,
    manually_hidden: bool,
}

impl Presence {
    fn new() -> Self {
        Self {
            geom: PillGeometry::default(),
            pinned: false,
            mode: PillMode::CompactBar,
            hovered: false,
            scroll:0.,
            leave_at: None,
            enter_at: None,
            line_since: None,
            line_hidden: false,
            manually_hidden: false,
        }
    }

    fn morph(&mut self, (w, h, r): (f32, f32, f32)) {
        self.geom.morph_to(w, h, r);
        if !isle_render::window::animations_enabled() {
            self.geom.width.value = w;
            self.geom.height.value = h;
            self.geom.corner_radius.value = r;
            self.geom.width.velocity = 0.;
            self.geom.height.velocity = 0.;
            self.geom.corner_radius.velocity = 0.;
        }
    }

    fn hover(&mut self, inside: bool, options: isle_core::config::PillOptions) {
        self.hovered = inside;
        self.leave_at = None;
        self.enter_at = None;
        if inside && self.mode == PillMode::Minimized && !options.line_click {
            self.manually_hidden = false;
            self.mode = PillMode::CompactBar;
            self.morph(COMPACT);
        }
        // "Abrir painel ao passar o mouse" alone decides, the notch too: off, the mouse shows the
        // closed island and a click on its items opens their tab.
        if inside && options.hover_open && self.mode == PillMode::CompactBar {
            self.enter_at = Some(Instant::now() + Duration::from_millis(250));
        }
    }

    /// The mouse away: an open panel closes, then (auto-hide) the pill hides to its line.
    fn leave(&mut self, now: Instant, auto_hide: bool, options: isle_core::config::PillOptions) {
        if self.pinned && options.arrow_pins { self.leave_at = None; return; }
        let leaves = !self.hovered
            && match self.mode {
                PillMode::Expanded(tab) => tab != TabPage::Tray || !options.tray_keep_open,
                PillMode::CompactBar => auto_hide,
                _ => false,
            };
        if !leaves {
            self.leave_at = None;
            return;
        }
        match self.leave_at {
            None => self.leave_at = Some(now + Duration::from_millis(u64::from(options.hide_delay_ms))),
            Some(at) if now >= at => {
                self.leave_at = None;
                if matches!(self.mode, PillMode::Expanded(_)) {
                    self.mode = PillMode::CompactBar;
                    self.morph(COMPACT);
                } else {
                    self.mode = PillMode::Minimized;
                    self.morph(LINE);
                }
            }
            _ => {}
        }
    }

    /// A click on this island's pill: open or close it, or switch its tab.
    fn toggle(&mut self, tab: Option<TabPage>) {
        self.scroll=0.;
        self.enter_at = None;
        match (tab, &self.mode) {
            (Some(tab), _) => self.mode = PillMode::Expanded(tab),
            (None, PillMode::CompactBar | PillMode::AlertBanner { .. }) => {
                self.mode = PillMode::Expanded(TabPage::AiHub)
            }
            (None, _) => {
                self.mode = PillMode::CompactBar;
                self.morph(COMPACT);
                return;
            }
        }
        self.morph(match self.mode {
            PillMode::Expanded(tab) => isle_render::layout::expanded_size(tab),
            _ => EXPANDED,
        });
    }
}

struct VolumeDrag {
    window: usize,
    left: f32,
    width: f32,
    /// None is the master; app commands keep the executable key across list refreshes.
    app_id: Option<String>,
}

/// Asks waiting at once, the card included, at most (Port of isle's MAX_WAITING, agents/mod.rs).
const MAX_WAITING: usize = 8;
/// How long a freshly shown permission card ignores its buttons (a double-click on the one before).
const PERMISSION_CLICK_GUARD: Duration = Duration::from_millis(400);

pub struct IsleApp {
    icons_work: icon_packs::Work,
    pub config: IsleConfig,
    pub islands: Vec<Presence>,
    pub state: IslandState,
    pub overlay: MultiMonitorOverlay,
    tray_service: Option<TrayService>,
    settings: settings::Settings,
    release: release::Release,
    phase8: phase8::Phase8,
    follow_monitor_at: Option<Instant>,
    /// Where module services start (set in `run`); and the ones already started.
    services: Option<Services>,
    /// The weather's refresh loop, made again when the city changes.
    weather_task: Option<tokio::task::JoinHandle<()>>,
    /// The holidays' and the calendar's fetches; the AI limits' loop.
    agenda_tasks: Vec<tokio::task::JoinHandle<()>>,
    usage_task: Option<tokio::task::JoinHandle<()>>,
    started: Vec<&'static str>,
    tray_panel_open: bool,
    pub pomodoro: Pomodoro,
    ics: Vec<IcsEvent>,
    /// The calendar as fetched, and the offset its times were read with: read again when the zone changes.
    ics_raw: (String, i64),
    holidays: Vec<AgendaItem>,
    pending_reply: Option<oneshot::Sender<PermissionDecision>>,
    /// When the card on screen showed: a click this soon was meant for the card before it.
    permission_shown: Instant,
    /// Asks waiting behind the card on screen, oldest first (Port of isle's `Asks.waiting`, agents/mod.rs).
    permission_queue: std::collections::VecDeque<(PermissionCard, oneshot::Sender<PermissionDecision>)>,
    /// The playing track's lyrics: which track they are for, and their timed lines (empty: none).
    lyrics_key: Option<String>,
    lyrics: Vec<isle_core::lrc::LyricLine>,
    banner_until: Option<Instant>,
    /// The island files are dragged over: it stays as it is until they leave or are let go.
    files_over: Option<usize>,
    notification_banner: Option<(isle_core::notifications::Notification, PillMode)>,
    /// Where "Limpar tudo"'s watermark is kept between runs (none in tests).
    pub cleared_path: Option<std::path::PathBuf>,
    /// settings.json, saved again when a setting changes from the island (none in tests).
    pub settings_path: Option<std::path::PathBuf>,
    /// The Timer tab's countdown and stopwatch (the Pomodoro is `pomodoro`).
    countdown: isle_core::timers::Countdown,
    stopwatch: isle_core::timers::Stopwatch,
    /// The next agenda event's countdown (refreshed each minute), shown when no timer runs.
    agenda_countdown: Option<String>,
    /// Reminders and timer ends waiting for the main island: shown one at a time (title, text,
    /// and the tab a click on it opens).
    alerts: std::collections::VecDeque<(String, String, Option<Opens>)>,
    /// The alert banner showing now and the tab its click opens.
    alert_banner: Option<(PillMode, Opens)>,
    /// Manual reminders (isle's, kept until shown) and what's typed for a new one.
    reminders: isle_core::reminders::Reminders,
    reminder_draft: isle_core::notifications::Reply,
    reminder_typing: bool,
    reminder_minutes: u32,
    /// Agenda reminders already given (see agenda::reminder_key), and where both are kept.
    agenda_given: std::collections::HashSet<String>,
    reminder_files: Option<(std::path::PathBuf, std::path::PathBuf)>,
    audio_tx: Option<Sender<AudioCommand>>,
    controls_tx: Option<Sender<ControlCommand>>,
    control_panel_open: bool,
    volume_drag: Option<VolumeDrag>,
    /// A control held in the control center's edit mode: (island, its index).
    tile_drag: Option<(usize, usize)>,
    /// A press on a shelf file: its index and where, until it moves past the drag threshold (or is let go).
    shelf_press: Option<(usize, f32, f32)>,
    /// The island where a reply is being typed (its panel stays open meanwhile).
    reply_window: usize,
    chat_window: Option<usize>,
    /// The page the ✦ was pressed on: the assistant's back arrow returns to it.
    ai_return: Option<TabPage>,
    /// The models each provider's CLI listed (asked once a run, when the chat or the settings open).
    ai_models: Vec<(isle_core::state::AiProviderKind, Vec<isle_core::chat::ModelChoice>)>,
    ai_models_asked: Vec<isle_core::state::AiProviderKind>,
    chat_tx: Option<tokio::sync::mpsc::UnboundedSender<isle_ai::assistant::Command>>,
}

#[cfg(test)]
thread_local! {
    /// How many alert sounds would have played (tests stay silent).
    pub(crate) static BEEPS: std::cell::Cell<u32> = Default::default();
}

#[cfg(test)]
thread_local! {
    /// What the power button would have asked Windows for (tests never run it).
    pub(crate) static POWER_RAN: std::cell::RefCell<Vec<isle_core::power::PowerAction>> = Default::default();
}

/// When a reminder is due, as the list says it: "às 17:30", or "amanhã às 08:00" past midnight.
/// The system's drag threshold (SM_CXDRAG / SM_CYDRAG), in logical pixels' stand-in: 4 off Windows.
fn drag_threshold() -> (f32, f32) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXDRAG, SM_CYDRAG};
        // SAFETY: plain metric query.
        return unsafe { (GetSystemMetrics(SM_CXDRAG).max(2) as f32, GetSystemMetrics(SM_CYDRAG).max(2) as f32) };
    }
    #[cfg(not(windows))]
    (4., 4.)
}

fn when_text(due_ms: i64, now_ms: i64) -> String {
    let local = |ms: i64| ms.div_euclid(1000) + local_offset();
    let (due, now) = (local(due_ms), local(now_ms));
    let minute = due.rem_euclid(86_400) / 60;
    let time = format!("{:02}:{:02}", minute / 60, minute % 60);
    trf(if due.div_euclid(86_400) > now.div_euclid(86_400) { "reminder.tomorrow_at" } else { "reminder.at" }, &[("time", &time)])
}

/// "Claude Code concluiu · isle-win": the AI's name, what it does, and the project when known.
fn quota_name(provider: isle_core::state::AiProviderKind) -> &'static str {
    match provider {
        isle_core::state::AiProviderKind::ClaudeCode => "Claude Code",
        isle_core::state::AiProviderKind::Codex => "Codex",
        isle_core::state::AiProviderKind::OpenCodeGo => "OpenCode Go",
    }
}

/// Whether the AI agent `id` ("claude", "codex") is switched on in IA › Agentes; other providers always are.
fn agent_on(c: &IsleConfig, id: &str) -> bool {
    match id {
        "claude" => c.ai_claude_enabled,
        "codex" => c.ai_codex_enabled,
        _ => true,
    }
}

/// `doing`: a key of locales/ with the `{name}` ("agent.done"), or "" for the name alone.
fn titled(provider: &str, doing: &str, project: &str) -> String {
    let name = match provider {
        "claude" => "Claude Code",
        "codex" => "Codex",
        other => other,
    };
    let head = if doing.is_empty() { name.to_owned() } else { trf(doing, &[("name", &name)]) };
    if project.is_empty() {
        head
    } else {
        format!("{head} · {project}")
    }
}

impl IsleApp {
    pub fn new(config: IsleConfig) -> Self {
        let config = config.sanitized();
        // The tests keep the texts they were written against (pt-BR), whatever this PC's language.
        if !cfg!(test) { isle_platform_win::locale::apply_language(&config.language); }
        SYSTEM_ZONE.store(config.timezone_system, std::sync::atomic::Ordering::Relaxed);
        let fahrenheit = config.weather_fahrenheit;
        let (bar, bar_quota_strip, modules) = (config.bar_shown(), config.bar_quota_strip, config.modules);
        let per_page = config.notifications_per_page as usize;
        let ui = config.ui;
        let power = config.power;
        let (control_tiles, control_mixer, control_outputs) = (config.control_tiles.clone(), config.control_mixer, config.control_outputs);
        isle_render::set_accent(config.accent_rgb);
        isle_render::window::set_animation_level(config.animations);
        let config_timer_ms = config.timer_default_secs.max(60) as i64 * 1000;
        let mut overlay = MultiMonitorOverlay::new(config.monitors.clone(), config.all_monitors, config.main_monitor.clone(),
        ); overlay.over_fullscreen = config.over_fullscreen;
        overlay.hide_maximized = config.hide_maximized;
        overlay.hide_apps = config.hide_apps.clone();
        overlay.follow_mouse = config.ui.follow_mouse;
        let pomodoro = Pomodoro::new(PomodoroConfig {
            focus_minutes: config.pomodoro_focus_secs / 60,
            short_break_minutes: config.pomodoro_break_secs / 60,
            ..PomodoroConfig::default()
        });
        Self {
            config,
            islands: vec![Presence::new()],
            state: {
                let mut state = IslandState { ui, fahrenheit, bar, bar_quota_strip, modules, control_tiles, control_mixer, control_outputs, ..IslandState::default() };
                state.notifications.per_page = per_page;
                state.power = power;
                state.hibernate_allowed = !cfg!(test) && isle_platform_win::power::hibernate_allowed();
                state
            },
            overlay,
            tray_service: None,
            settings: Default::default(),
            release: Default::default(),
            phase8: Default::default(),
            follow_monitor_at: None,
            services: None,
            icons_work: Default::default(),
            weather_task: None,
            agenda_tasks: Vec::new(),
            usage_task: None,
            started: Vec::new(),
            tray_panel_open: false,
            pomodoro,
            ics: Vec::new(),
            ics_raw: (String::new(), 0),
            holidays: Vec::new(),
            pending_reply: None,
            permission_shown: Instant::now(),
            permission_queue: Default::default(),
            lyrics_key: None,
            lyrics: Vec::new(),
            banner_until: None,
            files_over: None,
            notification_banner: None,
            cleared_path: None,
            settings_path: None,
            countdown: isle_core::timers::Countdown::new(config_timer_ms),
            stopwatch: Default::default(),
            agenda_countdown: None,
            alerts: Default::default(),
            alert_banner: None,
            reminders: Default::default(),
            reminder_draft: Default::default(),
            reminder_typing: false,
            reminder_minutes: 30,
            agenda_given: Default::default(),
            reminder_files: None,
            audio_tx: None,
            controls_tx: None,
            control_panel_open: false,
            volume_drag: None,
            tile_drag: None,
            shelf_press: None,
            reply_window: 0,
            chat_window: None,
            ai_return: None,
            ai_models: Vec::new(),
            ai_models_asked: Vec::new(),
            chat_tx: None,
        }
    }

    pub fn load_ics(&mut self, raw: &str) {
        self.ics_raw = (raw.to_owned(), local_offset());
        self.ics = parse_ics(raw, self.ics_raw.1);
        self.rebuild_agenda();
    }

    /// One year's holidays (Nager.Date) replace that year's earlier ones.
    pub fn load_holidays(&mut self, json: &str) {
        let fresh = parse_holidays(json);
        let years: Vec<i64> = fresh.iter().map(|h| civil_from_days(h.day).0).collect();
        self.holidays.retain(|h| !years.contains(&civil_from_days(h.day).0));
        self.holidays.extend(fresh);
        self.rebuild_agenda();
    }

    /// The occurrences from a month back to a year ahead: room for paging months and "next".
    fn rebuild_agenda(&mut self) {
        let today = self.state.today;
        let (from, to) = (today - 40, today + 400);
        let mut items = expand(&self.ics, from, to);
        items.extend(self.holidays.iter().filter(|h| self.config.holidays && (from..to).contains(&h.day)).cloned(),
        );
        items.sort_by_key(|i| (i.day, i.minute.map_or(-1, |m| m as i64), !i.holiday));
        self.state.agenda = items;
    }

    pub fn tick_runtime_domain(&mut self, now_unix: u64) {
        // Windows' time zone changed: the calendar's times are read again in the new one.
        if !self.ics_raw.0.is_empty() && self.ics_raw.1 != local_offset() {
            let raw = std::mem::take(&mut self.ics_raw.0);
            self.load_ics(&raw);
        }
        let today = (now_unix as i64 + local_offset()).div_euclid(86_400);
        if today != self.state.today {
            if self.state.agenda_day == self.state.today { self.state.agenda_day = today; }
            self.state.today = today;
            self.rebuild_agenda();
        }
        self.state.now_ms = now_unix as i64 * 1000;
        let local = now_unix as i64 + local_offset();
        self.state.clock_text = isle_core::clock::clock_text(local, &self.config);
        self.state.date_text = isle_core::clock::date_text(local, &self.config);
        self.state.utc_offset_s = local_offset();
        // Only what comes within 12 hours, as Mac Island & Dock; otherwise the bar shows the date.
        let local = now_unix as i64 + local_offset();
        self.agenda_countdown = self.state.agenda.iter()
            .filter_map(|i| Some((i, i.day * 86_400 + i.minute? as i64 * 60 - local)))
            .filter(|_| self.config.countdown_events)
            .find(|(_, rem)| (1..=self.config.countdown_hours as i64 * 3600).contains(rem))
            .map(|(ev, rem)| match (rem + 59) / 60 {
                mins if mins < 60 => format!("{} ({mins}m)", ev.title),
                mins => format!("{} ({}h)", ev.title, mins / 60),
            });
        self.remind_agenda(local / 60);
        self.refresh_timers(now_unix as i64 * 1000);
    }

    /// The agenda's reminders due at `now` (local minutes since 1970): each once, on time or up to
    /// 12 hours late (the mod's rules); five at most at once, then "e mais N".
    fn remind_agenda(&mut self, now: i64) {
        use isle_core::agenda::{ReminderDue, due_reminders, reminder_key};
        if !self.config.agenda_reminders { return; }
        let due: Vec<_> = due_reminders(&self.state.agenda, now, self.config.agenda_reminder_minutes as i64, &self.agenda_given,
        )
            .into_iter().map(|(i, d)| (reminder_key(i), i.title.clone(), i.minute.unwrap_or(0), d)).collect();
        if due.is_empty() { return; }
        let mut shown = 0;
        for (key, title, minute, verdict) in &due {
            self.agenda_given.insert(key.clone());
            if *verdict == ReminderDue::TooOld { continue; }
            shown += 1;
            if shown > 5 { continue; }
            let time = format!("{:02}:{:02}", minute / 60, minute % 60);
            let key = if *verdict == ReminderDue::Missed { "agenda.was_at" }
                else if self.config.agenda_reminder_minutes > 0 { "agenda.at" }
                else { "agenda.now" };
            self.alert(tr("tab.agenda"), &trf(key, &[("title", title), ("time", &time)]), Some(TabPage::CalendarWeather));
        }
        if shown > 5 { self.alert(tr("tab.agenda"), &trn("agenda.more", (shown - 5) as i64, &[]), Some(TabPage::CalendarWeather),
            ); }
        // Only the last two days' keys are kept.
        let today = now.div_euclid(1440);
        self.agenda_given.retain(|k| {
            k.split('|').next().and_then(|d| d.parse::<i64>().ok()).is_some_and(|d| d >= today - 1)
        });
        self.save_reminders();
    }

    /// Where reminders are kept (lembretes.json, agenda-avisados.json): read now, written on change.
    pub fn set_reminder_files(&mut self, reminders: std::path::PathBuf, given: std::path::PathBuf) {
        if let Ok(json) = std::fs::read_to_string(&reminders) {
            match serde_json::from_str(&json) { Ok(r) => self.reminders = r, Err(err) => tracing::warn!("lembretes: {err:#}"),
            }
        }
        if let Ok(json) = std::fs::read_to_string(&given) {
            self.agenda_given = serde_json::from_str(&json).unwrap_or_default();
        }
        self.reminder_files = Some((reminders, given));
    }

    fn save_reminders(&self) {
        let Some((reminders, given)) = &self.reminder_files else { return;
        };
        for (path, json) in [(reminders, serde_json::to_string(&self.reminders)), (given, serde_json::to_string(&self.agenda_given)),
        ] {
            match json {
                Ok(json) => {
                    if let Err(err) = std::fs::write(path, json) { tracing::warn!("lembretes: {err:#}"); }
                }
                Err(err) => tracing::warn!("lembretes: {err:#}"),
            }
        }
    }

    /// The Pomodoro, countdown and stopwatch at `now_ms`: their ends become alerts, the Timer tab
    /// and the compact bar get their numbers (the bar: what ends first, else the stopwatch, else
    /// the next agenda event).
    pub fn refresh_timers(&mut self, now_ms: i64) {
        use isle_core::timers::clock;
        if let Some(t) = self.pomodoro.tick(now_ms) {
            let text = tr(if t.finished == Phase::Focus { "focus.focus_done" } else { "focus.break_done" });
            self.alert(tr("focus.pomodoro"), text, Some(TabPage::Timer));
        }
        if self.countdown.tick(now_ms) {
            self.alert(tr("focus.timer"), tr("focus.time_up"), Some(TabPage::Timer));
        }
        // isle's reminders are kept until shown: one due while the island was closed shows now.
        let due: Vec<_> = self.reminders.items().iter().filter(|r| r.due_ms <= now_ms).cloned().collect();
        for r in &due {
            self.reminders.dismiss(r.id);
            self.alert(tr("focus.reminder"), &r.text, Some(TabPage::Timer));
        }
        if !due.is_empty() { self.save_reminders(); }
        let reminders = self.reminders.items().iter().map(|r| isle_core::timers::ReminderLine {
            id: r.id, text: r.text.clone(), when: when_text(r.due_ms, now_ms),
        }).collect();
        let p = self.pomodoro.view(now_ms);
        let mode = self.state.timers.mode;
        self.state.timers = isle_core::timers::TimersView {
            mode,
            pomodoro_phase: p.phase,
            pomodoro_running: p.running,
            pomodoro_remaining_ms: p.remaining_ms,
            pomodoro_duration_ms: p.duration_ms,
            focus_minutes: self.config.pomodoro_focus_secs / 60,
            break_minutes: self.config.pomodoro_break_secs / 60,
            countdown_running: self.countdown.running(),
            countdown_started: self.countdown.started(),
            countdown_remaining_ms: self.countdown.remaining(now_ms),
            countdown_duration_ms: self.countdown.duration_ms,
            stopwatch_running: self.stopwatch.running(),
            stopwatch_ms: self.stopwatch.elapsed(now_ms),
            reminders,
            reminder_text: self.reminder_draft.text.clone(),
            reminder_typing: self.reminder_typing,
            reminder_minutes: self.reminder_minutes,
        };
        // (ends at, text, shown in the Next slot?)
        let mut ending = Vec::new();
        if p.phase != Phase::Idle {
            let name = tr(if p.phase == Phase::Focus { "focus.focus" } else { "focus.break" });
            ending.push((p.ends_at_ms.unwrap_or(i64::MAX), format!("{name} {}", clock(p.remaining_ms, true)), self.config.countdown_pomodoro));
        }
        if self.countdown.started() {
            ending.push((self.countdown.deadline().unwrap_or(i64::MAX), format!("{} {}", tr("focus.timer"), clock(self.countdown.remaining(now_ms), true)), self.config.countdown_timer));
        }
        let elapsed = self.stopwatch.elapsed(now_ms);
        let stopwatch = (elapsed > 0).then(|| format!("{} {}", tr("focus.stopwatch"), clock(elapsed, false)));
        let first = |next_only: bool| ending.iter().filter(|e| !next_only || e.2).min_by_key(|e| e.0).map(|e| e.1.clone());
        self.state.timer_text = first(false).or_else(|| stopwatch.clone());
        let stopwatch = stopwatch.filter(|_| self.config.countdown_timer);
        self.state.next_countdown_text = first(true).or(stopwatch).or_else(|| self.agenda_countdown.clone());
    }

    /// Something the main island should say (a reminder, a timer's end): queued, shown when it can.
    fn alert(&mut self, title: &str, text: &str, opens: Option<TabPage>) {
        self.alerts.push_back((title.to_owned(), text.to_owned(), opens.map(Opens::Tab)));
    }

    /// The next queued alert as a banner, when the main island is free: compact or hidden (no
    /// panel open, no other banner), no permission card, no Nao Perturbe.
    fn show_alert(&mut self, now: Instant) {
        if self.alerts.is_empty() || self.state.dnd_active || self.pending_reply.is_some()
            || !matches!(self.islands[0].mode, PillMode::CompactBar | PillMode::Minimized) { return; }
        let Some((title, subtitle, opens)) = self.alerts.pop_front() else { return;
        };
        // The mod's reminder/timer sound (MessageBeep), when the banner shows (so not during Nao Perturbe).
        if self.config.alert_sound {
            #[cfg(test)]
            BEEPS.with(|b| b.set(b.get() + 1));
            #[cfg(not(test))]
            isle_platform_win::sound::alert_beep();
        }
        self.islands[0].mode = PillMode::AlertBanner { title, subtitle };
        self.alert_banner = opens.map(|tab| (self.islands[0].mode.clone(), tab));
        let size = self.notification_banner_size();
        self.islands[0].morph(size);
        self.notification_banner = None;
        self.banner_until = Some(now + BANNER_FOR);
    }

    fn running_timer(&self) -> bool {
        self.pomodoro.deadline().is_some() || self.countdown.running() || self.stopwatch.running()
    }

    /// settings.json again, after a setting changed from the island.
    /// What changed in the settings (from `old` to `self.config`) reaches the island now;
    /// `save` writes settings.json (not while a slider is dragged).
    pub(crate) fn apply_config(&mut self, old: &IsleConfig, save: bool) {
        self.config = std::mem::take(&mut self.config).sanitized();
        let c = self.config.clone();
        if old.language != c.language {
            // Every text is looked up as it is drawn: the island and the settings change on the next frame.
            isle_platform_win::locale::apply_language(&c.language);
            self.refresh_settings_view();
        }
        if old.icon_pack != c.icon_pack || old.icon_folder != c.icon_folder { self.start_icons(); }
        if !c.ui.arrow_pins || !c.auto_hide { for island in &mut self.islands { island.pinned = false; } }
        isle_render::set_accent(c.accent_rgb);
        SYSTEM_ZONE.store(c.timezone_system, std::sync::atomic::Ordering::Relaxed);
        isle_render::window::set_animation_level(c.animations);
        self.state.bar = c.bar_shown();
        self.state.bar_quota_strip = c.bar_quota_strip;
        self.state.modules = c.modules;
        self.state.ui = c.ui;
        self.state.fahrenheit = c.weather_fahrenheit;
        self.state.control_tiles = c.control_tiles.clone();
        self.state.control_mixer = c.control_mixer;
        self.state.control_outputs = c.control_outputs;
        self.state.power = c.power;
        self.state.notifications.per_page = c.notifications_per_page as usize;
        self.state.notification_page = self.state.notification_page.min(self.state.notifications.last_page());
        // The running Pomodoro and timer take the new lengths now (a started timer keeps its own).
        if (old.pomodoro_focus_secs, old.pomodoro_break_secs) != (c.pomodoro_focus_secs, c.pomodoro_break_secs) {
            self.pomodoro.set_config(PomodoroConfig {
                focus_minutes: c.pomodoro_focus_secs / 60,
                short_break_minutes: c.pomodoro_break_secs / 60,
                ..self.pomodoro.config().clone()
            });
        }
        if old.timer_default_secs != c.timer_default_secs {
            self.countdown.set_duration(c.timer_default_secs as i64 * 1000);
        }
        // An agent switched off leaves the island now: its quota bar and its usage page.
        if !c.ai_claude_enabled { self.state.claude_quota = isle_core::state::QuotaStatus::new(isle_core::state::AiProviderKind::ClaudeCode); }
        if !c.ai_codex_enabled { self.state.codex_quota = isle_core::state::QuotaStatus::new(isle_core::state::AiProviderKind::Codex); }
        self.state.usage.retain(|p| agent_on(&c, &p.id));
        self.overlay.over_fullscreen = c.over_fullscreen;
        self.overlay.hide_maximized = c.hide_maximized;
        self.overlay.hide_apps = c.hide_apps.clone();
        #[cfg(windows)]
        if (old.over_fullscreen, old.hide_maximized, &old.hide_apps) != (c.over_fullscreen, c.hide_maximized, &c.hide_apps) {
            self.overlay.step_aside();
        }
        let monitors_changed = (&old.monitors, old.all_monitors, &old.main_monitor)
            != (&c.monitors, c.all_monitors, &c.main_monitor) || old.ui.follow_mouse != c.ui.follow_mouse;
        if monitors_changed {
            self.overlay.monitors = c.monitors.clone();
            self.overlay.all_monitors = c.all_monitors;
            self.overlay.main_monitor = c.main_monitor.clone();
            self.overlay.follow_mouse = c.ui.follow_mouse;
            self.follow_monitor_at = None;
            // Only islands that exist are made again (tests have none).
            if self.overlay.window_count() > 0 {
                self.unbind_phase8_windows();
                self.overlay.destroy_windows();
                if let Err(err) = self.overlay.create_windows() { tracing::warn!("monitores: {err:#}"); }
                self.sync_islands();
                self.bind_phase8_windows();
                self.bind_shortcuts();
            }
        }
        if old.ui.hover_open && !c.ui.hover_open {
            for island in &mut self.islands { island.enter_at = None; }
        }
        if !c.auto_hide && old.auto_hide {
            for island in &mut self.islands {
                if island.mode == PillMode::Minimized && !island.manually_hidden { island.mode = PillMode::CompactBar; }
            }
        }
        if old.opencode_api_key != c.opencode_api_key {
            if let Some(tx) = &self.chat_tx { let _ = tx.send(isle_ai::assistant::Command::Key(c.opencode_api_key.clone())); }
        }
        if old.pinned_tray_apps != c.pinned_tray_apps {
            if let Some(service) = &self.tray_service {
                service.send(TrayCommand::Pins(c.pinned_tray_apps.clone()));
            }
        }
        let agenda_source = |c: &IsleConfig| (c.calendar_ics_url.clone(), c.holidays, c.holidays_country.clone(), c.timezone_system);
        if agenda_source(old) != agenda_source(&c) {
            if c.calendar_ics_url != old.calendar_ics_url {
                self.ics.clear();
                self.ics_raw.0.clear();
            }
            if c.holidays_country != old.holidays_country {
                self.holidays.clear();
            }
            self.rebuild_agenda();
        }
        // A module off stops its service; a changed source starts it over (start_modules below).
        let (m, was) = (c.modules, old.modules);
        let ai = |c: &IsleConfig| c.modules.ai && (c.ai_claude_enabled || c.ai_codex_enabled);
        if !m.weather || old.weather != c.weather {
            self.stop_service("weather");
        }
        if !m.agenda || agenda_source(old) != agenda_source(&c) {
            self.stop_service("agenda");
        }
        let agents = |c: &IsleConfig| (c.ai_claude_enabled, c.ai_codex_enabled, c.claude_renew_login);
        if !ai(&c) || agents(old) != agents(&c) {
            self.stop_service("usage");
        }
        if !m.tray && was.tray {
            self.stop_service("tray");
        }
        if old.modules != c.modules || old.weather != c.weather || agents(old) != agents(&c) || agenda_source(old) != agenda_source(&c) {
            self.start_modules();
        }
        self.keep_visible_tabs();
        // The clock's and the countdowns' texts follow the new options now, not in a minute.
        if self.state.now_ms > 0 {
            self.tick_runtime_domain(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs());
        }
        self.apply_phase8_config(Some(old));
        self.sync_model_label();
        if old.check_updates != c.check_updates {
            self.sync_update_check();
        }
        if old.start_with_windows != c.start_with_windows {
            self.sync_autostart();
        }
        self.bind_shortcuts();
        if save { self.save_config(); }
    }

    /// Stops a module's service (tokio tasks aborted, the tray's thread told to stop). Media,
    /// notifications and the control center only wait on Windows' events: they stay, their events
    /// unused while the module is off.
    fn stop_service(&mut self, name: &'static str) {
        match name {
            "weather" => self.weather_task.take().into_iter().for_each(|t| t.abort()),
            "agenda" => self.agenda_tasks.drain(..).for_each(|t| t.abort()),
            "usage" => self.usage_task.take().into_iter().for_each(|t| t.abort()),
            // Dropping the service sends it Stop.
            "tray" => self.tray_service = None,
            _ => return,
        }
        self.started.retain(|s| *s != name);
    }

    /// An open panel whose module went off moves to the first module left, or the pill closes.
    /// The page the island opens on: "Aba ao abrir" while its tab shows, else the first tab shown.
    fn opening_tab(&self) -> Option<TabPage> {
        let tabs = isle_render::layout::visible_tabs(&self.config.modules);
        Some(self.config.ui.open_tab).filter(|t| tabs.contains(t)).or(tabs.first().copied())
    }

    fn keep_visible_tabs(&mut self) {
        let tabs = isle_render::layout::visible_tabs(&self.config.modules);
        for island in &mut self.islands {
            if let PillMode::Expanded(tab) = island.mode {
                if !tabs.contains(&tab) && tab != TabPage::Power {
                    match tabs.first() {
                        Some(first) => island.toggle(Some(*first)),
                        None => { island.mode = PillMode::Expanded(tab); island.toggle(None); }
                    }
                }
            }
        }
    }

    fn save_config(&self) {
        let Some(path) = &self.settings_path else { return;
        };
        match serde_json::to_string_pretty(&self.config) {
            Ok(json) => {
                if let Err(err) = isle_core::config::write_atomic(path, &json) { tracing::warn!("configurações: {err:#}"); }
            }
            Err(err) => tracing::warn!("configurações: {err:#}"),
        }
    }

    fn timer_action(&mut self, island: usize, action: isle_render::pages::TimerAction) {
        use isle_core::timers::TimerMode;
        use isle_render::pages::TimerAction;
        let now = self.state.now_ms;
        let mode = self.state.timers.mode;
        match action {
            TimerAction::Mode(next) => {
                if next != TimerMode::Reminders { self.stop_typing_reminder(); }
                self.state.timers.mode = next;
            }
            TimerAction::Field => {
                // One text field has the keyboard at a time.
                self.close_ai_input();
                self.close_reply();
                self.reminder_typing = true;
                self.reply_window = island;
                // The click on the island lets it come to the front (as for a reply).
                if let Err(err) = self.overlay.take_keyboard(island) { tracing::warn!("lembrete: {err:#}"); }
            }
            TimerAction::Add => {
                let text = std::mem::take(&mut self.reminder_draft.text);
                if self.reminders.add(&text, now + self.reminder_minutes as i64 * 60_000).is_some() {
                    self.save_reminders();
                } else {
                    self.reminder_draft.text = text;
                }
                self.stop_typing_reminder();
            }
            TimerAction::In(delta) => {
                self.reminder_minutes = (self.reminder_minutes as i32 + delta).clamp(5, 24 * 60) as u32
            }
            TimerAction::InPreset(minutes) => self.reminder_minutes = minutes,
            TimerAction::Delete(id) => {
                if self.reminders.dismiss(id) { self.save_reminders(); }
            }
            TimerAction::Toggle => match mode {
                TimerMode::Pomodoro => {
                    if self.pomodoro.deadline().is_some() { self.pomodoro.pause(now) } else { self.pomodoro.start(now) }
                }
                TimerMode::Countdown => {
                    if self.countdown.running() { self.countdown.pause(now) } else { self.countdown.start(now) }
                }
                TimerMode::Stopwatch => {
                    if self.stopwatch.running() { self.stopwatch.pause(now) } else { self.stopwatch.start(now) }
                }
                TimerMode::Reminders => {}
            },
            TimerAction::Reset => match mode {
                TimerMode::Pomodoro => self.pomodoro.stop(),
                TimerMode::Countdown => self.countdown.reset(),
                TimerMode::Stopwatch => self.stopwatch.reset(),
                TimerMode::Reminders => {}
            },
            TimerAction::Focus(delta) | TimerAction::Break(delta) => {
                let focus = matches!(action, TimerAction::Focus(_));
                let (secs, low, high) = if focus { (&mut self.config.pomodoro_focus_secs, 5, 180) } else { (&mut self.config.pomodoro_break_secs, 1, 60) };
                *secs = ((*secs / 60) as i32 + delta).clamp(low, high) as u32 * 60;
                self.pomodoro.set_config(PomodoroConfig {
                    focus_minutes: self.config.pomodoro_focus_secs / 60,
                    short_break_minutes: self.config.pomodoro_break_secs / 60,
                    ..self.pomodoro.config().clone()
                });
                self.save_config();
            }
            TimerAction::Duration(_) | TimerAction::Preset(_) => {
                let minutes = match action {
                    TimerAction::Preset(m) => m as i64,
                    TimerAction::Duration(d) => {
                        (self.countdown.duration_ms / 60_000 + d as i64).clamp(1, 24 * 60)
                    }
                    _ => unreachable!(),
                };
                self.countdown.set_duration(minutes * 60_000);
                self.config.timer_default_secs = (self.countdown.duration_ms / 1000) as u32;
                self.save_config();
            }
        }
        self.refresh_timers(now);
    }

    fn stop_typing_reminder(&mut self) {
        if std::mem::take(&mut self.reminder_typing) { self.overlay.release_keyboard(); }
    }

    /// A click on the main island.
    #[cfg(test)]
    pub fn apply_ui_action(&mut self, action: UiAction) {
        self.apply_ui_action_on(0, action);
    }

    fn sync_control_panel(&mut self) {
        let open = self.islands.iter().any(|i| i.mode == PillMode::Expanded(TabPage::ControlCenter));
        if !open && (self.state.control_editing || self.tile_drag.is_some()) {
            self.state.control_editing = false;
            self.state.control_drag = None;
            self.tile_drag = None;
        }
        if open != self.control_panel_open {
            self.control_panel_open = open;
            self.send_audio(AudioCommand::Panel(open));
            self.send_control(ControlCommand::Panel(open));
        }
        if self.volume_drag.as_ref().is_some_and(|d| {
            self.islands.get(d.window).is_none_or(|i| i.mode != PillMode::Expanded(TabPage::ControlCenter))
        }) {
            self.volume_drag = None;
            self.overlay.end_slider_drag();
        }
    }

    fn sync_tray_panel(&mut self) {
        let open = self.islands.iter().any(|i| i.mode == PillMode::Expanded(TabPage::Tray));
        if open != self.tray_panel_open {
            self.tray_panel_open = open;
            if let Some(service) = &self.tray_service { service.send(TrayCommand::Panel(open)); }
        }
        let width = isle_render::pages::compact_tray_width(&self.state);
        let height = self.config.ui.pill_height_px();
        let curve = self.config.notch_curve as f32 / 75.0;
        let notch = self.config.ui.pill_style == isle_core::config::PillStyle::Notch;
        for island in &mut self.islands {
            match island.mode {
                PillMode::CompactBar if notch => island.morph((width, height, isle_render::renderer::notch_radius(height, self.config.notch_curve))),
                PillMode::CompactBar => island.morph((width, height, (height * 0.5 * curve).min(height * 0.5))),
                PillMode::Minimized => {
                    let line_width = if self.config.ui.line_full_width { width } else { LINE.0 };
                    let line_height = if self.config.ui.timer_on_line && self.state.timer_text.is_some() { 19. } else { LINE.1 };
                    island.morph((line_width, line_height, (LINE.2 * curve).min(line_height * 0.5)));
                }
                _ => {}
            }
        }
    }

    /// The control (its index) under the mouse in island `window`'s edit mode.
    fn control_under(&mut self, window: usize, x: f32, y: f32) -> Option<usize> {
        let island = self.island(window);
        let (w, h, mode, scroll) = (island.geom.width.value, island.geom.height.value, island.mode.clone(), island.scroll);
        self.state.mode = mode;
        self.state.panel_scroll = scroll;
        compute_hitboxes(CANVAS_MAX_WIDTH as f32, w, h, self.config.top_gap(), &self.state).into_iter()
            .find_map(|hb| match hb.action {
                UiAction::ControlGrab(i) if hb.contains(x, y) => Some(i),
                _ => None,
            })
    }

    /// The wheel over a volume slider of the open control center turns it, 2% a notch; over an app's
    /// row, that app. Anywhere else on an open panel it scrolls or does nothing.
    // Port of ControlPanel::OnWheel (mac-island 3.1.1).
    fn wheel_slider(&mut self, window: usize, delta: i32, x: f32, y: f32) -> bool {
        let Some(island) = self.islands.get(window) else { return false };
        if !self.config.wheel_volume || island.mode != PillMode::Expanded(TabPage::ControlCenter) { return false; }
        let (w, h) = (island.geom.width.value, island.geom.height.value);
        let hit = compute_hitboxes(CANVAS_MAX_WIDTH as f32, w, h, self.config.top_gap(), &self.state).into_iter()
            .find(|b| b.contains(x, y) && matches!(b.action, UiAction::SetVolume(_) | UiAction::AppVolume(..)));
        match hit.map(|b| b.action) {
            Some(UiAction::SetVolume(_)) => self.send_audio(AudioCommand::Wheel(delta)),
            Some(UiAction::AppVolume(i, _)) => {
                let Some(app) = self.state.app_volumes.get_mut(i) else { return false };
                // Kept here too, so quick notches add up before the mixer refreshes.
                app.volume_pct = audio_osd::wheel_volume(app.volume_pct, delta);
                let command = AudioCommand::SetAppVolume(app.id.clone(), app.volume_pct);
                self.send_audio(command);
            }
            _ => return false,
        }
        true
    }

    fn send_audio(&self, command: AudioCommand) {
        if let Some(tx) = &self.audio_tx { let _ = tx.send(command); }
    }
    fn send_control(&self, command: ControlCommand) {
        if let Some(tx) = &self.controls_tx { let _ = tx.send(command); }
    }

    fn drag_volume_to(&self, window: usize, x: f32) {
        if let Some(drag) = &self.volume_drag {
            // Only while that island still shows the control center's sliders.
            if drag.window != window || self.islands.get(window).is_none_or(|i| i.mode != PillMode::Expanded(TabPage::ControlCenter)) { return; }
            let pct = slider_percent(x, drag.left, drag.width);
            self.send_audio(match &drag.app_id {
                Some(id) => AudioCommand::SetAppVolume(id.clone(), pct),
                None => AudioCommand::SetVolume(pct),
            });
        }
    }

    fn apply_notifications(&mut self, items: Vec<isle_core::notifications::Notification>, initial: bool,
    ) {
        let muted = &self.config.muted_apps;
        let items: Vec<_> = items.into_iter().filter(|n| !muted.iter().any(|m| m.eq_ignore_ascii_case(&n.app_id))).collect();
        let newest = self.state.notifications.receive(items, initial);
        self.state.notification_page = self.state.notification_page.min(self.state.notifications.last_page());
        let Some(n) = newest else { return; };
        if !self.config.banners { return; }
        let own_banner = self.notification_banner.as_ref().is_some_and(|(_,mode)| *mode == self.islands[0].mode);
        if self.state.dnd_active || self.pending_reply.is_some()
            || !(own_banner || self.islands[0].mode == PillMode::CompactBar
                || (self.config.ui.banner_when_hidden && self.islands[0].mode == PillMode::Minimized)) { return; }
        self.islands[0].mode = PillMode::AlertBanner {
            title: format!("{} · {}", n.app_name.chars().take(60).collect::<String>(), n.title.chars().take(120).collect::<String>()),
            subtitle: n.body.replace('\n', " ").chars().take(180).collect(),
        };
        let size = self.notification_banner_size();
        self.islands[0].morph(size);
        self.notification_banner = Some((n,self.islands[0].mode.clone()));
        self.banner_until = Some(Instant::now() + Duration::from_secs(self.config.banner_secs as u64));
    }

    fn apply_notification_icon(&mut self, app_id: &str, image: std::sync::Arc<isle_core::artwork::Artwork>) {
        self.state.notifications.set_app_icon(app_id,image.clone());
        if let Some((n,_)) = &mut self.notification_banner {
            if n.app_id.eq_ignore_ascii_case(app_id) { n.app_icon=Some(image); }
        }
    }

    /// The mod's SendReply: what's typed goes to the app as its own reply box would send it.
    fn send_reply(&mut self) {
        if let Some(reply) = self.state.reply.as_ref().filter(|r| !r.text.trim().is_empty()) {
            let target = self.state.notifications.ordered().into_iter()
                .find(|n| n.id == reply.id && n.arrival_ms == reply.arrival_ms && n.can_reply())
                .map(|n| {
                    (n.app_id.clone(), n.reply_args.clone(), n.reply_input.clone(),
                    )
                });
            tracing::info!("resposta: {} caracteres, aviso ainda na ilha: {}", reply.text.chars().count(), target.is_some());
            if let Some((app, args, input)) = target.filter(|_| !cfg!(test)) {
                notifications::send_reply(app, args, input, reply.text.clone());
            }
        }
        self.close_reply();
    }

    fn close_reply(&mut self) {
        self.state.reply = None;
        self.overlay.release_keyboard();
    }

    fn toggle_notifications(&mut self) {
        if self.pending_reply.is_none() && self.islands[0].mode != PillMode::PermissionPrompt {
            let tab = (self.islands[0].mode != PillMode::Expanded(TabPage::Notifications)).then_some(TabPage::Notifications);
            self.islands[0].toggle(tab);
        }
    }

    fn apply_audio(&mut self, status: AudioStatus) {
        let changed = self.state.audio_available && status.volume_pct.is_some()
            && (status.volume_pct != Some(self.state.volume_pct) || status.muted != self.state.muted);
        self.state.audio_available = status.volume_pct.is_some();
        self.state.volume_pct = status.volume_pct.unwrap_or(0);
        self.state.muted = status.muted;
        self.state.mic_available = status.mic_muted.is_some();
        self.state.mic_muted = status.mic_muted.unwrap_or(false);
        self.state.app_volumes = status.apps;
        self.state.audio_outputs = status.outputs;
        let main = &mut self.islands[0];
        let can_show = matches!(main.mode, PillMode::CompactBar | PillMode::Minimized)
            || matches!(&main.mode, PillMode::AlertBanner { title, .. } if title == tr("banner.volume"));
        if changed && can_show {
            main.mode = PillMode::AlertBanner { title: tr("banner.volume").into(),
                subtitle: if status.muted { tr("banner.muted").into() } else { format!("{}%", self.state.volume_pct) },
            };
            let (w, h) = self.config.ui.banner_px();
            let k = self.config.ui.banner_scale();
            main.morph(((w * 300.0 / 440.0).round(), h, 20.0 * k));
            self.banner_until = Some(Instant::now() + Duration::from_secs(2));
        }
    }

    pub fn apply_ui_action_on(&mut self, island: usize, action: UiAction) {
        match action {
            UiAction::Phase8(action) => self.phase8_action(island, action),
            UiAction::AiOpen | UiAction::AiBack | UiAction::AiInput | UiAction::AiSend | UiAction::AiStop | UiAction::AiNew | UiAction::AiProvider(_) | UiAction::AiScroll(_) | UiAction::AiIdea(_) | UiAction::AiRun | UiAction::AiDeny => self.ai_action(island,action),
            UiAction::ReplyNotification(id, arrival) => {
                self.close_ai_input();
                self.stop_typing_reminder();
                // A click on the box already open keeps what's typed.
                if self.state.reply.as_ref().is_none_or(|r| (r.id, r.arrival_ms) != (id, arrival)) {
                    self.state.reply = Some(isle_core::notifications::Reply { id, arrival_ms: arrival, text: String::new(),
                    });
                }
                self.state.notifications.mark_read(id, arrival);
                self.reply_window = island;
                // The click on the island lets it come to the front; if Windows still says no, the
                // box stays (a click elsewhere closes it).
                if let Err(err) = self.overlay.take_keyboard(island) { tracing::warn!("resposta: {err:#}"); }
            }
            UiAction::SendReply => self.send_reply(),
            UiAction::ReadNotification(id, arrival) => {
                self.state.notifications.mark_read(id, arrival);
                // Like the mod's panel: a click opens the notification in its app (not in tests).
                let clicked = self.state.notifications.ordered().into_iter()
                    .find(|n| n.id == id && n.arrival_ms == arrival)
                    .map(|n| (n.app_id.clone(), n.launch.clone(), n.launch_type.clone()));
                if let Some((app, launch, launch_type)) = clicked.filter(|_| !cfg!(test)) {
                    notifications::open_notification(app, launch, launch_type);
                }
            }
            UiAction::ClearNotifications => {
                self.state.notifications.clear(); self.state.notification_page = 0;
                if let Some(path) = &self.cleared_path {
                    if let Err(err) = std::fs::write(path, self.state.notifications.cleared_until.to_string()) {
                        tracing::warn!("marca de avisos limpos: {err:#}");
                    }
                }
            }
            UiAction::NotificationsPage(page) => {
                self.state.notification_page = page.min(self.state.notifications.last_page());
            }
            UiAction::Permission(decision, card) => {
                // A double-click answers one card: the next one, sliding in under the cursor, takes no
                // click for a moment, nor one aimed at a card already gone.
                if self.state.pending_permission.as_ref().map(|c| c.key()) != Some(card)
                    || self.permission_shown.elapsed() < PERMISSION_CLICK_GUARD { return; }
                if let Some(reply) = self.pending_reply.take() {
                    let _ = reply.send(decision);
                }
                self.close_permission_card();
            }
            // Port of isle's agent alert `show` (Main.activateWindow): the card stays to be answered.
            UiAction::PermissionFocus(card) => {
                if let Some(card) = self.state.pending_permission.as_ref().filter(|c| c.key() == card) {
                    #[cfg(all(windows, not(test)))]
                    if !isle_platform_win::agent_window::focus(&card.pids, &card.project) {
                        tracing::info!("janela do agente não encontrada ({})", card.project);
                    }
                    #[cfg(any(not(windows), test))]
                    let _ = card;
                }
            }
            UiAction::Timer(action) => self.timer_action(island, action),
            UiAction::OpenSettings => self.open_settings(),
            UiAction::OpenWeather => {
                self.apply_ui_action_on(island, UiAction::SwitchTab(TabPage::Weather))
            }
            UiAction::WeatherDay(day) => self.state.weather_day = day,
            UiAction::SwitchTab(tab) => {
                if !isle_render::layout::visible_tabs(&self.config.modules).contains(&tab) { return; }
                if tab == TabPage::CalendarWeather { self.state.agenda_day = self.state.today; }
                if tab == TabPage::Weather { self.state.weather_day = 0; }
                // The Uso tab is the usage page; the chat is the ✦'s.
                if tab == TabPage::AiHub { self.state.chat.open = false; }
                self.island(island).toggle(Some(tab));
            }
            UiAction::AgendaDay(day) => self.state.agenda_day = day,
            UiAction::AgendaMonth(delta) => {
                let (y, m, d) = civil_from_days(self.state.agenda_day);
                let index = y * 12 + m - 1 + delta as i64;
                let (y, m) = (index.div_euclid(12), index.rem_euclid(12) + 1);
                self.state.agenda_day = days_from_civil(y, m, d.min(month_len(y, m)));
            }
            UiAction::ToggleCollapse => {
                // Port of IslandItem::Minimize/ArrowPinsOn (mac-island 3.1.1, 118466).
                if self.config.ui.arrow_pins && self.config.auto_hide && matches!(self.island(island).mode,PillMode::Expanded(_)) {
                    let presence = self.island(island);
                    presence.pinned = !presence.pinned;
                    presence.leave_at = None;
                    return;
                }
                if island == 0 && self.notification_banner.as_ref().is_some_and(|(_,mode)| *mode == self.islands[0].mode) {
                    // The mod's banner click: the notification opens in its app (WhatsApp: the chat).
                    let banner = self.notification_banner.take();
                    let main = &mut self.islands[0];
                    main.mode = PillMode::CompactBar;
                    main.morph(COMPACT);
                    main.geom.glow_intensity.set_target(0.0);
                    if let Some((n,_)) = banner { self.apply_ui_action_on(0, UiAction::ReadNotification(n.id,n.arrival_ms)); }
                } else if let Some((_, opens)) = self.alert_banner.take().filter(|(mode, _)| island == 0 && *mode == self.islands[0].mode) {
                    match opens {
                        Opens::Tab(tab) => self.apply_ui_action_on(0, UiAction::SwitchTab(tab)),
                        Opens::Settings(tab) => {
                            let main = &mut self.islands[0];
                            main.mode = PillMode::CompactBar;
                            main.morph(COMPACT);
                            self.open_settings();
                            self.settings_action(isle_render::settings::SettingsAction::Tab(tab));
                        }
                    }
                } else {
                    // Opening: on "Aba ao abrir" (isle's _defaultPage), the assistant closed.
                    let opening = matches!(self.island(island).mode, PillMode::CompactBar | PillMode::AlertBanner { .. });
                    let tab = self.opening_tab().filter(|_| opening);
                    if opening { self.state.chat.open = false; }
                    self.island(island).toggle(tab); self.keep_visible_tabs();
                }
            }
            UiAction::Media(key) => {
                let command = match key {
                    MediaKey::Previous => MediaCommand::Previous,
                    MediaKey::PlayPause => MediaCommand::PlayPause,
                    MediaKey::Next => MediaCommand::Next,
                };
                // Windows may take its time answering: never on the UI thread.
                std::thread::spawn(move || {
                    if let Err(err) = media::send(command) {
                        tracing::warn!("comando de mídia: {err:#}");
                    }
                });
            }
            UiAction::Control(isle_core::state::ControlTile::Power) | UiAction::OpenPower => self.open_power(island),
            UiAction::Power(_) | UiAction::PowerConfirm | UiAction::PowerCancel => self.power_action(island, action),
            UiAction::EditControls => {
                self.state.control_editing = !self.state.control_editing;
                self.state.control_drag = None;
                self.tile_drag = None;
                self.island(island).scroll = 0.;
            }
            UiAction::ControlRemove(tile) => self.edit_controls(|tiles| tiles.retain(|t| *t != tile)),
            UiAction::ControlAdd(tile) => self.edit_controls(|tiles| if !tiles.contains(&tile) { tiles.push(tile) }),
            UiAction::ControlGrab(_) => {}
            UiAction::Control(tile) => self.activate_control(island, tile),
            UiAction::OpenMixer => self.send_control(ControlCommand::OpenMixer),
            UiAction::SetOutput(i) => {
                if let Some(output) = self.state.audio_outputs.get(i) { self.send_audio(AudioCommand::SetOutput(output.id.clone())); }
            }
            UiAction::SetVolume(pct) => self.send_audio(AudioCommand::SetVolume(pct)),
            UiAction::ToggleMute => self.send_audio(AudioCommand::ToggleMute),
            UiAction::ToggleMic => self.send_audio(AudioCommand::ToggleMic),
            UiAction::AppVolume(i, pct) => {
                if let Some(app) = self.state.app_volumes.get(i) { self.send_audio(AudioCommand::SetAppVolume(app.id.clone(),pct)); }
            }
            UiAction::AppMute(i) => {
                if let Some(app) = self.state.app_volumes.get(i) { self.send_audio(AudioCommand::ToggleAppMute(app.id.clone())); }
            }
            UiAction::ToggleTray => {
                if self.island(island).mode != PillMode::PermissionPrompt {
                    let tab = (self.island(island).mode != PillMode::Expanded(TabPage::Tray)).then_some(TabPage::Tray);
                    self.island(island).toggle(tab);
                }
            }
            UiAction::TrayPage(page) => self.state.tray_page = page,
            UiAction::TrayClick(index,right) => {
                if let (Some(service),Some(item)) = (&self.tray_service,self.state.tray.get(index)) {
                    service.send(TrayCommand::Activate(item.key.clone(),right));
                }
            }
        }
        self.sync_ai_input();
    }

    /// The control center's controls changed in its edit mode: kept in the settings at once.
    fn edit_controls(&mut self, change: impl FnOnce(&mut Vec<isle_core::state::ControlTile>)) {
        let old = self.config.clone();
        change(&mut self.config.control_tiles);
        self.apply_config(&old, true);
    }

    /// A control center button. Port of ControlPanel::OnMouseUp (mac-island 3.1.1): the island's own
    /// open here (the microphone, the timer's tabs, the weather, the agenda, the USB drives); the
    /// shortcuts that act on the window in front close the island first; the rest go to Windows.
    fn activate_control(&mut self, island: usize, tile: isle_core::state::ControlTile) {
        use isle_core::state::ControlTile as C;
        use isle_core::timers::TimerMode;
        self.state.control_error = None;
        let visible = isle_render::layout::visible_tabs(&self.config.modules);
        let timer = match tile {
            C::Timer => Some(TimerMode::Countdown),
            C::Pomodoro => Some(TimerMode::Pomodoro),
            C::Stopwatch => Some(TimerMode::Stopwatch),
            C::Alarms => Some(TimerMode::Reminders),
            _ => None,
        };
        let tab = match tile {
            _ if timer.is_some() => Some(TabPage::Timer),
            C::Weather => Some(TabPage::Weather),
            C::NextEvent => Some(TabPage::CalendarWeather),
            C::Drives => Some(TabPage::Usb),
            _ => None,
        }.filter(|t| visible.contains(t));
        if let Some(tab) = tab {
            if let Some(mode) = timer { self.state.timers.mode = mode; }
            self.apply_ui_action_on(island, UiAction::SwitchTab(tab));
            return;
        }
        if matches!(tile, C::Timer | C::Pomodoro | C::Stopwatch | C::Alarms | C::Weather | C::NextEvent) {
            self.state.control_error = Some(trf("control.module_off", &[("name", &tile.label())]));
            return;
        }
        if tile == C::Microphone {
            self.send_audio(AudioCommand::ToggleMic);
            return;
        }
        if tile.closes_island() && matches!(self.island(island).mode, PillMode::Expanded(_)) {
            self.island(island).toggle(None);
        }
        self.send_control(ControlCommand::Activate(tile));
    }

    /// Port of the power button's click (mac-island 3.1.1, OnlyPowerAction): the one action on runs at
    /// once unless it asks first; otherwise its menu opens.
    fn open_power(&mut self, island: usize) {
        let power = self.config.power;
        match power.only(self.state.hibernate_allowed) {
            Some(only) if !power.asks(only) => self.run_power(island, only),
            _ => {
                self.state.power_confirm = None;
                let open = self.island(island).mode == PillMode::Expanded(TabPage::Power);
                self.island(island).toggle((!open).then_some(TabPage::Power));
            }
        }
    }

    /// Port of PowerPanel::OnMouseUp: an action that asks opens its confirmation (again: closes it).
    fn power_action(&mut self, island: usize, action: UiAction) {
        match action {
            UiAction::Power(a) if self.config.power.asks(a) => {
                self.state.power_confirm = (self.state.power_confirm != Some(a)).then_some(a);
            }
            UiAction::Power(a) => self.run_power(island, a),
            UiAction::PowerConfirm => {
                if let Some(a) = self.state.power_confirm.take() { self.run_power(island, a); }
            }
            _ => self.state.power_confirm = None,
        }
    }

    /// The menu closes, then Windows does it (on its own thread: sleeping returns on waking).
    fn run_power(&mut self, island: usize, action: isle_core::power::PowerAction) {
        self.state.power_confirm = None;
        if matches!(self.island(island).mode, PillMode::Expanded(_)) { self.island(island).toggle(None); }
        tracing::info!("energia: {action:?}");
        #[cfg(test)]
        POWER_RAN.with(|ran| ran.borrow_mut().push(action));
        #[cfg(not(test))]
        std::thread::spawn(move || {
            if let Err(err) = isle_platform_win::power::run(action) { tracing::warn!("energia: {err:#}"); }
        });
    }

    /// Island `i`, or the main one for an index no longer there (monitors changed meanwhile).
    fn island(&mut self, i: usize) -> &mut Presence {
        let i = if i < self.islands.len() { i } else { 0 };
        &mut self.islands[i]
    }

    /// One island per window (at least the main one, also without windows).
    fn sync_islands(&mut self) {
        let count = self.overlay.window_count().max(1);
        self.islands.truncate(count);
        while self.islands.len() < count {
            self.islands.push(Presence::new());
        }
    }

    /// The mouse reached (`true`) or left the main island.
    #[cfg(test)]
    pub fn on_hover(&mut self, inside: bool) {
        self.islands[0].hover(inside, self.config.ui);
    }

    /// Banners going by themselves (main island), open panels closing and pills hiding once the mouse
    /// left (every island). Called on every wake; a deadline is armed the first time its state is seen.
    pub fn on_deadlines(&mut self, now: Instant) {
        let main = &mut self.islands[0];
        if matches!(main.mode, PillMode::AlertBanner { .. }) {
            match self.banner_until {
                None => self.banner_until = Some(now + BANNER_FOR),
                Some(at) if now >= at && !main.hovered && self.files_over != Some(0) => {
                    self.banner_until = None;
                    main.mode = PillMode::CompactBar;
                    main.morph(COMPACT);
                    main.geom.glow_intensity.set_target(0.0);
                }
                _ => {}
            }
        } else {
            self.banner_until = None;
        }
        self.show_alert(now);
        let auto_hide = self.config.auto_hide;
        let ui = self.config.ui;
        let initial_tab = self.opening_tab();
        let mut hover_opened = false;
        let width = isle_render::pages::compact_tray_width(&self.state);
        let keep_shelf = self.shelf_share_in_progress();
        let clipboard_search = self.clipboard_search_window();
        let files_over = self.files_over;
        for (index,island) in self.islands.iter_mut().enumerate() {
            if island.enter_at.is_some_and(|at| now >= at) {
                island.enter_at = None;
                if island.hovered && island.mode == PillMode::CompactBar {
                    if let Some(tab) = initial_tab { island.toggle(Some(tab)); hover_opened = true; }
                }
            }
            #[cfg(windows)]
            let hide = auto_hide && (!matches!(island.mode, PillMode::CompactBar | PillMode::Minimized)
                || self.overlay.smart_hide(index, ui.hide_when, width, ui.pill_height_px(), self.config.top_gap()));
            #[cfg(not(windows))]
            let hide = auto_hide && ui.hide_when == isle_core::config::HideWhen::MouseAway;
            if !hide && island.mode == PillMode::Minimized && !island.manually_hidden {
                island.mode = PillMode::CompactBar;
                island.morph(COMPACT);
            }
            if self.volume_drag.as_ref().is_some_and(|drag| drag.window == index)
                || ((self.state.reply.is_some() || self.reminder_typing) && self.reply_window == index)
                || self.chat_window == Some(index) || clipboard_search == Some(index) || files_over == Some(index)
                || (keep_shelf && island.mode == PillMode::Expanded(TabPage::Shelf)) {
                island.leave_at = None;
            } else {
                island.leave(now, hide, ui);
            }
            if island.mode == PillMode::Minimized {
                let since = *island.line_since.get_or_insert(now);
                let timer = ui.timer_on_line && self.state.timer_text.is_some();
                let downloading = self.state.phase8.download_meter && self.state.phase8.downloads.is_some();
                island.line_hidden = !island.hovered && !timer && !downloading && ui.line_expire_secs > 0
                    && now >= since + Duration::from_secs(u64::from(ui.line_expire_secs));
            } else {
                island.line_since = None;
                island.line_hidden = false;
            }
        }
        if hover_opened { self.state.chat.open = false; }
    }

    /// When `on_deadlines` next has something to do.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.islands
            .iter()
            .flat_map(|i| {
                let expires = self.config.ui.line_expire_secs;
                let timer = self.config.ui.timer_on_line && self.state.timer_text.is_some();
                let downloading = self.state.phase8.download_meter && self.state.phase8.downloads.is_some();
                let line = i.line_since.filter(|_| expires > 0 && !i.hovered && !i.line_hidden && !timer && !downloading)
                    .map(|since| since + Duration::from_secs(u64::from(expires)));
                [i.leave_at, i.enter_at, line]
            })
            .chain([self.banner_until.filter(|_| !self.islands[0].hovered && self.files_over != Some(0))])
            .flatten()
            .min()
    }

    /// Closes the card on screen and shows the oldest ask still waiting, if any.
    fn close_permission_card(&mut self) {
        self.pending_reply = None;
        self.state.pending_permission = None;
        while let Some((card, reply)) = self.permission_queue.pop_front() {
            if !reply.is_closed() {
                self.show_permission(card, reply);
                return;
            }
        }
        self.state.permission_waiting = 0;
        self.islands[0].mode = PillMode::CompactBar;
        self.islands[0].geom.morph_to(220.0, 34.0, 17.0);
        self.islands[0].geom.glow_intensity.set_target(0.0);
    }

    fn show_permission(&mut self, card: PermissionCard, reply: oneshot::Sender<PermissionDecision>) {
        let height = card_height(&card.command);
        self.pending_reply = Some(reply);
        self.permission_shown = Instant::now();
        self.state.pending_permission = Some(card);
        self.state.permission_waiting = self.permission_queue.len();
        self.islands[0].mode = PillMode::PermissionPrompt;
        self.islands[0].geom.morph_to(CARD_WIDTH, height, 22.0);
        self.islands[0].geom.glow_intensity.set_target(1.0);
    }

    /// Closes the card once the hook server stopped waiting (timeout), dropping asks that gave up
    /// too; returns true if anything changed.
    pub fn expire_stale_permission(&mut self) -> bool {
        let before = self.permission_queue.len();
        self.permission_queue.retain(|(_, r)| !r.is_closed());
        self.state.permission_waiting = self.permission_queue.len();
        if self.pending_reply.as_ref().is_some_and(|r| r.is_closed()) {
            self.close_permission_card();
            return true;
        }
        before != self.permission_queue.len()
    }

    /// An agent's or a quota's banner. A permission card outranks it: while one waits, the banner waits
    /// its turn behind it in the alerts' queue (`show_alert`), never covering the card.
    fn agent_banner(&mut self, title: String, subtitle: String, (w, h, r): (f32, f32, f32), glow: Option<f32>) {
        if self.pending_reply.is_some() {
            self.alerts.push_back((title, subtitle, None));
            return;
        }
        self.islands[0].mode = PillMode::AlertBanner { title, subtitle };
        self.islands[0].geom.morph_to(w, h, r);
        if let Some(glow) = glow { self.islands[0].geom.glow_intensity.set_target(glow); }
    }

    pub fn handle_hook_event(&mut self, ev: HookEvent, now_unix: u64) {
        self.apply_hook_event(ev, now_unix);
        // A new banner gets its own time on screen.
        if matches!(self.islands[0].mode, PillMode::AlertBanner { .. }) {
            self.banner_until = None;
        }
    }

    fn apply_hook_event(&mut self, ev: HookEvent, now_unix: u64) {
        match ev {
            HookEvent::Ask { request, reply } if !command_fits(&request.input) => {
                // Dropping `reply` answers nothing: Claude asks in its terminal, where the command shows whole.
                drop(reply);
                self.agent_banner(titled(&request.provider, "agent.permission", &request.project),
                    tr("agent.too_long").into(), (360.0, 64.0, 20.0), None);
            }
            HookEvent::Ask { request, reply } => {
                let card = PermissionCard {
                    request_id: request.id,
                    agent: titled(&request.provider, "", &request.project),
                    tool_name: request.tool,
                    command: request.input,
                    pids: request.pids,
                    project: request.project,
                };
                if self.pending_reply.is_none() {
                    self.show_permission(card, reply);
                } else if self.permission_queue.len() + 1 < MAX_WAITING {
                    self.permission_queue.push_back((card, reply));
                    self.state.permission_waiting = self.permission_queue.len();
                }
                // Over the limit the reply is dropped: that one asks in its own terminal (a flood is never shown).
            }
            // Claude also notifies the permission it is asking the island about: the card already shows it.
            HookEvent::Agent(event)
                if event.kind == "permission" && self.state.pending_permission.is_some() => {}
            HookEvent::Agent(event) => {
                let doing = match event.kind.as_str() {
                    "done" => "agent.done",
                    "permission" => "agent.permission",
                    _ => "agent.waiting",
                };
                let glow = (event.kind == "done").then_some(0.6);
                self.agent_banner(titled(&event.provider, doing, &event.project), event.message, (360.0, 64.0, 20.0), glow);
            }
            HookEvent::QuotaSync {
                provider,
                used_pct,
                reset_at_unix,
            } => {
                let id = match provider {
                    isle_core::state::AiProviderKind::ClaudeCode => "claude",
                    isle_core::state::AiProviderKind::Codex => "codex",
                    isle_core::state::AiProviderKind::OpenCodeGo => "opencode",
                };
                if !agent_on(&self.config, id) { return; }
                let alerts = self.config.quota_alerts;
                let target_quota = match provider {
                    isle_core::state::AiProviderKind::ClaudeCode => &mut self.state.claude_quota,
                    isle_core::state::AiProviderKind::Codex => &mut self.state.codex_quota,
                    isle_core::state::AiProviderKind::OpenCodeGo => &mut self.state.opencode_quota,
                };
                if let Some(alert) = target_quota.update_reading(used_pct, reset_at_unix, now_unix).filter(|_| alerts)
                {
                    match alert {
                        QuotaAlertEvent::Threshold80(p) => {
                            self.agent_banner(trf("quota.80", &[("name", &quota_name(p))]),
                                tr("quota.80_sub").to_string(), (340.0, 58.0, 18.0), None);
                        }
                        QuotaAlertEvent::Threshold95(p) => {
                            self.agent_banner(trf("quota.95", &[("name", &quota_name(p))]),
                                tr("quota.95_sub").to_string(), (340.0, 58.0, 18.0), Some(0.9));
                        }
                        QuotaAlertEvent::QuotaResetReady { title, .. } => {
                            self.agent_banner(title, tr("quota.reset_sub").to_string(), (360.0, 64.0, 20.0), Some(1.0));
                        }
                    }
                }
            }
        }
    }

    /// What plays changed. A new track (another title, artist or length) returns its lyrics lookup.
    pub fn apply_media(&mut self, status: Option<MediaStatus>) -> Option<TrackQuery> {
        let Some(status) = status else {
            self.state.media = None;
            self.state.media_title = None;
            self.lyrics_key = None;
            self.lyrics.clear();
            return None;
        };
        let query = TrackQuery {
            title: status.title.clone(),
            artist: status.artist.clone(),
            album: status.album.clone(),
            length_s: status.duration_ms as f64 / 1000.0,
        };
        self.state.media_title = Some(status.title.clone());
        self.state.media = Some(MediaInfo {
            artwork: status.artwork,
            title: status.title,
            artist: status.artist,
            playing: status.playing,
            position_ms: status.position_ms,
            duration_ms: status.duration_ms,
            sampled_at_ms: status.sampled_at_ms,
        });
        let key = query.key();
        if self.lyrics_key.as_deref() == Some(key.as_str()) {
            return None;
        }
        self.lyrics_key = Some(key);
        self.lyrics.clear();
        self.config.lyrics.then_some(query)
    }

    /// Lyrics found for a track: kept only if it still plays, and only timed lines (plain ones can't follow).
    pub fn apply_lyrics(&mut self, lyrics: Lyrics) {
        if self.lyrics_key.as_deref() == Some(lyrics.track.as_str()) && lyrics.status == LyricsStatus::Synced {
            self.lyrics = lyrics.lines.into_iter().filter(|l| l.time_ms >= 0).collect();
        }
    }

    /// The line sung at `now_ms`.
    pub fn refresh_lyric(&mut self, now_ms: i64) {
        let position = self.state.media.as_ref().map(|m| m.position_at(now_ms));
        self.state.active_lyric_line = position.and_then(|p| {
            self.lyrics
                .iter()
                .rev()
                .find(|l| l.time_ms <= p)
                .map(|l| l.text.clone())
                .filter(|t| !t.is_empty())
        });
    }

    /// When the next line starts (Unix ms), while the song plays.
    pub fn next_lyric_at(&self, now_ms: i64) -> Option<i64> {
        let media = self.state.media.as_ref().filter(|m| m.playing)?;
        let position = media.position_at(now_ms);
        let next = self.lyrics.iter().find(|l| l.time_ms > position)?;
        Some(now_ms + next.time_ms - position)
    }

    /// New limits read: all of them for the AI tab; each AI's fullest window drives the bar on the pill and
    /// the 80% / 95% / "de volta!" alerts.
    pub fn apply_usage(&mut self, providers: Vec<isle_core::state::UsageProvider>, now_unix: u64) {
        let providers: Vec<_> = providers.into_iter().filter(|p| agent_on(&self.config, &p.id)).collect();
        for provider in &providers {
            let kind = match provider.id.as_str() {
                "claude" => isle_core::state::AiProviderKind::ClaudeCode,
                "codex" => isle_core::state::AiProviderKind::Codex,
                _ => continue,
            };
            let Some(fullest) = provider.windows.iter().max_by(|a, b| a.percent.total_cmp(&b.percent)) else {
                continue;
            };
            self.handle_hook_event(
                HookEvent::QuotaSync {
                    provider: kind,
                    used_pct: fullest.percent as f32,
                    reset_at_unix: (fullest.resets_at_ms > 0).then_some(fullest.resets_at_ms as u64 / 1000),
                },
                now_unix,
            );
        }
        self.state.usage = providers;
    }

    /// Draws every island as it is now (each its own mode and spring, the same data).
    fn present(&mut self) -> Result<()> {
        self.watch_downloads_tab();
        let alert = matches!(self.islands[0].mode, PillMode::AlertBanner { .. } | PillMode::PermissionPrompt);
        self.overlay.keep_main_shown(alert && self.config.ui.banner_over_apps);
        let gap = self.config.top_gap();
        for (i, island) in self.islands.iter().enumerate().take(self.overlay.window_count()) {
            self.state.mode = island.mode.clone();
            self.state.arrow_pinned = island.pinned;
            self.state.notification_banner = self.notification_banner.as_ref()
                .filter(|(_,mode)| i == 0 && *mode == island.mode)
                .map(|(n,_)| n.clone());
            self.state.panel_scroll=island.scroll;
            self.state.line_hidden = island.line_hidden;
            self.state.drop_hint = self.files_over == Some(i) && matches!(island.mode, PillMode::Expanded(_));
            let hit = HitArea::of(&island.geom, gap, &island.mode);
            self.overlay.present(i, &island.geom, &self.state, &self.config, hit)?;
        }
        Ok(())
    }

    fn notification_banner_size(&self) -> (f32, f32, f32) {
        let ui = self.config.ui;
        let (available, _) = self.overlay.available_size(0, self.config.top_gap());
        let (w, h) = ui.banner_px();
        (w.min(available), h, 20. * ui.banner_scale())
    }

    fn fit_panels(&mut self) {
        if self.notification_banner.as_ref().is_some_and(|(_, mode)| *mode == self.islands[0].mode) {
            let size = self.notification_banner_size();
            self.islands[0].morph(size);
        }
        for (i, island) in self.islands.iter_mut().enumerate() {
            let PillMode::Expanded(tab) = island.mode else { continue };
            let (aw, ah) = self.overlay.available_size(i, self.config.top_gap());
            let (mut w, h, r) = isle_render::layout::fit_size(tab, aw, ah);
            if self.config.ui.uniform_panels {
                w = w.max(isle_render::pages::compact_tray_width(&self.state)).min(aw.max(320.));
            }
            // Wide enough for the header's tabs and status side by side, as far as the screen allows.
            w = w.max(isle_render::layout::header_room(&self.state).min(aw.max(320.)));
            let radius = r * self.config.notch_curve as f32 / 75.0;
            island.morph((w, h, radius));
            let max_scroll = (isle_render::layout::content_height(tab, &self.state) - h).max(0.);
            island.scroll = island.scroll.clamp(0., max_scroll);
        }
    }
    fn follow_cursor_monitor(&mut self, now: Instant) -> Result<()> {
        #[cfg(windows)]
        if self.config.ui.follow_mouse && self.follow_monitor_at.is_none_or(|at| now >= at) {
            self.follow_monitor_at = Some(now + Duration::from_millis(400));
            if self.overlay.cursor_monitor_changed() {
                self.close_clipboard_search();
                self.close_ai_input();
                self.close_reply();
                self.stop_typing_reminder();
                self.unbind_phase8_windows();
                self.overlay.destroy_windows();
                self.overlay.create_windows()?;
                self.sync_islands();
                self.bind_phase8_windows();
                self.bind_shortcuts();
            }
        }
        let _ = now;
        Ok(())
    }

    fn apply_input(&mut self, input: UiInput) -> Result<()> {
        let selected=match input{UiInput::Click{window,..}|UiInput::PointerDown{window,..}|UiInput::RightClick{window,..}|UiInput::Drag{window,..}|UiInput::Wheel{window,..}|UiInput::Hover{window,..}|UiInput::Char{window,..}=>Some(window),_=>None};
        if let Some(i)=selected{if let Some(island)=self.islands.get(i){self.state.panel_scroll=island.scroll;self.state.mode=island.mode.clone();}}
        if let UiInput::Wheel{window,delta,x,y}=input { if delta != 0 && self.wheel_slider(window, delta, x, y) { return Ok(()) } }
        if let UiInput::Wheel{window,delta,..}=input {if let Some(island)=self.islands.get_mut(window){if let PillMode::Expanded(tab)=island.mode{if !(tab==TabPage::AiHub&&self.state.chat.open){let max=(isle_render::layout::content_height(tab,&self.state)-island.geom.height.target).max(0.);if max>0.{island.scroll=(island.scroll-delta as f32/120.*48.).clamp(0.,max);return Ok(())}}}}}

        match input {
            UiInput::Click { window, x, y } => {
                self.shelf_press = None;
                if self.volume_drag.is_some() {
                    self.drag_volume_to(window, x); self.volume_drag = None;
                    self.overlay.end_slider_drag(); return Ok(());
                }
                if let Some((held_in, from)) = self.tile_drag.take() {
                    self.state.control_drag = None;
                    let to = (held_in == window).then(|| self.control_under(window, x, y)).flatten();
                    if let Some(to) = to.filter(|to| *to != from) {
                        self.edit_controls(|tiles| {
                            let tile = tiles.remove(from);
                            tiles.insert(to.min(tiles.len()), tile);
                        });
                    }
                    return Ok(());
                }
                if self.islands.get(window).is_some_and(|i| i.mode == PillMode::Minimized) && self.config.ui.line_click {
                    let island = self.island(window);
                    island.manually_hidden = false;
                    island.mode = PillMode::CompactBar;
                    island.morph(COMPACT);
                    return Ok(());
                }
                let island = self.island(window);
                let (w, h, mode) = (island.geom.width.value, island.geom.height.value, island.mode.clone(),
                );
                self.state.mode = mode;
                let action = hit_test_action(x as i32, y as i32, CANVAS_MAX_WIDTH, w, h, self.config.top_gap(), &self.state,
                );
                if let Some(action) = action {
                    self.apply_ui_action_on(window, action);
                }
            }
            UiInput::RightClick { window, x, y } => {
                let island = self.island(window);
                let (w,h,mode) = (island.geom.width.value,island.geom.height.value,island.mode.clone(),
                );
                self.state.mode = mode;
                match hit_test_action(x as i32,y as i32,CANVAS_MAX_WIDTH,w,h,self.config.top_gap(),&self.state) {
                    Some(UiAction::TrayClick(index,_)) => self.apply_ui_action_on(window,UiAction::TrayClick(index,true)),
                    // Anywhere else on the pill: the settings (the mod's right click opens its menu).
                    Some(_) => self.open_settings(),
                    None => {}
                }
            }
            UiInput::PointerDown { window, x, y } => {
                self.volume_drag = None;
                self.overlay.end_slider_drag();
                let island = self.island(window);
                let (w,h,mode) = (island.geom.width.value, island.geom.height.value, island.mode.clone(),
                );
                self.state.mode = mode;
                self.shelf_press = compute_hitboxes(CANVAS_MAX_WIDTH as f32, w, h, self.config.top_gap(), &self.state)
                    .into_iter().find(|hb| hb.contains(x, y)).and_then(|hb| match hb.action {
                        UiAction::Phase8(isle_render::phase8_view::Phase8Action::ShelfRow(i)) => Some((i, x, y)),
                        _ => None,
                    });
                if self.state.control_editing {
                    // Port of ControlPanel's drag in edit mode: the control held follows to where it's let go.
                    let held = compute_hitboxes(CANVAS_MAX_WIDTH as f32, w, h, self.config.top_gap(), &self.state)
                        .into_iter().find(|hb| hb.contains(x, y)).and_then(|hb| match hb.action {
                            UiAction::ControlGrab(i) => Some(i),
                            _ => None,
                        });
                    if let Some(i) = held {
                        self.tile_drag = Some((window, i));
                        self.state.control_drag = Some((i, i));
                        return Ok(());
                    }
                }
                for hb in compute_hitboxes(CANVAS_MAX_WIDTH as f32,w,h,self.config.top_gap(),&self.state,
                ) {
                    if !hb.contains(x,y) { continue; }
                    let app_id = match hb.action {
                        UiAction::SetVolume(_) => None,
                        UiAction::AppVolume(i,_) => {
                            self.state.app_volumes.get(i).map(|a| a.id.clone())
                        }
                        _ => continue,
                    };
                    // Without capture the drag still follows the mouse inside the island; the click still counts.
                    if let Err(err) = self.overlay.begin_slider_drag(window) {
                        tracing::warn!("arrasto de volume: {err:#}");
                    }
                    self.volume_drag = Some(VolumeDrag { window, left: hb.x, width: hb.w, app_id,
                    });
                    self.drag_volume_to(window,x); break;
                }
            }
            UiInput::Drag { window, x, y } if self.tile_drag.is_some_and(|(w, _)| w == window) => {
                if let (Some((_, from)), Some(over)) = (self.tile_drag, self.control_under(window, x, y)) {
                    self.state.control_drag = Some((from, over));
                }
            }
            UiInput::Drag { x, y, .. } if self.shelf_press.is_some() => {
                // Past the system drag threshold the press becomes a drag out of the island (SHDoDragDrop).
                if let Some((i, x0, y0)) = self.shelf_press {
                    let (tx, ty) = drag_threshold();
                    if (x - x0).abs() > tx || (y - y0).abs() > ty {
                        self.shelf_press = None;
                        #[cfg(windows)]
                        self.drag_shelf_file(i);
                        #[cfg(not(windows))]
                        let _ = i;
                    }
                }
            }
            UiInput::Drag { window, x, .. } => self.drag_volume_to(window,x),
            UiInput::CancelDrag => { self.shelf_press = None; self.volume_drag = None; self.overlay.end_slider_drag(); self.tile_drag = None; self.state.control_drag = None; }
            UiInput::Wheel { window, delta, .. } if self.state.chat.open && self.islands.get(window).is_some_and(|i| i.mode == PillMode::Expanded(TabPage::AiHub)) =>
            {
                self.ai_scroll(if delta > 0 {3} else {-3})
            }
            UiInput::Wheel { window, delta, x, .. } if delta != 0 && self.config.ui.media_wheel
                && self.islands.get(window).is_some_and(|i| i.mode == PillMode::CompactBar
                    && isle_render::pages::compact_media_contains(&self.state, CANVAS_MAX_WIDTH as f32, i.geom.width.value, x)) =>
            {
                let key = if delta > 0 { MediaKey::Previous } else { MediaKey::Next };
                self.apply_ui_action_on(window, UiAction::Media(key));
            }
            // The closed island's wheel only: on an open panel a touchpad's swipe is a scroll, never the volume.
            UiInput::Wheel { window, delta, .. } if self.config.wheel_volume
                && !self.islands.get(window).is_some_and(|i| matches!(i.mode, PillMode::Expanded(_))) => self.send_audio(AudioCommand::Wheel(delta)),
            UiInput::Wheel { .. } => {}
            UiInput::Hover { window, inside } => {
                let options = self.config.ui;
                self.island(window).hover(inside, options);
            }
            UiInput::Char { window, ch } if self.clipboard_search_input(window, ch) => {},
            UiInput::Char { window, ch } if self.reminder_typing => {
                // A reminder's text, while its field has the keyboard in that island's Timer tab.
                if window != self.reply_window || self.islands.get(window).is_none_or(|i| i.mode != PillMode::Expanded(TabPage::Timer)) {
                    self.stop_typing_reminder();
                } else {
                    match ch {
                        '\r' => self.timer_action(window, isle_render::pages::TimerAction::Add),
                        '\u{1b}' => self.stop_typing_reminder(),
                        '\u{16}' => self.reminder_draft.paste(&if cfg!(test) { String::new() } else { notifications::clipboard_text() }),
                        c => self.reminder_draft.type_char(c),
                    }
                    self.refresh_timers(self.state.now_ms);
                }
            }
            UiInput::Char { window, ch } => {
                if self.chat_window.is_some() { self.ai_char(window,ch); }
                else {
                // Typing only goes to an open reply in that island's Avisos.
                if window != self.reply_window || self.islands.get(window).is_none_or(|i| i.mode != PillMode::Expanded(TabPage::Notifications)) {
                    self.close_reply();
                } else if let Some(reply) = &mut self.state.reply {
                    match ch {
                        '\r' => self.send_reply(),
                        '\u{1b}' => self.close_reply(),
                        '\u{16}' => reply.paste(&if cfg!(test) { String::new() } else { notifications::clipboard_text() }),
                        c => reply.type_char(c),
                    }
                }
                }
            }
            UiInput::OpenSettings => self.open_settings(),
            UiInput::ClipboardChanged => self.queue_clipboard_capture(),
            UiInput::DevicesChanged => self.refresh_usb(),
            UiInput::FilesOver { window, over } => self.files_over(window, over),
            UiInput::FilesDropped { window, paths } => { self.files_over(window, false); self.add_shelf_files(paths); }
            UiInput::SessionUnavailable => self.stop_shelf_share(),
            UiInput::Hotkey(id) => self.handle_island_hotkey(shortcuts::island_hotkey(id)),
            UiInput::KeyboardLost => {
                self.close_clipboard_search();
                self.close_ai_input();
                if self.state.reply.is_some() { self.close_reply(); }
                if self.reminder_typing { self.stop_typing_reminder(); self.refresh_timers(self.state.now_ms); }
            }
            UiInput::MonitorsChanged => {
                self.close_clipboard_search();
                self.close_ai_input();
                self.unbind_phase8_windows();
                self.overlay.destroy_windows();
                self.overlay.create_windows()?;
                self.sync_islands();
                self.bind_phase8_windows();
                self.bind_shortcuts();
            }
        }
        Ok(())
    }

    /// How long the UI thread may sleep: a frame while the spring moves, else until the next deadline,
    /// the next minute (the clock) or, while a card waits, a second (it may be withdrawn).
    /// The bar's clock shows seconds: the loop wakes each second, only then.
    fn clock_ticks_seconds(&self) -> bool {
        self.config.clock_seconds && self.config.bar_shown().contains(&isle_core::config::BarItem::Clock)
    }

    /// The chat input has the keyboard and its tab is on screen: its caret blinks.
    fn caret_focused(&self) -> bool {
        !self.state.chat.busy && self.state.chat.open && self.chat_window.and_then(|w| self.islands.get(w)).is_some_and(|i| i.mode == PillMode::Expanded(TabPage::AiHub))
    }

    fn sleep_for(&self, now: Instant, now_unix: u64) -> Duration {
        let next_minute = Duration::from_secs(60 - now_unix % 60);
        let mut sleep = next_minute;
        if self.caret_focused() || self.settings_typing() {
            let (blink, ms) = (isle_platform_win::caret_blink_ms(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64);
            if blink != u64::MAX { sleep = sleep.min(Duration::from_millis(blink - ms % blink + 5)); }
        }
        if let Some(at) = self.follow_monitor_at { sleep = sleep.min(at.saturating_duration_since(now)); }
        if let Some(at) = self.next_deadline() {
            sleep = sleep.min(at.saturating_duration_since(now));
        }
        if self.pending_reply.is_some() {
            sleep = sleep.min(Duration::from_secs(1));
        }
        // The bar's m:ss: the next whole second while a timer runs; nothing when none does.
        if self.running_timer() || self.clock_ticks_seconds() {
            let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
            sleep = sleep.min(Duration::from_millis(1000 - ms % 1000 + 5));
        }
        // The song's next line, and its progress, while a media tab shows them.
        let media_open = self.islands.iter().any(|i| i.mode == PillMode::Expanded(TabPage::Media));
        let compact_visible = self.islands.iter().any(|i| i.mode == PillMode::CompactBar);
        let download_meter_visible = self.islands.iter().any(|i| matches!(i.mode, PillMode::CompactBar | PillMode::Minimized));
        if download_meter_visible && isle_render::window::animations_enabled()
            && self.state.phase8.download_meter
            && self.state.phase8.downloads.is_some_and(|value| value.permille.is_none())
        {
            sleep = sleep.min(Duration::from_millis(50));
        }
        // The pill's download fill: a frame while it tweens or fades out; nothing once settled.
        if download_meter_visible && isle_render::window::animations_enabled()
            && self.state.phase8.download_meter && self.state.phase8.download_animation
            && self.state.phase8.download_fill.is_animating(self.state.now_ms)
        {
            sleep = sleep.min(Duration::from_millis(16));
        }
        // The Downloads tab's bars: the fill's tween, or a sliding marker without a total; only while it shows.
        let p8 = &self.state.phase8;
        if self.islands.iter().any(|i| i.mode == PillMode::Expanded(TabPage::Downloads)) && isle_render::window::animations_enabled()
            && (p8.bar_sliding() || (p8.download_animation && p8.download_fill.is_animating(self.state.now_ms)))
        {
            sleep = sleep.min(Duration::from_millis(if p8.bar_sliding() { 50 } else { 16 }));
        }
        // The music's bars on the open panel's header, while something plays.
        if isle_render::window::animations_enabled() && self.state.modules.media
            && self.state.media.as_ref().is_some_and(|m| m.playing)
            && self.islands.iter().any(|i| matches!(i.mode, PillMode::Expanded(_)))
        {
            sleep = sleep.min(Duration::from_millis(50));
        }
        // A title scrolling on the bar: every animation frame, as mac-island 3.1.1 redraws it (25 a
        // second moved it 1.4 px at a time, visibly stepping).
        if compact_visible && isle_render::window::animations_enabled() && self.islands.iter().any(|island| {
            island.mode == PillMode::CompactBar && isle_render::pages::compact_marquee_needed(&self.state, island.geom.width.target)
        }) {
            sleep = sleep.min(FRAME);
        }
        let lyrics_on_bar = compact_visible && self.config.ui.media_lyric_title && self.config.lyrics;
        if (media_open || lyrics_on_bar) && self.state.media.as_ref().is_some_and(|m| m.playing) {
            let now_ms = (now_unix * 1000) as i64;
            let line = self.next_lyric_at(now_ms).map(|at| Duration::from_millis((at - now_ms).max(50) as u64));
            sleep = sleep.min(line.unwrap_or(Duration::from_secs(1)).min(Duration::from_secs(1)),
            );
        }
        sleep
    }

    /// The island: windows on this (UI) thread, sleeping until something happens; the services (hook
    /// server, weather, calendar) on `rt`'s threads, waking it with what they bring.
    pub fn run(mut self, rt: &tokio::runtime::Runtime) -> Result<()> {
        init_dpi();
        // Before the windows; undone when run returns.
        #[cfg(windows)]
        let _ole = isle_platform_win::clipboard::Ole::initialize();
        self.overlay.create_windows()?;
        self.sync_islands();
        let waker = Waker::for_this_thread();
        let (tx, rx) = std::sync::mpsc::channel::<AppEvent>();
        let (audio, chat) = spawn_services(rt, &self.config, self.state.dnd_active, tx.clone(), waker.clone())?;
        self.services = Some(Services { rt: rt.handle().clone(), tx: tx.clone(), waker: waker.clone() });
        self.start_icons();
        self.start_modules();
        self.sync_update_check();
        self.sync_autostart();
        self.apply_phase8_config(None);
        self.bind_shortcuts();
        if self.settings.at_start {
            self.open_settings();
        }
        self.chat_tx = Some(chat);
        self.audio_tx = Some(audio);

        let unix_now = || {
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
        };
        let mut last_minute = None;
        let mut pacer = FramePacer::new(Instant::now());
        loop {
            if !self.overlay.pump() {
                return Ok(());
            }
            for input in self.overlay.take_inputs() {
                self.apply_input(input)?;
            }
            let mut island_changed = false;
            while let Ok(event) = rx.try_recv() {
                island_changed = true;
                match event {
                    AppEvent::Hook(ev) => self.handle_hook_event(ev, unix_now()),
                    AppEvent::Chat(update) => self.apply_ai_update(update),
                    AppEvent::Usage(providers) => self.apply_usage(providers, unix_now()),
                    AppEvent::Media(status) => {
                        if let Some(query) = self.apply_media(status) {
                            let (tx, waker) = (tx.clone(), waker.clone());
                            rt.spawn(async move {
                                let _ = tx.send(AppEvent::Lyrics(lyrics::fetch(&query).await));
                                waker.wake();
                            });
                        }
                    }
                    AppEvent::Tray(snapshot) => {
                        self.state.tray = snapshot.items;
                        self.state.tray_bridge = snapshot.bridge;
                        self.state.tray_error = snapshot.error;
                        self.state.tray_page = isle_render::pages::tray_page(&self.state);
                    }
                    AppEvent::Audio(AudioEvent::Status(status)) => self.apply_audio(status),
                    AppEvent::Notifications(NotificationEvent::Batch { items, initial, present, badged,
                    }) => {
                        self.apply_notifications(items,initial);
                        self.state.notifications.retain_present(&present,&badged);
                        self.state.notification_page = self.state.notification_page.min(self.state.notifications.last_page());
                        tracing::info!("avisos: {} na ilha, {} não lidos", self.state.notifications.ordered().len(), self.state.notifications.unread_count());
                    }
                    AppEvent::Notifications(NotificationEvent::AppIcon{app_id,image}) => {
                        self.apply_notification_icon(&app_id,image);
                    }
                    AppEvent::Notifications(NotificationEvent::Status(error)) => {
                        if let Some(error) = &error { tracing::warn!("{error}"); }
                        self.state.notification_error = error
                    }
                    AppEvent::Notifications(NotificationEvent::Open) if self.config.modules.notifications => self.toggle_notifications(),
                    AppEvent::Notifications(NotificationEvent::Open) => {},
                    AppEvent::Notifications(NotificationEvent::Minimize) => self.handle_island_hotkey(1),
                    AppEvent::Controls(ControlEvent::Status(status)) => {
                        self.state.wifi_name = status.wifi_name;
                        self.state.wifi_connected = status.wifi_connected;
                        self.state.bluetooth_on = status.bluetooth_on;
                        self.state.night_light_on = status.night_light_on;
                    }
                    AppEvent::Audio(AudioEvent::Error(err)) | AppEvent::Controls(ControlEvent::Error(err)) => {
                        tracing::warn!("{err}"); self.state.control_error = Some(err);
                    }
                    AppEvent::Lyrics(found) => self.apply_lyrics(found),
                    AppEvent::Weather(Ok(weather)) => { self.state.weather = Some(weather); self.state.weather_error = None; }
                    AppEvent::Weather(Err(error)) => { tracing::warn!("clima: {error}"); self.state.weather_error = Some(error); }
                    AppEvent::Holidays(json) => self.load_holidays(&json),
                    AppEvent::Calendar(raw) => {
                        self.load_ics(&raw);
                        last_minute = None;
                    }
                    AppEvent::Failed(service) => return Err(anyhow::anyhow!("{service} parou")),
                    AppEvent::Places(answer) => self.apply_places(answer),
                    AppEvent::BackupFile { save, path } => self.backup_picked(save, path),
                    AppEvent::Phase8(event) => self.apply_phase8_event(event),
                    AppEvent::Icons(event) => self.apply_icons(event),
                    AppEvent::Update { checked, manual } => self.apply_update(checked, manual),
                    AppEvent::Installer(result) => self.run_installer(result),
                }
            }

            self.sync_ai_input();
            self.sync_clipboard_search();
            let (now, now_unix) = (Instant::now(), unix_now());
            let tick = if self.clock_ticks_seconds() { now_unix } else { now_unix / 60 };
            if last_minute != Some(tick) {
                last_minute = Some(tick);
                island_changed = true;
                self.tick_runtime_domain(now_unix);
            }
            self.state.now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64;
            let blink = isle_platform_win::caret_blink_ms();
            self.state.chat.caret = self.caret_focused().then(|| blink == u64::MAX || self.state.now_ms as u64 / blink % 2 == 0);
            self.blink_settings_caret(self.state.now_ms as u64, blink);
            self.refresh_timers(self.state.now_ms);
            self.refresh_lyric(self.state.now_ms);
            self.expire_stale_permission();
            self.on_deadlines(now);
            self.sync_control_panel();
            self.sync_tray_panel();
            self.fit_panels();
            self.follow_cursor_monitor(now)?;
            if island_changed {
                self.settings_follow_island();
            }
            self.pump_settings();
            let dt = pacer.dt(now);
            for island in &mut self.islands {
                if !island.geom.is_settled() {
                    island.geom.tick(dt);
                }
            }
            self.present()?;

            let now = Instant::now();
            let moving = self.islands.iter().any(|i| !i.geom.is_settled());
            let sleep = pacer.wait(now, moving, self.sleep_for(now, unix_now()));
            // RUST_LOG=cimori::frame=debug: each frame's step, then (window.rs) its drawing and its trip to the screen.
            tracing::debug!(target: "cimori::frame", "passo {:.1} ms, espera {:.1} ms{}", dt * 1e3, sleep.as_secs_f64() * 1e3, if moving { ", animando" } else { "" });
            waker.wait(Some(sleep));
        }
    }
}

/// When the springs move and by how much: a frame every FRAME while one moves, by the time that passed.
struct FramePacer {
    last_frame: Instant,
    next_frame: Instant,
    moving: bool,
}

impl FramePacer {
    fn new(now: Instant) -> Self {
        Self { last_frame: now, next_frame: now, moving: false }
    }

    /// The seconds the springs move on this frame: the time since the last frame while one moves; one
    /// just set off moves one frame (the loop may have slept for minutes: that landed the pill at once).
    fn dt(&mut self, now: Instant) -> f32 {
        let dt = now.saturating_duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        if self.moving { return dt; }
        self.next_frame = now;
        dt.min(FRAME.as_secs_f32())
    }

    /// How long to wait after a frame: `idle` (the next deadline) unless a spring still moves, then until
    /// the next frame, FRAME after the last one started (port of mac-island 3.1.1's WaitForAnimationFrame:
    /// a late frame starts the next interval from now).
    fn wait(&mut self, now: Instant, moving: bool, idle: Duration) -> Duration {
        self.moving = moving;
        if !moving { return idle; }
        self.next_frame = (self.next_frame + FRAME).max(now);
        idle.min(self.next_frame - now)
    }
}

/// What the services bring to the UI thread.
enum AppEvent {
    Icons(icon_packs::Event),
    Chat(isle_ai::assistant::Update),
    Tray(TraySnapshot),
    Notifications(NotificationEvent),
    Hook(HookEvent),
    Audio(AudioEvent),
    Controls(ControlEvent),
    Media(Option<MediaStatus>),
    Lyrics(Lyrics),
    Usage(Vec<isle_core::state::UsageProvider>),
    /// The forecast, or why it didn't come (the last one stays).
    Weather(Result<isle_core::weather::Weather, String>),
    Calendar(String),
    Holidays(String),
    Failed(&'static str),
    /// Places found by the settings' city search.
    Places(Result<Vec<(String, f64, f64)>, String>),
    /// The backup file picked in Windows' dialog (`save`: export), None if cancelled.
    BackupFile { save: bool, path: Option<std::path::PathBuf> },
    Phase8(phase8::Event),
    /// What GitHub said of the latest release; `manual`: "Buscar atualizações" asked.
    Update { checked: release::Checked, manual: bool },
    /// The new version's installer, downloaded and checked (or why not).
    Installer(Result<std::path::PathBuf, String>),
}

/// Where a banner's click leads: a tab of the island, or one of the settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Opens {
    Tab(TabPage),
    Settings(isle_core::prefs::SettingsTab),
}

fn send(tx: &std::sync::mpsc::Sender<AppEvent>, waker: &Waker, event: AppEvent) {
    let _ = tx.send(event);
    waker.wake();
}

/// Where module services start, kept so a module turned on in the settings starts at once.
#[derive(Clone)]
struct Services {
    rt: tokio::runtime::Handle,
    tx: std::sync::mpsc::Sender<AppEvent>,
    waker: Waker,
}

/// What every module needs: the AI chat, the agents' server and the volume.
fn spawn_services(
    rt: &tokio::runtime::Runtime,
    cfg: &IsleConfig,
    dnd: bool,
    tx: std::sync::mpsc::Sender<AppEvent>,
    waker: Waker,
) -> Result<(Sender<AudioCommand>, tokio::sync::mpsc::UnboundedSender<isle_ai::assistant::Command>)> {
    let chat = {
        let (tx, waker) = (tx.clone(), waker.clone());
        isle_ai::assistant::spawn(rt, cfg.opencode_api_key.clone(), move |update| send(&tx, &waker, AppEvent::Chat(update)))
    };
    let bind_addr = format!("0.0.0.0:{}", cfg.hook_port);
    let (server, mut hook_rx) = rt.block_on(HookServer::spawn(&bind_addr, dnd, cfg.hooks_local_only))?;
    {
        let (tx, waker) = (tx.clone(), waker.clone());
        rt.spawn(async move {
            let _server = server;
            while let Some(ev) = hook_rx.recv().await {
                send(&tx, &waker, AppEvent::Hook(ev));
            }
            send(&tx, &waker, AppEvent::Failed("o servidor de hooks"));
        });
    }
    let audio = audio_osd::spawn(move |event| send(&tx, &waker, AppEvent::Audio(event)));
    Ok((audio, chat))
}

impl IsleApp {
    /// Starts the service of each module that is on and not started yet (a module turned off keeps
    /// its service until Cimori opens again: the settings say so).
    fn start_modules(&mut self) {
        let Some(Services { rt, tx, waker }) = self.services.clone() else { return };
        let m = self.config.modules;
        let wanted: [(&'static str, bool); 7] = [
            ("media", m.media),
            ("usage", m.ai && (self.config.ai_claude_enabled || self.config.ai_codex_enabled)),
            ("weather", m.weather),
            ("tray", m.tray),
            ("control", m.control),
            ("notifications", m.notifications),
            ("agenda", m.agenda),
        ];
        for (name, on) in wanted {
            if !on || self.started.contains(&name) {
                continue;
            }
            self.started.push(name);
            let (tx, waker) = (tx.clone(), waker.clone());
            match name {
                "media" => {
                    // The session manager's first answer is capped at 2 s; right after login it can be slower.
                    std::thread::spawn(move || loop {
                        let (tx, waker) = (tx.clone(), waker.clone());
                        match media::watch(move |status| send(&tx, &waker, AppEvent::Media(status))) {
                            Ok(_watch) => loop {
                                std::thread::park();
                            },
                            Err(err) => {
                                tracing::warn!("mídia do Windows indisponível, nova tentativa em 30 s: {err:#}");
                                std::thread::sleep(Duration::from_secs(30));
                            }
                        }
                    });
                }
                "usage" => {
                    let renew = self.config.claude_renew_login;
                    self.usage_task = Some(rt.spawn(isle_ai::usage::run(renew, move |providers| send(&tx, &waker, AppEvent::Usage(providers)))));
                }
                "weather" => {
                    let weather = self.config.weather.clone();
                    // The mod's: at start, then every 30 minutes; a failure keeps the last forecast.
                    self.weather_task = Some(rt.spawn(async move {
                        let url = isle_core::weather::forecast_url(weather.latitude, weather.longitude);
                        loop {
                            let result = match isle_platform_win::weather::fetch_url_text(&url).await {
                                Ok(json) => isle_core::weather::Weather::from_open_meteo(&weather.label, &json)
                                    .map_err(|e| trf("weather.bad_answer", &[("err", &e)])),
                                Err(e) => Err(trf("weather.unavailable", &[("err", &e)])),
                            };
                            send(&tx, &waker, AppEvent::Weather(result));
                            tokio::time::sleep(std::time::Duration::from_secs(isle_core::weather::REFRESH_SECS)).await;
                        }
                    }));
                }
                "tray" => {
                    self.tray_service = Some(tray::spawn(self.config.pinned_tray_apps.clone(), move |snapshot| {
                        send(&tx, &waker, AppEvent::Tray(snapshot))
                    }));
                }
                "control" => {
                    self.controls_tx = Some(system_controls::spawn(move |event| send(&tx, &waker, AppEvent::Controls(event))));
                }
                "notifications" => {
                    let icons = self.config.notification_icons;
                    notifications::spawn(false, icons, move |event| send(&tx, &waker, AppEvent::Notifications(event)));
                }
                "agenda" => {
                    // Nager.Date, as the mod: this year's and next year's holidays, once.
                    if self.config.holidays {
                        let (tx, waker) = (tx.clone(), waker.clone());
                        let country = self.config.holidays_country_or(isle_platform_win::locale::region_code());
                        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
                        let year = civil_from_days((now + local_offset()).div_euclid(86_400)).0;
                        self.agenda_tasks.push(rt.spawn(async move {
                            for year in [year, year + 1] {
                                let url = format!("https://date.nager.at/api/v3/PublicHolidays/{year}/{country}");
                                if let Ok(json) = isle_platform_win::weather::fetch_url_text(&url).await {
                                    send(&tx, &waker, AppEvent::Holidays(json));
                                }
                            }
                        }));
                    }
                    if let Some(url) = self.config.calendar_ics_url.clone() {
                        // Read again every 15 minutes, as the mod reads its agendas.
                        self.agenda_tasks.push(rt.spawn(async move {
                            loop {
                                match isle_platform_win::weather::fetch_url_text(&url).await {
                                    Ok(text) => send(&tx, &waker, AppEvent::Calendar(text)),
                                    Err(err) => tracing::warn!("agenda: {err:#}"),
                                }
                                tokio::time::sleep(Duration::from_secs(15 * 60)).await;
                            }
                        }));
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tray_panel_opens_on_clicked_monitor_and_permission_card_is_preserved() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.islands.push(Presence::new());
        app.apply_ui_action_on(1,UiAction::ToggleTray);
        assert_eq!(app.islands[1].mode,PillMode::Expanded(TabPage::Tray));
        assert_eq!(app.islands[0].mode,PillMode::CompactBar);
        assert_eq!(app.islands[1].geom.height.target,
            isle_render::layout::expanded_size(TabPage::Tray).1
        );
        app.islands[0].mode = PillMode::PermissionPrompt;
        app.apply_ui_action_on(0,UiAction::ToggleTray);
        assert_eq!(app.islands[0].mode,PillMode::PermissionPrompt);
    }
    use isle_ai::agents::{Event, Request};
    use isle_core::state::AiProviderKind;

    /// Clicks a button of the card on screen, as long after it showed as a person would.
    pub(super) fn answer(app: &mut IsleApp, decision: PermissionDecision) {
        app.permission_shown -= PERMISSION_CLICK_GUARD;
        let card = app.state.pending_permission.as_ref().map_or(0, |c| c.key());
        app.apply_ui_action(UiAction::Permission(decision, card));
    }

    pub(super) fn ask(reply: oneshot::Sender<PermissionDecision>) -> HookEvent {
        HookEvent::Ask {
            request: Request {
                id: "claude-0".into(),
                provider: "claude".into(),
                project: "isle-win".into(),
                tool: "Bash".into(),
                input: "cargo test".into(),
                ..Request::default()
            },
            reply,
        }
    }

    fn agent(kind: &str, message: &str) -> HookEvent {
        HookEvent::Agent(Event {
            provider: "claude".into(),
            kind: kind.into(),
            project: "isle-win".into(),
            message: message.into(),
            ..Event::default()
        })
    }

    #[test]
    fn claudes_own_permission_notice_does_not_cover_the_card() {
        let mut app = IsleApp::new(IsleConfig::default());
        let (tx, _rx) = oneshot::channel();
        app.handle_hook_event(ask(tx), 1000);
        app.handle_hook_event(agent("permission", "Claude needs your permission to use Bash"), 1000,
        );
        assert_eq!(app.islands[0].mode, PillMode::PermissionPrompt);
        let card = app.state.pending_permission.as_ref().unwrap();
        assert_eq!((card.agent.as_str(), card.command.as_str()), ("Claude Code · isle-win", "cargo test"));
    }

    #[test]
    fn a_waiting_card_outranks_banners_that_wait_their_turn_behind_it() {
        let mut app = IsleApp::new(IsleConfig::default());
        let (tx, mut rx) = oneshot::channel();
        app.handle_hook_event(ask(tx), 1000);
        app.handle_hook_event(agent("done", "Fixed the build."), 1000);
        app.handle_hook_event(HookEvent::QuotaSync { provider: isle_core::state::AiProviderKind::ClaudeCode, used_pct: 96., reset_at_unix: None }, 1000);
        let later = Instant::now() + Duration::from_secs(60);
        app.on_deadlines(later);
        assert_eq!(app.islands[0].mode, PillMode::PermissionPrompt, "the card stays until it is answered");
        answer(&mut app, PermissionDecision::Allow);
        assert_eq!(rx.try_recv().unwrap(), PermissionDecision::Allow);
        app.on_deadlines(later);
        assert!(matches!(&app.islands[0].mode, PillMode::AlertBanner { title, .. } if title == "Claude Code concluiu · isle-win"),
            "{:?}", app.islands[0].mode);
    }

    #[test]
    fn a_click_on_the_card_off_its_buttons_keeps_it_waiting() {
        let mut app = IsleApp::new(IsleConfig::default());
        let (tx, mut rx) = oneshot::channel();
        app.handle_hook_event(ask(tx), 1000);
        let (w, h) = (app.islands[0].geom.width.target, app.islands[0].geom.height.target);
        app.islands[0].geom.width.value = w;
        app.islands[0].geom.height.value = h;
        app.state.mode = app.islands[0].mode.clone();
        let boxes = compute_hitboxes(CANVAS_MAX_WIDTH as f32, w, h, app.config.top_gap(), &app.state);
        let body = boxes.iter().find(|b| matches!(b.action, UiAction::PermissionFocus(_))).unwrap();
        let allow = boxes.iter().find(|b| matches!(b.action, UiAction::Permission(PermissionDecision::Allow, _))).unwrap();
        // The buttons still answer where they are; the rest of the card is the agent's window.
        assert!(matches!(hit_test_action((allow.x + 4.) as i32, (allow.y + 4.) as i32, CANVAS_MAX_WIDTH, w, h, app.config.top_gap(), &app.state),
            Some(UiAction::Permission(..))));
        app.permission_shown -= PERMISSION_CLICK_GUARD;
        app.apply_input(UiInput::Click { window: 0, x: body.x + 30., y: body.y + 14. }).unwrap();
        assert!(rx.try_recv().is_err());
        assert_eq!(app.islands[0].mode, PillMode::PermissionPrompt);
    }

    #[test]
    fn a_finished_agent_shows_a_banner_with_its_project() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.handle_hook_event(agent("done", "Fixed the build."), 1000);
        assert_eq!(
            app.islands[0].mode,
            PillMode::AlertBanner {
                title: "Claude Code concluiu · isle-win".into(),
                subtitle: "Fixed the build.".into(),
            }
        );
    }

    #[test]
    fn app_handles_permission_prompt_and_resolves_via_ui_action() {
        let mut app = IsleApp::new(IsleConfig::default());
        let (tx, mut rx) = oneshot::channel();
        app.handle_hook_event(
            ask(tx),
            1000);
        assert_eq!(app.islands[0].mode, PillMode::PermissionPrompt);
        answer(&mut app, PermissionDecision::Allow);
        assert_eq!(rx.try_recv().unwrap(), PermissionDecision::Allow);
        assert_eq!(app.islands[0].mode, PillMode::CompactBar);
    }

    #[test]
    fn asks_queue_up_to_eight_and_advance_oldest_first() {
        let mut app = IsleApp::new(IsleConfig::default());
        let mut rxs = vec![];
        for _ in 0..10 {
            let (tx, rx) = oneshot::channel();
            app.handle_hook_event(ask(tx), 1000);
            rxs.push(rx);
        }
        // 8 in all: the card plus 7 waiting; the last two were dropped (they ask in their terminals).
        assert_eq!(app.state.permission_waiting, 7);
        assert!(rxs[8].try_recv().is_err() && rxs[9].try_recv().is_err());
        answer(&mut app, PermissionDecision::Allow);
        assert_eq!(rxs[0].try_recv().unwrap(), PermissionDecision::Allow);
        assert_eq!((app.islands[0].mode.clone(), app.state.permission_waiting), (PillMode::PermissionPrompt, 6));
        answer(&mut app, PermissionDecision::Deny);
        assert_eq!(rxs[1].try_recv().unwrap(), PermissionDecision::Deny);
    }

    #[test]
    fn a_double_click_answers_only_the_card_it_was_on() {
        let mut app = IsleApp::new(IsleConfig::default());
        let (a, mut ra) = oneshot::channel();
        let (b, mut rb) = oneshot::channel();
        for (id, tx) in [("claude-1", a), ("claude-2", b)] {
            let HookEvent::Ask { mut request, reply } = ask(tx) else { unreachable!() };
            request.id = id.into();
            app.handle_hook_event(HookEvent::Ask { request, reply }, 1000);
        }
        let first = app.state.pending_permission.as_ref().unwrap().key();
        answer(&mut app, PermissionDecision::Allow);
        assert_eq!(ra.try_recv().unwrap(), PermissionDecision::Allow);
        // The second click lands on the next card: too soon, and with the first card's key.
        app.apply_ui_action(UiAction::Permission(PermissionDecision::Allow, first));
        let second = app.state.pending_permission.as_ref().unwrap().key();
        app.apply_ui_action(UiAction::Permission(PermissionDecision::Allow, second));
        assert!(rb.try_recv().is_err() && app.islands[0].mode == PillMode::PermissionPrompt);
        app.permission_shown -= PERMISSION_CLICK_GUARD;
        app.apply_ui_action(UiAction::Permission(PermissionDecision::Allow, first));
        assert!(rb.try_recv().is_err());
        app.apply_ui_action(UiAction::Permission(PermissionDecision::Deny, second));
        assert_eq!(rb.try_recv().unwrap(), PermissionDecision::Deny);
    }

    #[test]
    fn a_queued_ask_that_gave_up_is_dropped() {
        let mut app = IsleApp::new(IsleConfig::default());
        let (a, _ra) = oneshot::channel();
        let (b, rb) = oneshot::channel();
        let (c, mut rc) = oneshot::channel();
        for tx in [a, b, c] { app.handle_hook_event(ask(tx), 1000); }
        drop(rb);
        assert!(app.expire_stale_permission() && app.state.permission_waiting == 1);
        answer(&mut app, PermissionDecision::Allow);
        answer(&mut app, PermissionDecision::Always);
        assert_eq!(rc.try_recv().unwrap(), PermissionDecision::Always);
        assert_eq!(app.islands[0].mode, PillMode::CompactBar);
    }

    #[test]
    fn permission_card_closes_when_hook_times_out() {
        let mut app = IsleApp::new(IsleConfig::default());
        let (tx, rx) = oneshot::channel();
        app.handle_hook_event(
            ask(tx),
            1000);
        assert!(!app.expire_stale_permission());
        assert_eq!(app.islands[0].mode, PillMode::PermissionPrompt);

        drop(rx); // hook server gave up after its 25s timeout
        assert!(app.expire_stale_permission());
        assert_eq!(app.islands[0].mode, PillMode::CompactBar);
        assert!(app.state.pending_permission.is_none());
    }

    #[test]
    fn app_surfaces_native_claude_de_volta_banner_on_quota_reset() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.handle_hook_event(
            HookEvent::QuotaSync {
                provider: AiProviderKind::ClaudeCode,
                used_pct: 96.0,
                reset_at_unix: Some(2000),
            },
            1500,
        );
        app.handle_hook_event(
            HookEvent::QuotaSync {
                provider: AiProviderKind::ClaudeCode,
                used_pct: 0.0,
                reset_at_unix: Some(20000),
            },
            2005,
        );
        assert_eq!(
            app.islands[0].mode,
            PillMode::AlertBanner {
                title: "Claude de volta!".to_string(),
                subtitle: "Cota renovada e pronta para uso".to_string(),
            }
        );
    }

    #[test]
    fn the_compact_bar_shows_weekday_and_date_in_portuguese() {
        let date = |unix: i64| isle_core::clock::date_text(unix + local_offset(), &IsleConfig::default());
        assert_eq!(date(1794361260), "ter 10 nov");
        // Still the 31st in São Paulo, though already New Year in UTC.
        assert_eq!(date(1767236340), "qua 31 dez");
        assert_eq!(date(1835449200), "ter 29 fev");
        let mut app = IsleApp::new(IsleConfig::default());
        app.tick_runtime_domain(1794361260);
        assert_eq!((app.state.clock_text.as_str(), app.state.date_text.as_str()), ("22:41", "ter 10 nov"));
    }

    #[test]
    fn app_updates_clock_and_calendar_countdown() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.load_ics("BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Deploy Prod\r\nDTSTART:20261110T220000Z\r\nEND:VEVENT\r\nEND:VCALENDAR");
        let ev_epoch = (isle_core::agenda::days_from_civil(2026, 11, 10) * 86_400 + 22 * 3600) as u64;
        app.tick_runtime_domain(ev_epoch - 300);

        assert_ne!(app.state.clock_text, "20:00");
        assert_eq!(app.state.next_countdown_text.as_deref(), Some("Deploy Prod (5m)"));
        // More than 12 hours away the bar shows the date instead (as Mac Island & Dock), and a past
        // event leaves no stale countdown behind.
        app.tick_runtime_domain(ev_epoch - 13 * 3600);
        assert_eq!(app.state.next_countdown_text, None);
        app.tick_runtime_domain(ev_epoch - 3 * 3600);
        assert_eq!(app.state.next_countdown_text.as_deref(), Some("Deploy Prod (3h)"));
        app.tick_runtime_domain(ev_epoch + 60);
        assert_eq!(app.state.next_countdown_text, None);
    }

    #[test]
    fn the_next_slot_follows_the_countdown_settings() {
        let ics = concat!("BEGIN:VCALENDAR
BEGIN:VEVENT
SUMMARY:Deploy Prod
", "DTSTART:20261110T220000Z
END:VEVENT
END:VCALENDAR");
        let ev_epoch = (isle_core::agenda::days_from_civil(2026, 11, 10) * 86_400 + 22 * 3600) as u64;
        let mut cfg = IsleConfig::default();
        cfg.countdown_hours = 2;
        let mut app = IsleApp::new(cfg.clone());
        app.load_ics(ics);
        app.tick_runtime_domain(ev_epoch - 3 * 3600);
        assert_eq!(app.state.next_countdown_text, None, "3 h away, the window is 2 h");
        app.tick_runtime_domain(ev_epoch - 3600);
        assert_eq!(app.state.next_countdown_text.as_deref(), Some("Deploy Prod (1h)"));
        cfg.countdown_events = false;
        let mut app = IsleApp::new(cfg);
        app.load_ics(ics);
        app.tick_runtime_domain(ev_epoch - 300);
        assert_eq!(app.state.next_countdown_text, None);
    }

    #[test]
    fn a_timer_left_out_of_the_next_slot_still_shows_on_the_timer_item() {
        use isle_core::timers::TimerMode;
        use isle_render::pages::TimerAction;
        let mut cfg = IsleConfig::default();
        cfg.countdown_timer = false;
        let mut app = IsleApp::new(cfg);
        app.apply_ui_action(UiAction::Timer(TimerAction::Mode(TimerMode::Countdown)));
        app.apply_ui_action(UiAction::Timer(TimerAction::Preset(10)));
        app.apply_ui_action(UiAction::Timer(TimerAction::Toggle));
        app.refresh_timers(60_000);
        assert_eq!(app.state.next_countdown_text, None);
        assert_eq!(app.state.timer_text.as_deref(), Some("Timer 9:00"));
    }

    #[test]
    fn the_bar_comes_from_the_settings() {
        use isle_core::config::BarItem;
        let mut cfg = IsleConfig::default();
        cfg.bar_items = vec![BarItem::Weather, BarItem::Clock];
        cfg.bar_quota_strip = false;
        let app = IsleApp::new(cfg);
        assert_eq!(app.state.bar, vec![BarItem::Weather, BarItem::Clock]);
        assert!(!app.state.bar_quota_strip);
        let mut cfg = IsleConfig::default();
        cfg.bar_items.clear();
        assert_eq!(IsleApp::new(cfg).state.bar, vec![BarItem::Clock], "never an empty bar");
    }

    #[test]
    fn a_module_turned_off_leaves_the_navigation_and_its_open_panel() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.apply_ui_action(UiAction::SwitchTab(TabPage::Weather));
        let old = app.config.clone();
        app.config.modules.weather = false;
        app.apply_config(&old, false);
        assert!(!isle_render::layout::visible_tabs(&app.state.modules).contains(&TabPage::Weather));
        assert_eq!(app.islands[0].mode, PillMode::Expanded(TabPage::Media), "the first tab left, Música");
        app.apply_ui_action(UiAction::SwitchTab(TabPage::Weather));
        assert_ne!(app.islands[0].mode, PillMode::Expanded(TabPage::Weather), "a hidden tab does not open");
    }

    #[test]
    fn the_island_opens_on_its_chosen_tab_else_the_first_module_left() {
        let mut cfg = IsleConfig::default();
        cfg.modules.media = false;
        let mut app = IsleApp::new(cfg.clone());
        app.apply_ui_action(UiAction::ToggleCollapse);
        assert_eq!(app.islands[0].mode, PillMode::Expanded(TabPage::Notifications), "Música off: the first tab left");
        cfg.modules.media = true;
        let mut app = IsleApp::new(cfg.clone());
        app.apply_ui_action(UiAction::ToggleCollapse);
        assert_eq!(app.islands[0].mode, PillMode::Expanded(TabPage::Media), "isle opens on Music");
        cfg.ui.open_tab = TabPage::AiHub;
        let mut app = IsleApp::new(cfg);
        app.state.chat.open = true;
        app.apply_ui_action(UiAction::ToggleCollapse);
        assert_eq!(app.islands[0].mode, PillMode::Expanded(TabPage::AiHub));
        assert!(!app.state.chat.open, "the Uso page, not the assistant");
    }

    #[test]
    fn the_star_opens_the_assistant_and_its_arrow_goes_back_to_the_page() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.apply_ui_action(UiAction::SwitchTab(TabPage::Timer));
        app.apply_ui_action(UiAction::AiOpen);
        assert_eq!(app.islands[0].mode, PillMode::Expanded(TabPage::AiHub));
        assert!(app.state.chat.open);
        app.apply_ui_action(UiAction::AiIdea(1));
        assert_eq!(app.state.chat.input.text, "Instale ", "an idea starts the words");
        app.apply_ui_action(UiAction::AiBack);
        assert!(!app.state.chat.open);
        assert_eq!(app.islands[0].mode, PillMode::Expanded(TabPage::Timer));
        app.apply_ui_action(UiAction::AiOpen);
        app.apply_ui_action(UiAction::SwitchTab(TabPage::AiHub));
        assert!(!app.state.chat.open, "the Uso tab is the usage page");
    }

    #[test]
    fn with_every_module_off_the_pill_stays_closed() {
        let mut cfg = IsleConfig::default();
        cfg.modules = isle_core::config::Modules { ai: false, control: false, agenda: false, weather: false,
            media: false, notifications: false, timer: false, tray: false, ..Default::default() };
        let mut app = IsleApp::new(cfg);
        app.apply_ui_action(UiAction::ToggleCollapse);
        assert_eq!(app.islands[0].mode, PillMode::CompactBar);
    }

    #[test]
    fn a_changed_bar_reaches_the_pill_without_saving_when_asked_not_to() {
        use isle_core::config::BarItem;
        let mut app = IsleApp::new(IsleConfig::default());
        let old = app.config.clone();
        app.config.bar_items = vec![BarItem::Weather];
        app.apply_config(&old, false);
        assert_eq!(app.state.bar, vec![BarItem::Weather]);
    }

    #[test]
    fn agenda_merges_events_and_holidays_and_opens_on_today() {
        use isle_core::agenda::days_from_civil;
        let mut app = IsleApp::new(IsleConfig::default());
        let noon = (days_from_civil(2026, 11, 11) * 86_400 + 15 * 3600) as u64;
        app.tick_runtime_domain(noon);
        app.load_ics("BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:x\r\nDTSTART;VALUE=DATE:20261104\r\nRRULE:FREQ=MONTHLY;BYMONTHDAY=4\r\nSUMMARY:Aluguel\r\nEND:VEVENT\r\nEND:VCALENDAR");
        app.load_holidays(r#"[{"date":"2026-11-15","localName":"Proclamação da República"}]"#);
        let today = days_from_civil(2026, 11, 11);
        assert_eq!(app.state.today, today);
        assert!(app.state.agenda.iter().any(|i| i.holiday && i.day == days_from_civil(2026, 11, 15)));
        assert!(app.state.agenda.iter().any(|i| i.title == "Aluguel" && i.day == days_from_civil(2026, 12, 4)));
        app.apply_ui_action(UiAction::AgendaDay(days_from_civil(2026, 11, 20)));
        app.apply_ui_action(UiAction::AgendaMonth(1));
        assert_eq!(app.state.agenda_day, days_from_civil(2026, 12, 20));
        app.apply_ui_action(UiAction::SwitchTab(TabPage::CalendarWeather));
        assert_eq!(app.state.agenda_day, today);
        assert_eq!(app.islands[0].geom.height.target, isle_render::layout::PANEL.1);
    }

    #[test]
    fn frames_come_every_sixtieth_while_a_spring_moves_and_never_when_idle() {
        let t = Instant::now();
        let mut pacer = FramePacer::new(t);
        // Idle: the deadline as asked, no frame clock.
        assert_eq!(pacer.wait(t, false, Duration::from_secs(42)), Duration::from_secs(42));
        // A click a minute later sets a spring off: it moves one frame, not the minute it slept.
        assert_eq!(pacer.dt(t + Duration::from_secs(60)), FRAME.as_secs_f32());
        // Moving: the next frame FRAME after the last one, counting the time it took to draw.
        let drawn = t + Duration::from_secs(60) + Duration::from_millis(6);
        let wait = pacer.wait(drawn, true, Duration::from_secs(42));
        assert!(wait <= FRAME && wait > FRAME - Duration::from_millis(7), "{wait:?}");
        // Then by the time that passed, a slow frame included.
        assert!((pacer.dt(drawn + wait + Duration::from_millis(20)) - 0.0367).abs() < 0.002);
        // A frame that ran late: the next one at once, and the interval starts again from now.
        assert_eq!(pacer.wait(drawn + Duration::from_millis(80), true, Duration::from_secs(42)), Duration::ZERO);
    }

    #[test]
    fn music_bars_on_the_open_header_get_frames_while_it_plays() {
        let mut app = IsleApp::new(IsleConfig::default());
        // After `new`, which sets the config's level ("System": off on the CI's Windows Server).
        isle_render::window::set_animation_level(isle_core::config::AnimationLevel::On);
        app.islands[0].mode = PillMode::Expanded(TabPage::ControlCenter);
        app.state.media = Some(isle_core::state::MediaInfo { artwork: None, title: "a".into(), artist: "b".into(),
            playing: true, position_ms: 0, duration_ms: 200_000, sampled_at_ms: 0 });
        let now = Instant::now();
        assert!(app.sleep_for(now, 1_000_000) <= Duration::from_millis(50));
        app.state.media.as_mut().unwrap().playing = false;
        assert!(app.sleep_for(now, 1_000_000) > Duration::from_millis(50));
    }

    #[test]
    fn pomodoro_countdown_follows_wall_clock_not_tick_count() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.pomodoro.start(1_000_000 * 1000);
        // One tick ten minutes later must show 15 of 25 minutes left, however rarely we tick.
        app.tick_runtime_domain(1_000_000 + 600);
        assert_eq!(app.state.next_countdown_text.as_deref(), Some("Foco 15:00"));
    }

    use std::time::{Duration, Instant};

    fn after(t: Instant, secs: f32) -> Instant {
        t + Duration::from_secs_f32(secs)
    }

    fn heading_to(app: &IsleApp) -> (f32, f32) {
        (app.islands[0].geom.width.target, app.islands[0].geom.height.target,
        )
    }

    #[test]
    fn the_pill_hides_to_a_line_once_the_mouse_leaves_and_comes_back_under_it() {
        let mut app = IsleApp::new(IsleConfig::default());
        let t = Instant::now();
        app.on_hover(false);
        app.on_deadlines(t);
        app.on_deadlines(t + Duration::from_secs_f32(1.0));
        assert_eq!(app.islands[0].mode, PillMode::CompactBar);
        app.on_deadlines(t + Duration::from_secs_f32(2.0));
        assert_eq!(app.islands[0].mode, PillMode::Minimized);
        assert_eq!(heading_to(&app), (120.0, 5.0));
        app.on_hover(true);
        assert_eq!(app.islands[0].mode, PillMode::CompactBar);
        assert_eq!(heading_to(&app), (220.0, 34.0));
        // Under the mouse it stays.
        app.on_deadlines(t + Duration::from_secs_f32(10.0));
        assert_eq!(app.islands[0].mode, PillMode::CompactBar);
    }

    #[test]
    fn without_auto_hide_the_pill_stays() {
        let mut cfg = IsleConfig::default();
        cfg.auto_hide = false;
        let mut app = IsleApp::new(cfg);
        let t = Instant::now();
        app.on_hover(false);
        app.on_deadlines(t);
        app.on_deadlines(t + Duration::from_secs_f32(60.0));
        assert_eq!(app.islands[0].mode, PillMode::CompactBar);
    }

    #[test]
    fn a_banner_shows_over_the_line_then_goes_by_itself() {
        let mut app = IsleApp::new(IsleConfig::default());
        let t = Instant::now();
        app.on_hover(false);
        app.on_deadlines(t);
        app.on_deadlines(t + Duration::from_secs_f32(2.0));
        assert_eq!(app.islands[0].mode, PillMode::Minimized);
        app.handle_hook_event(agent("done", "Pronto."), 1000);
        let shown = after(t, 3.0);
        app.on_deadlines(shown);
        assert!(matches!(app.islands[0].mode, PillMode::AlertBanner { .. }));
        app.on_deadlines(after(shown, 5.0));
        assert!(matches!(app.islands[0].mode, PillMode::AlertBanner { .. }), "still reading");
        app.on_deadlines(after(shown, 7.0));
        assert_eq!(app.islands[0].mode, PillMode::CompactBar);
        app.on_deadlines(after(shown, 7.5));
        app.on_deadlines(after(shown, 9.0));
        assert_eq!(app.islands[0].mode, PillMode::Minimized);
    }

    #[test]
    fn an_open_panel_closes_once_the_mouse_leaves_it() {
        let mut app = IsleApp::new(IsleConfig::default());
        let t = Instant::now();
        app.on_hover(true);
        app.apply_ui_action(UiAction::ToggleCollapse);
        assert!(matches!(app.islands[0].mode, PillMode::Expanded(_)));
        app.on_deadlines(t + Duration::from_secs_f32(30.0));
        assert!(matches!(app.islands[0].mode, PillMode::Expanded(_)), "open while under the mouse");
        app.on_hover(false);
        app.on_deadlines(t + Duration::from_secs_f32(31.0));
        app.on_deadlines(t + Duration::from_secs_f32(33.0));
        assert_eq!(app.islands[0].mode, PillMode::CompactBar);
    }

    #[test]
    fn a_permission_card_never_hides_by_itself() {
        let mut app = IsleApp::new(IsleConfig::default());
        let (tx, _rx) = oneshot::channel();
        app.handle_hook_event(ask(tx), 1000);
        let t = Instant::now();
        app.on_hover(false);
        app.on_deadlines(t);
        app.on_deadlines(t + Duration::from_secs_f32(120.0));
        assert_eq!(app.islands[0].mode, PillMode::PermissionPrompt);
        assert!(app.next_deadline().is_none() || app.next_deadline() > Some(after(t, 120.0)));
    }

    #[test]
    fn each_monitor_has_its_own_island_and_alerts_go_to_the_main_one() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.islands.push(Presence::new());
        let t = Instant::now();

        // The mouse on the second monitor's island opens that one only.
        app.apply_input(UiInput::Hover { window: 1, inside: true,
        }).unwrap();
        app.apply_input(UiInput::Click { window: 1, x: CANVAS_MAX_WIDTH as f32 / 2.0 - 70.0, y: 20.0,
        }).unwrap();
        assert!(matches!(app.islands[1].mode, PillMode::Expanded(_)));
        assert_eq!(app.islands[0].mode, PillMode::CompactBar);

        // Mouse away from both: the main one hides; the second closes its panel first.
        app.apply_input(UiInput::Hover { window: 1, inside: false,
        }).unwrap();
        app.on_deadlines(t);
        app.on_deadlines(t + Duration::from_secs_f32(2.0));
        assert_eq!(app.islands[0].mode, PillMode::Minimized);
        assert_eq!(app.islands[1].mode, PillMode::CompactBar);

        // A permission card and a banner show on the main island, never on the other.
        let (tx, _rx) = oneshot::channel();
        app.handle_hook_event(ask(tx), 1000);
        assert_eq!(app.islands[0].mode, PillMode::PermissionPrompt);
        assert_eq!(app.islands[1].mode, PillMode::CompactBar);
        answer(&mut app, PermissionDecision::Deny);
        app.handle_hook_event(agent("done", "Pronto."), 1000);
        assert!(matches!(app.islands[0].mode, PillMode::AlertBanner { .. }));
        assert!(!matches!(app.islands[1].mode, PillMode::AlertBanner { .. }));
    }

    #[test]
    fn real_usage_fills_the_ai_tab_and_warns_on_the_fullest_window() {
        use isle_core::state::{UsageProvider, UsageWindow};
        let mut app = IsleApp::new(IsleConfig::default());
        let claude = UsageProvider {
            id: "claude".into(),
            name: "Claude Code".into(),
            windows: vec![
                UsageWindow { label: "5 horas".into(), percent: 83.0, resets_at_ms: 2_000_000,
                },
                UsageWindow { label: "Semana".into(), percent: 40.0, resets_at_ms: 9_000_000,
                },
            ],
            ..UsageProvider::default()
        };
        app.apply_usage(vec![claude.clone()], 1_000);
        assert_eq!(app.state.usage, vec![claude]);
        assert_eq!(app.state.claude_quota.used_pct, 83.0);
        assert_eq!(app.state.claude_quota.reset_at_unix, Some(2_000));
        assert_eq!(
            app.islands[0].mode,
            PillMode::AlertBanner {
                title: "Claude Code: 80% da cota".into(),
                subtitle: "Atenção ao consumo da janela atual".into(),
            }
        );
    }

    #[test]
    fn playing_media_shows_and_its_lyrics_follow_the_song() {
        use isle_platform_win::lyrics::{Lyrics, LyricsStatus};
        use isle_platform_win::media::MediaStatus;
        let mut app = IsleApp::new(IsleConfig::default());
        let song = MediaStatus {
            title: "Song (Official Video)".into(),
            artist: "Artist".into(),
            playing: true,
            position_ms: 10_000,
            duration_ms: 200_000,
            sampled_at_ms: 1_000_000,
            ..MediaStatus::default()
        };
        // A new track asks for its lyrics once; the same track again does not.
        let query = app.apply_media(Some(song.clone())).expect("a lyrics lookup");
        assert_eq!((query.title.as_str(), query.length_s), ("Song (Official Video)", 200.0));
        assert!(app.apply_media(Some(song.clone())).is_none());
        assert_eq!(app.state.media.as_ref().map(|m| m.title.as_str()), Some("Song (Official Video)"));

        // Lyrics of another track (arrived late) are ignored; this track's are kept.
        let lines = isle_core::lrc::parse("[00:05.00]um\n[00:12.00]dois\n[00:20.00]três");
        app.apply_lyrics(Lyrics { track: "outra".into(), status: LyricsStatus::Synced, lines: lines.clone(),
        });
        app.refresh_lyric(1_003_000);
        assert_eq!(app.state.active_lyric_line, None);
        app.apply_lyrics(Lyrics { track: query.key(), status: LyricsStatus::Synced, lines,
        });
        app.refresh_lyric(1_003_000); // 13 s into the song
        assert_eq!(app.state.active_lyric_line.as_deref(), Some("dois"));
        assert_eq!(app.next_lyric_at(1_003_000), Some(1_003_000 + 7_000));

        // Paused, the song stays where it is.
        app.apply_media(Some(MediaStatus { playing: false, position_ms: 13_000, sampled_at_ms: 1_003_000, ..song }));
        app.refresh_lyric(1_060_000);
        assert_eq!(app.state.active_lyric_line.as_deref(), Some("dois"));
        assert_eq!(app.next_lyric_at(1_060_000), None);

        // Nothing playing any more: no media, no lyrics.
        assert!(app.apply_media(None).is_none());
        app.refresh_lyric(1_060_000);
        assert_eq!((app.state.media.clone(), app.state.active_lyric_line.clone()), (None, None));
    }

    #[test]
    fn a_command_the_card_cannot_show_whole_goes_to_the_terminal() {
        let mut app = IsleApp::new(IsleConfig::default());
        let (tx, mut rx) = oneshot::channel();
        app.handle_hook_event(
            HookEvent::Ask {
                request: Request { id: "claude-1".into(), provider: "claude".into(), tool: "Bash".into(), input: "x".repeat(2_000), ..Request::default() },
                reply: tx,
            },
            1000,
        );
        // No answer from the island (the hook prints nothing): Claude asks in its own terminal.
        assert!(rx.try_recv().is_err() && app.pending_reply.is_none());
        assert_ne!(app.islands[0].mode, PillMode::PermissionPrompt);
    }
}

#[cfg(test)]
mod control_tests {
    use super::*;
    use isle_platform_win::audio_osd::AudioStatus;
    #[test]
    fn a_slider_drag_outside_the_pill_keeps_its_panel_open() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.apply_ui_action(UiAction::SwitchTab(TabPage::ControlCenter));
        app.volume_drag = Some(VolumeDrag { window: 0, left: 92.0, width: 326.0, app_id: None,
        });
        let now = Instant::now();
        app.on_deadlines(now); app.on_deadlines(now + LEAVE_AFTER);
        assert_eq!(app.islands[0].mode,PillMode::Expanded(TabPage::ControlCenter));
        assert_eq!(app.islands[0].leave_at,None);
        app.apply_input(UiInput::CancelDrag).unwrap();
        app.on_deadlines(now + LEAVE_AFTER);
        app.on_deadlines(now + LEAVE_AFTER*2);
        assert_eq!(app.islands[0].mode,PillMode::CompactBar);
    }
    #[test]
    fn an_expired_volume_osd_under_the_mouse_has_no_busy_loop_deadline() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.apply_audio(AudioStatus { volume_pct: Some(50), ..Default::default() });
        app.apply_audio(AudioStatus { volume_pct: Some(52), ..Default::default() });
        let expires = app.banner_until.unwrap();
        app.on_hover(true);
        app.on_deadlines(expires + Duration::from_secs(1));
        assert!(matches!(app.islands[0].mode,PillMode::AlertBanner { .. }));
        assert_eq!(app.next_deadline(),None);
        app.on_hover(false);
        app.on_deadlines(expires + Duration::from_secs(1));
        assert_eq!(app.islands[0].mode,PillMode::CompactBar);
    }
    #[test]
    fn drag_keeps_app_identity_and_clamps_outside_the_window() {
        let mut app = IsleApp::new(IsleConfig::default());
        let (tx,rx) = std::sync::mpsc::channel(); app.audio_tx = Some(tx);
        app.state.audio_available = true;
        app.state.app_volumes = vec![isle_core::state::AppVolume { id: "firefox".into(), name: "Firefox".into(), volume_pct: 30, muted: false,
        }];
        app.apply_ui_action(UiAction::SwitchTab(TabPage::ControlCenter));
        app.islands[0].geom.width.value = 600.0;
        app.islands[0].geom.height.value = 470.0;
        app.state.mode = app.islands[0].mode.clone();
        let slider = compute_hitboxes(CANVAS_MAX_WIDTH as f32, 600., 470., 6., &app.state)
            .into_iter()
            .find(|b| matches!(b.action, UiAction::AppVolume(0, _)))
            .unwrap();
        app.apply_input(UiInput::PointerDown { window: 0, x: slider.x + slider.w / 2.,
            y: slider.y + slider.h / 2.,
        }).unwrap();
        assert_eq!(rx.try_recv().ok(), Some(AudioCommand::SetAppVolume("firefox".into(), 50)));
        app.state.app_volumes[0].id = "other-app".into();
        app.apply_input(UiInput::Drag { window: 0, x: 900.0, y: 900.0,
        }).unwrap();
        assert_eq!(rx.try_recv().ok(), Some(AudioCommand::SetAppVolume("firefox".into(), 100)));
        app.apply_input(UiInput::Click { window: 0, x: -100.0, y: 900.0,
        }).unwrap();
        assert_eq!(rx.try_recv().ok(), Some(AudioCommand::SetAppVolume("firefox".into(), 0)));
        assert!(app.volume_drag.is_none());
    }
    #[test]
    fn only_the_control_center_slider_changes_the_volume_on_an_open_island() {
        for tab in [TabPage::ControlCenter, TabPage::Media, TabPage::Weather, TabPage::Timer] {
            let mut app = IsleApp::new(IsleConfig::default());
            let (tx,rx) = std::sync::mpsc::channel(); app.audio_tx = Some(tx);
            app.state.audio_available = true;
            app.apply_ui_action(UiAction::SwitchTab(tab));
            app.islands[0].geom.width.value = 600.0;
            app.islands[0].geom.height.value = 470.0;
            app.state.mode = app.islands[0].mode.clone();
            let slider = compute_hitboxes(CANVAS_MAX_WIDTH as f32, 600., 470., 6., &app.state).into_iter()
                .find(|b| matches!(b.action, UiAction::SetVolume(_)));
            // Pressed off the slider, dragged across the panel, swiped (a touchpad's wheel): no volume.
            let (x, y) = (CANVAS_MAX_WIDTH as f32 / 2. - 200., 120.);
            app.apply_input(UiInput::PointerDown { window: 0, x, y }).unwrap();
            app.apply_input(UiInput::Drag { window: 0, x: x + 150., y: y + 200. }).unwrap();
            app.apply_input(UiInput::Wheel { window: 0, delta: -120, x, y }).unwrap();
            assert!(rx.try_recv().is_err(), "{tab:?}");
            // A drag that started on the slider follows it, and stops when the tab changes.
            if let Some(s) = slider {
                app.apply_input(UiInput::PointerDown { window: 0, x: s.x + s.w / 2., y: s.y + s.h / 2. }).unwrap();
                assert_eq!(rx.try_recv().ok(), Some(AudioCommand::SetVolume(50)));
                app.islands[0].mode = PillMode::Expanded(TabPage::Media);
                app.apply_input(UiInput::Drag { window: 0, x: s.x, y: s.y }).unwrap();
                assert!(rx.try_recv().is_err());
            }
        }
    }
    #[test]
    fn panel_refresh_follows_all_islands_and_stops_after_last_closes() {
        let mut app = IsleApp::new(IsleConfig::default());
        let (tx,rx) = std::sync::mpsc::channel(); app.audio_tx = Some(tx);
        app.islands.push(Presence::new());
        app.apply_ui_action_on(1,UiAction::SwitchTab(TabPage::ControlCenter));
        app.sync_control_panel();
        assert_eq!(rx.try_recv().ok(),Some(AudioCommand::Panel(true)));
        app.sync_control_panel(); assert!(rx.try_recv().is_err());
        app.apply_ui_action_on(1,UiAction::SwitchTab(TabPage::Media));
        app.sync_control_panel();
        assert_eq!(rx.try_recv().ok(),Some(AudioCommand::Panel(false)));
    }
    #[test]
    fn the_wheel_turns_only_the_slider_under_the_mouse() {
        let mut app = IsleApp::new(IsleConfig::default());
        let (tx,rx) = std::sync::mpsc::channel(); app.audio_tx = Some(tx);
        app.state.audio_available = true;
        app.state.app_volumes = vec![isle_core::state::AppVolume { id: "spotify".into(), name: "Spotify".into(), volume_pct: 40, muted: false }];
        app.apply_ui_action(UiAction::SwitchTab(TabPage::ControlCenter));
        app.islands[0].geom.width.value = 600.0;
        app.islands[0].geom.height.value = 470.0;
        app.state.mode = app.islands[0].mode.clone();
        let boxes = compute_hitboxes(CANVAS_MAX_WIDTH as f32, 600., 470., 6., &app.state);
        let at = |b: &isle_render::pages::HitBox| (b.x + b.w / 2., b.y + b.h / 2.);
        let (mx, my) = at(boxes.iter().find(|b| matches!(b.action, UiAction::SetVolume(_))).unwrap());
        let (ax, ay) = at(boxes.iter().find(|b| matches!(b.action, UiAction::AppVolume(0, _))).unwrap());
        app.apply_input(UiInput::Wheel { window: 0, delta: -120, x: mx, y: my }).unwrap();
        assert_eq!(rx.try_recv().ok(), Some(AudioCommand::Wheel(-120)));
        app.apply_input(UiInput::Wheel { window: 0, delta: -120, x: ax, y: ay }).unwrap();
        assert_eq!(rx.try_recv().ok(), Some(AudioCommand::SetAppVolume("spotify".into(), 38)));
        // Off the sliders (the tiles, the title row): nothing.
        app.apply_input(UiInput::Wheel { window: 0, delta: -120, x: mx, y: my - 120. }).unwrap();
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn the_notch_opens_on_hover_only_with_the_option() {
        let mut options = IsleConfig::default().ui;
        options.pill_style = isle_core::config::PillStyle::Notch;
        let mut island = Presence::new();
        island.hover(true, options);
        assert!(island.enter_at.is_none());
        options.hover_open = true;
        island.hover(true, options);
        assert!(island.enter_at.is_some());
    }
    #[test]
    fn wheel_on_secondary_island_sends_signed_delta_without_opening_it() {
        let mut app = IsleApp::new(IsleConfig::default());
        let (tx,rx) = std::sync::mpsc::channel(); app.audio_tx = Some(tx);
        app.islands.push(Presence::new());
        app.apply_input(UiInput::Wheel { window: 1, delta: -240, x: 0., y: 0.,
        }).unwrap();
        assert_eq!(rx.try_recv().ok(),Some(AudioCommand::Wheel(-240)));
        assert_eq!(app.islands[1].mode,PillMode::CompactBar);
    }
    #[test]
    fn choosing_an_output_sends_its_device_id() {
        let mut app = IsleApp::new(IsleConfig::default());
        let (tx,rx) = std::sync::mpsc::channel(); app.audio_tx = Some(tx);
        app.apply_audio(AudioStatus { volume_pct: Some(50), outputs: vec![
            isle_core::state::AudioOutput { id: "{spk}".into(), name: "Alto-falantes".into(), is_default: true,
                },
            isle_core::state::AudioOutput { id: "{fone}".into(), name: "Fone".into(), is_default: false,
                },
        ], ..Default::default() });
        app.apply_ui_action(UiAction::SetOutput(1));
        assert_eq!(rx.try_recv().ok(), Some(AudioCommand::SetOutput("{fone}".into())));
    }
    #[test]
    fn a_countdown_ends_once_as_a_banner_that_waits_out_do_not_disturb() {
        use isle_core::timers::TimerMode;
        use isle_render::pages::TimerAction;
        let mut app = IsleApp::new(IsleConfig::default());
        let t = Instant::now();
        app.apply_ui_action(UiAction::Timer(TimerAction::Mode(TimerMode::Countdown)));
        app.apply_ui_action(UiAction::Timer(TimerAction::Preset(10)));
        assert_eq!(app.state.timers.countdown_duration_ms, 600_000);
        app.apply_ui_action(UiAction::Timer(TimerAction::Toggle));
        app.refresh_timers(60_000);
        assert_eq!(app.state.next_countdown_text.as_deref(), Some("Timer 9:00"));
        // Started: the length can't change under it.
        app.apply_ui_action(UiAction::Timer(TimerAction::Preset(5)));
        assert_eq!(app.state.timers.countdown_duration_ms, 600_000);
        app.state.dnd_active = true;
        app.refresh_timers(600_000);
        app.refresh_timers(700_000);
        assert_eq!(app.alerts.len(), 1, "one end, one alert");
        app.on_deadlines(t);
        assert_eq!(app.islands[0].mode, PillMode::CompactBar, "waits under Não Perturbe");
        app.state.dnd_active = false;
        app.on_deadlines(t + Duration::from_secs_f32(1.0));
        assert!(matches!(&app.islands[0].mode, PillMode::AlertBanner { title, .. } if title == "Timer"));
        assert_eq!(app.state.next_countdown_text, None);
    }

    #[test]
    fn a_typed_reminder_is_kept_on_disk_shown_once_when_due_and_opens_the_timer_tab() {
        use isle_core::timers::TimerMode;
        use isle_render::pages::TimerAction;
        let dir = std::env::temp_dir().join(format!("isle-lembretes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (file, given) = (dir.join("lembretes.json"), dir.join("agenda-avisados.json"));
        let _ = std::fs::remove_file(&file);
        let mut app = IsleApp::new(IsleConfig::default());
        app.set_reminder_files(file.clone(), given.clone());
        app.apply_ui_action(UiAction::SwitchTab(TabPage::Timer));
        app.apply_ui_action(UiAction::Timer(TimerAction::Mode(TimerMode::Reminders)));
        app.apply_ui_action(UiAction::Timer(TimerAction::InPreset(15)));
        app.apply_ui_action(UiAction::Timer(TimerAction::Field));
        for ch in "Tirar a roupa\r".chars() { app.apply_input(UiInput::Char { window: 0, ch }).unwrap(); }
        assert!(!app.reminder_typing && app.state.timers.reminder_text.is_empty());
        assert_eq!(app.state.timers.reminders[0].text, "Tirar a roupa");
        // A new run reads it back; due 15 minutes later, it shows once.
        let mut next = IsleApp::new(IsleConfig::default());
        next.set_reminder_files(file.clone(), given);
        next.refresh_timers(15 * 60_000 - 1);
        assert!(next.alerts.is_empty());
        next.refresh_timers(15 * 60_000);
        next.refresh_timers(16 * 60_000);
        assert_eq!(next.alerts.len(), 1);
        assert!(next.state.timers.reminders.is_empty());
        assert!(!std::fs::read_to_string(&file).unwrap().contains("roupa"));
        next.on_deadlines(Instant::now());
        next.apply_ui_action(UiAction::ToggleCollapse);
        assert_eq!(next.islands[0].mode, PillMode::Expanded(TabPage::Timer));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn escape_leaves_the_field_with_the_text_kept_and_the_panel_waits_while_typing() {
        use isle_core::timers::TimerMode;
        use isle_render::pages::TimerAction;
        let mut app = IsleApp::new(IsleConfig::default());
        app.apply_ui_action(UiAction::SwitchTab(TabPage::Timer));
        app.apply_ui_action(UiAction::Timer(TimerAction::Mode(TimerMode::Reminders)));
        app.apply_ui_action(UiAction::Timer(TimerAction::Field));
        for ch in "oi".chars() { app.apply_input(UiInput::Char { window: 0, ch }).unwrap(); }
        let now = Instant::now();
        app.islands[0].hover(false, app.config.ui);
        app.on_deadlines(now); app.on_deadlines(now + Duration::from_secs(5));
        assert_eq!(app.islands[0].mode, PillMode::Expanded(TabPage::Timer));
        app.apply_input(UiInput::Char { window: 0, ch: '\u{1b}',
        }).unwrap();
        assert!(!app.reminder_typing);
        assert_eq!(app.state.timers.reminder_text, "oi");
    }

    #[test]
    fn agenda_reminders_come_five_at_a_time_and_each_once() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.state.agenda = (0..7).map(|i| isle_core::agenda::AgendaItem { title: format!("Evento {i}"), day: 100, minute: Some(600), holiday: false,
            }).collect();
        app.remind_agenda(100 * 1440 + 600);
        assert_eq!(app.alerts.len(), 6);
        assert_eq!(app.alerts.back().unwrap().1, "e mais 2 lembretes");
        assert!(app.alerts.iter().all(|a| a.2 == Some(Opens::Tab(TabPage::CalendarWeather))));
        app.remind_agenda(100 * 1440 + 601);
        assert_eq!(app.alerts.len(), 6, "each event once");
    }

    #[test]
    fn queued_alerts_show_one_after_another() {
        let mut app = IsleApp::new(IsleConfig::default());
        let t = Instant::now();
        app.alert("Lembrete", "Primeiro", None);
        app.alert("Lembrete", "Segundo", None);
        app.on_deadlines(t);
        assert!(matches!(&app.islands[0].mode, PillMode::AlertBanner { subtitle, .. } if subtitle == "Primeiro"));
        app.on_deadlines(t + Duration::from_secs_f32(1.0));
        assert!(matches!(&app.islands[0].mode, PillMode::AlertBanner { subtitle, .. } if subtitle == "Primeiro"));
        app.on_deadlines(t + Duration::from_secs_f32(7.0));
        assert!(matches!(&app.islands[0].mode, PillMode::AlertBanner { subtitle, .. } if subtitle == "Segundo"));
    }

    #[test]
    fn timer_lengths_and_agent_switches_from_the_settings_apply_at_once() {
        let mut app = IsleApp::new(IsleConfig::default());
        let old = app.config.clone();
        app.config.pomodoro_focus_secs = 50 * 60;
        app.config.pomodoro_break_secs = 7 * 60;
        app.config.timer_default_secs = 9 * 60;
        app.config.ai_codex_enabled = false;
        app.apply_config(&old, false);
        assert_eq!((app.pomodoro.config().focus_minutes, app.pomodoro.config().short_break_minutes), (50, 7));
        assert_eq!(app.countdown.duration_ms, 9 * 60_000);
        let provider = |id: &str| isle_core::state::UsageProvider { id: id.into(), ..Default::default() };
        app.apply_usage(vec![provider("claude"), provider("codex")], 1000);
        assert_eq!(app.state.usage.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["claude"]);
        let old = app.config.clone();
        app.config.ai_claude_enabled = false;
        app.apply_config(&old, false);
        assert!(app.state.usage.is_empty(), "switched off, its usage leaves at once");
    }

    #[test]
    fn an_alert_beeps_when_it_shows_unless_the_sound_is_off() {
        for (on, beeps) in [(true, 1), (false, 0)] {
            let mut app = IsleApp::new(IsleConfig { alert_sound: on, ..IsleConfig::default() });
            BEEPS.with(|b| b.set(0));
            app.alert("Lembrete", "Água", None);
            assert_eq!(BEEPS.with(|b| b.get()), 0, "queued is silent");
            app.on_deadlines(Instant::now());
            assert_eq!(BEEPS.with(|b| b.get()), beeps);
        }
    }

    #[test]
    fn pomodoro_steps_are_bounded_and_the_bar_shows_what_ends_first() {
        use isle_core::timers::TimerMode;
        use isle_render::pages::TimerAction;
        let mut app = IsleApp::new(IsleConfig::default());
        for _ in 0..10 { app.apply_ui_action(UiAction::Timer(TimerAction::Break(-1))); }
        assert_eq!(app.config.pomodoro_break_secs, 60);
        app.apply_ui_action(UiAction::Timer(TimerAction::Focus(-5)));
        assert_eq!(app.state.timers.focus_minutes, 20);
        app.apply_ui_action(UiAction::Timer(TimerAction::Toggle)); // Pomodoro: 20 min focus
        app.apply_ui_action(UiAction::Timer(TimerAction::Mode(TimerMode::Countdown)));
        app.apply_ui_action(UiAction::Timer(TimerAction::Preset(5)));
        app.apply_ui_action(UiAction::Timer(TimerAction::Toggle));
        app.apply_ui_action(UiAction::Timer(TimerAction::Mode(TimerMode::Stopwatch)));
        app.apply_ui_action(UiAction::Timer(TimerAction::Toggle));
        app.refresh_timers(61_000);
        assert_eq!(app.state.next_countdown_text.as_deref(), Some("Timer 3:59"));
        assert!(app.running_timer());
    }

    #[test]
    fn the_temperature_opens_the_weather_panel_on_today() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.state.weather_day = 4;
        app.apply_ui_action(UiAction::OpenWeather);
        assert_eq!(app.islands[0].mode, PillMode::Expanded(TabPage::Weather));
        assert_eq!((app.islands[0].geom.width.target, app.islands[0].geom.height.target), (isle_render::layout::PANEL.0, isle_render::layout::PANEL.1));
        assert_eq!(app.state.weather_day, 0);
        app.apply_ui_action(UiAction::WeatherDay(2));
        assert_eq!(app.state.weather_day, 2);
    }
    #[test]
    fn every_tab_opens_the_island_at_one_size() {
        let mut app = IsleApp::new(IsleConfig::default());
        let mut sizes = Vec::new();
        for tab in [TabPage::ControlCenter, TabPage::Weather, TabPage::Media, TabPage::Tray, TabPage::Notifications] {
            app.apply_ui_action(UiAction::SwitchTab(tab));
            sizes.push((tab, app.islands[0].geom.width.target, app.islands[0].geom.height.target));
        }
        assert!(sizes.iter().all(|s| (s.1, s.2) == (sizes[0].1, sizes[0].2)), "{sizes:?}");
    }
    #[test]
    fn volume_keys_show_main_osd_but_keep_panels_and_permission_cards() {
        let mut app = IsleApp::new(IsleConfig::default());
        let status = |v| AudioStatus { volume_pct: Some(v), ..Default::default() };
        app.apply_audio(status(50));
        assert_eq!(app.islands[0].mode, PillMode::CompactBar); // initial read is silent
        app.apply_audio(status(52));
        assert!(matches!(app.islands[0].mode, PillMode::AlertBanner { .. }));
        app.apply_ui_action(UiAction::SwitchTab(TabPage::ControlCenter));
        app.apply_audio(status(54));
        assert_eq!(app.islands[0].mode, PillMode::Expanded(TabPage::ControlCenter));
        let (tx, _rx) = oneshot::channel();
        app.handle_hook_event(super::tests::ask(tx), 1000);
        app.apply_audio(status(56));
        assert_eq!(app.islands[0].mode, PillMode::PermissionPrompt);
        assert_eq!(app.state.volume_pct, 56);
    }
}

#[cfg(test)]
mod notification_tests {
    use super::*;
    use isle_core::notifications::Notification;

    fn notice(id: i64) -> Notification {
        Notification { id, arrival_ms: id * 1000, app_id: "Mail".into(), app_name: "Mail".into(),
            title: "Uma mensagem".into(), body: "Texto recebido".into(), unread: false, ..Default::default() }
    }

    #[test]
    fn win_n_toggles_main_and_keeps_permission_cards() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.islands.push(Presence::new());
        app.toggle_notifications();
        assert_eq!(app.islands[0].mode,PillMode::Expanded(TabPage::Notifications));
        assert_eq!(app.islands[1].mode,PillMode::CompactBar);
        app.toggle_notifications();
        assert_eq!(app.islands[0].mode,PillMode::CompactBar);
        let (tx,_rx) = oneshot::channel();
        app.handle_hook_event(super::tests::ask(tx),1000);
        app.toggle_notifications();
        assert_eq!(app.islands[0].mode,PillMode::PermissionPrompt);
    }

    #[test]
    fn successive_banners_show_latest_and_click_opens_it_and_reads_only_it() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.islands[0].mode = PillMode::Minimized;
        app.apply_notifications(vec![notice(1)],false);
        app.apply_notifications(vec![notice(2)],false);
        app.apply_ui_action(UiAction::ToggleCollapse);
        // The notification opened in its app (skipped in tests); the banner gives way to the bar.
        assert_eq!(app.islands[0].mode,PillMode::CompactBar);
        assert_eq!(app.state.notifications.unread_count(),1);
        assert!(app.state.notifications.ordered().iter().find(|n| n.id == 1).unwrap().unread);
    }

    #[test]
    fn late_icon_keeps_banner_after_windows_dismissal_without_extending_it() {
        let mut app=IsleApp::new(IsleConfig::default());
        let n=notice(1);
        let app_id=n.app_id.clone();
        app.apply_notifications(vec![n],false);
        let deadline=app.banner_until;
        let mode=app.islands[0].mode.clone();
        app.state.notifications.retain_present(&[],&[]);
        assert!(app.state.notifications.ordered().is_empty());
        let icon=std::sync::Arc::new(isle_core::artwork::Artwork::new(1,1,vec![0,255,0,255]).unwrap());
        app.apply_notification_icon(&app_id,icon.clone());
        let (snapshot,_)=app.notification_banner.as_ref().unwrap();
        assert!(std::sync::Arc::ptr_eq(snapshot.app_icon.as_ref().unwrap(),&icon));
        assert_eq!(snapshot.title,"Uma mensagem");
        assert_eq!(app.islands[0].mode,mode);
        assert_eq!(app.banner_until,deadline);
        assert_eq!(app.state.notifications.unread_count(),0);
    }

    #[test]
    fn arrivals_banner_on_main_only_for_six_seconds_and_duplicates_do_not_extend_it() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.islands.push(Presence::new());
        app.apply_notifications(vec![notice(1)], true);
        assert_eq!(app.islands[0].mode, PillMode::CompactBar);
        app.apply_notifications(vec![notice(2)], false);
        assert!(matches!(&app.islands[0].mode, PillMode::AlertBanner { title, subtitle }
            if title == "Mail · Uma mensagem" && subtitle == "Texto recebido"));
        assert_eq!(app.islands[1].mode, PillMode::CompactBar);
        let until = app.banner_until.unwrap();
        assert!(until > Instant::now() + Duration::from_secs(5));
        app.apply_notifications(vec![notice(2)], false);
        assert_eq!(app.banner_until, Some(until));
        app.on_deadlines(until + Duration::from_millis(1));
        assert_eq!(app.islands[0].mode, PillMode::CompactBar);
    }

    #[test]
    fn notifications_keep_permission_panel_and_ai_banner_while_counting_unread() {
        for mode in [PillMode::PermissionPrompt, PillMode::Expanded(TabPage::Media),
            PillMode::Expanded(TabPage::Notifications), PillMode::AlertBanner { title: "Claude Code".into(), subtitle: "Done".into(),
            },
        ] {
            let mut app = IsleApp::new(IsleConfig::default());
            app.islands[0].mode = mode.clone();
            app.apply_notifications(vec![notice(1)], false);
            assert_eq!(app.islands[0].mode,mode);
            assert_eq!(app.state.notifications.unread_count(),1);
        }
    }

    #[test]
    fn notification_click_marks_only_that_item_and_clear_all_resets_page() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.apply_notifications((1..=5).map(notice).collect(), false);
        app.apply_ui_action(UiAction::ReadNotification(2,2000));
        assert_eq!(app.state.notifications.unread_count(),4);
        app.apply_ui_action(UiAction::SwitchTab(TabPage::Notifications));
        assert_eq!(app.islands[0].geom.height.target, isle_render::layout::PANEL.1);
        app.apply_ui_action(UiAction::NotificationsPage(usize::MAX));
        assert_eq!(app.state.notification_page,1);
        app.apply_ui_action(UiAction::ClearNotifications);
        assert!(app.state.notifications.ordered().is_empty());
        assert_eq!(app.state.notification_page,0);
    }

    #[test]
    fn a_reply_is_typed_in_avisos_and_keeps_the_panel_open() {
        let mut app = IsleApp::new(IsleConfig::default());
        let mut n = notice(1); n.reply_input = "box".into(); n.reply_args = "send".into();
        app.apply_notifications(vec![n], false);
        app.apply_ui_action(UiAction::SwitchTab(TabPage::Notifications));
        app.apply_ui_action(UiAction::ReplyNotification(1,1000));
        assert_eq!(app.state.notifications.unread_count(),0);
        for ch in "oi!\u{8}".chars() { app.apply_input(UiInput::Char { window: 0, ch }).unwrap(); }
        assert_eq!(app.state.reply.as_ref().unwrap().text,"oi");
        // The mouse leaves: the panel waits for the reply.
        let now = Instant::now();
        app.islands[0].hover(false, app.config.ui);
        app.on_deadlines(now); app.on_deadlines(now + Duration::from_secs(5));
        assert_eq!(app.islands[0].mode,PillMode::Expanded(TabPage::Notifications));
        app.apply_input(UiInput::Char { window: 0, ch: '\u{1b}',
        }).unwrap();
        assert!(app.state.reply.is_none());
        // Enter sends (skipped in tests) and closes the box too.
        app.apply_ui_action(UiAction::ReplyNotification(1,1000));
        app.apply_input(UiInput::Char { window: 0, ch: 'x' }).unwrap();
        app.apply_input(UiInput::Char { window: 0, ch: '\r',
        }).unwrap();
        assert!(app.state.reply.is_none());
    }

    #[test]
    fn dnd_keeps_notifications_without_a_banner() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.state.dnd_active = true;
        app.apply_notifications(vec![notice(1)],false);
        assert_eq!(app.islands[0].mode,PillMode::CompactBar);
        assert_eq!(app.state.notifications.unread_count(),1);
    }
}
