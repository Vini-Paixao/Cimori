use std::collections::HashMap;
use std::fs;
use std::path::Path;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::state::{ControlTile, TabPage};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PinnedTrayApp {
    pub exe_path: String,
    pub guid: Option<String>,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherConfig {
    pub label: String,
    pub latitude: f64,
    pub longitude: f64,
}

/// Fields missing from an older settings.json take their default, so an update never breaks it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IsleConfig {
    pub liquid_glass: bool,
    pub icon_pack: crate::icons::IconPack,
    pub icon_folder: String,
    pub monitors: Vec<String>,
    /// An island on every monitor, whatever their names (Mac Island & Dock's iislandAllMonitors);
    /// otherwise only on `monitors`.
    pub all_monitors: bool,
    /// The main island's monitor ("\\.\DISPLAY1"): notifications, alerts and permission cards go
    /// there (Mac Island & Dock's sislandMonitor). None, or gone: the primary monitor.
    pub main_monitor: Option<String>,
    pub gap_px: i32,
    pub background_rgb: (u8, u8, u8),
    pub notch_curve: u32,
    pub auto_hide: bool,
    pub pomodoro_focus_secs: u32,
    pub pomodoro_break_secs: u32,
    pub timer_default_secs: u32,
    pub weather: WeatherConfig,
    /// Temperatures in °F (the mod's UsesFahrenheit).
    pub weather_fahrenheit: bool,
    /// A banner when an agenda event is due, this many minutes before it (the mod: at its time).
    pub agenda_reminders: bool,
    pub agenda_reminder_minutes: u32,
    pub calendar_ics_url: Option<String>,
    pub pinned_tray_apps: Vec<PinnedTrayApp>,
    pub folder_pins: Vec<String>,
    pub ai_claude_enabled: bool,
    pub ai_codex_enabled: bool,
    pub opencode_api_key: Option<String>,
    /// The model each provider answers the chat with, as its CLI names it ("opus"); empty: its default.
    pub ai_claude_model: String,
    pub ai_codex_model: String,
    pub ai_opencode_model: String,
    pub hook_port: u16,

    // The settings window (docs/superpowers/specs/2026-11-11-configuracoes-design.md).
    /// What the closed pill shows, left to right: the first flush left, the rest flush right.
    pub bar_items: Vec<BarItem>,
    /// The AI quota bars along the pill's bottom.
    pub bar_quota_strip: bool,
    pub clock_24h: bool,
    pub clock_seconds: bool,
    pub clock_weekday: bool,
    pub clock_date: bool,
    pub clock_year: bool,
    pub date_order: DateOrder,
    pub month_number: bool,
    pub countdown_events: bool,
    pub countdown_timer: bool,
    pub countdown_pomodoro: bool,
    /// How far ahead an event counts down on the bar.
    pub countdown_hours: u32,
    pub accent_rgb: (u8, u8, u8),
    pub animations: AnimationLevel,
    /// Stay over borderless full-screen apps instead of stepping aside.
    pub over_fullscreen: bool,
    /// Step aside, as for a full-screen app, while a maximized window is on the island's monitor (a browser's
    /// tabs sit where the island is).
    pub hide_maximized: bool,
    /// Programs (exe file names, lowercase) the island steps aside for while one of them is the maximized
    /// window in front on its monitor; the others keep it.
    pub hide_apps: Vec<String>,
    pub modules: Modules,
    pub quota_alerts: bool,
    /// Cimori renews Claude Code's sign-in when it ran out and no Claude Code here renewed it (Claude Code
    /// used only on another computer). Off: the limits stop until Claude Code is opened on this one.
    pub claude_renew_login: bool,
    pub control_tiles: Vec<ControlTile>,
    pub control_mixer: bool,
    pub control_outputs: bool,
    pub wheel_volume: bool,
    pub holidays: bool,
    /// Two letters; empty: the country of Windows' region.
    pub holidays_country: String,
    /// Use Windows' time zone (off: São Paulo, UTC-3).
    pub timezone_system: bool,
    /// A beep when a reminder, a timer or an agenda event shows (the mod's alarm sound).
    pub alert_sound: bool,
    pub lyrics: bool,
    pub banners: bool,
    pub banner_secs: u32,
    pub notifications_per_page: u32,
    /// Apps whose notifications never reach the island (their AUMIDs).
    pub muted_apps: Vec<String>,
    pub notification_icons: bool,
    pub win_n: bool,
    /// The agents' server answers only this machine and Tailscale (100.64.0.0/10).
    pub hooks_local_only: bool,
    pub hooks_token: Option<String>,
    pub ui: PillOptions,
    pub clipboard_limit: u32,
    pub clipboard_images: bool,
    pub clipboard_files: bool,
    /// What's copied, in a banner on the pill (Mac Island's islandClipboard); nothing is kept.
    pub clipboard_notice: bool,
    pub clipboard_persist: bool,
    pub clipboard_paste: bool,
    pub clipboard_thumbnails: bool,
    pub clipboard_ignored_apps: Vec<String>,
    pub shelf_persist: bool,
    pub shelf_drag_move: bool,
    /// Files dragged to the closed pill go on the shelf (mac-island's island drop target).
    pub shelf_drop_pill: bool,
    pub shelf_capacity: u32,
    pub shelf_qr: bool,
    pub shelf_receive: bool,
    pub shelf_idle_minutes: u32,
    pub usb_alerts: bool,
    pub download_meter: bool,
    pub download_taskbar: bool,
    pub download_animation: bool,
    pub download_details: bool,
    /// The Downloads tab lists the newest finished files of the Downloads folder.
    pub download_recent: bool,
    pub settings_window: Option<WindowPlacement>,
    /// What the closed notch shows (`PillStyle::Notch`), as `bar_items` does for the pill; the clock alone
    /// at first, as isle's notch.
    pub notch_items: Vec<BarItem>,
    /// The power button's actions and its confirmation (Mac Island & Dock's islandPower*).
    pub power: crate::power::PowerOptions,
    /// Looks for a newer Cimori on GitHub at start and once a day (isle's Updates1).
    pub check_updates: bool,
    /// Opens at sign-in: the Run key of HKCU follows this (off at first).
    pub start_with_windows: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BarItem {
    Clock,
    /// Today's slot: what comes within the countdown window, else what plays, else the date.
    Next,
    Date,
    Media,
    Weather,
    Notifications,
    Timer,
    Tray,
    /// The power button (Mac Island & Dock's IslandItem::Power).
    Power,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DateOrder {
    #[default]
    WeekdayDayMonth,
    DayMonth,
    MonthDay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AnimationLevel {
    /// Windows' "animation effects".
    #[default]
    System,
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Modules {
    pub ai: bool,
    pub control: bool,
    pub agenda: bool,
    pub weather: bool,
    pub media: bool,
    pub notifications: bool,
    pub timer: bool,
    pub tray: bool,
    pub clipboard: bool,
    pub shelf: bool,
    pub usb: bool,
    pub downloads: bool,
    /// The open panel's tabs, in the order the user dragged them to ("Ordem das abas"): every tab once.
    pub order: [TabPage; 12],
}

impl Modules {
    /// Whether a tab's module is on: the panel shows it.
    pub fn shows(&self, t: TabPage) -> bool {
        match t {
            TabPage::AiHub => self.ai, TabPage::ControlCenter => self.control, TabPage::CalendarWeather => self.agenda,
            TabPage::Weather => self.weather, TabPage::Media => self.media, TabPage::Notifications => self.notifications,
            TabPage::Timer => self.timer, TabPage::Tray => self.tray, TabPage::Shelf => self.shelf,
            TabPage::Clipboard => self.clipboard, TabPage::Usb => self.usb, TabPage::Downloads => self.downloads,
            TabPage::Power => false,
        }
    }
}

impl Default for Modules {
    fn default() -> Self {
        Self { ai: true, control: true, agenda: true, weather: true, media: true, notifications: true, timer: true, tray: true, clipboard: false, shelf: false, usb: false, downloads: false, order: TabPage::NAV }
    }
}

/// Visual and interaction options shared by the app, layout and preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PillOptions {
    pub arrow_pins: bool,
    pub hide_delay_ms: u32,
    pub hide_when: HideWhen,
    pub line_expire_secs: u32,
    pub line_click: bool,
    pub line_full_width: bool,
    pub hover_open: bool,
    pub follow_mouse: bool,
    pub tooltips: bool,
    /// The usage ring on the open panel's header: 0 Claude, 1 Codex, 2 both, 3 none.
    pub header_usage: u8,
    pub hover_highlight: bool,
    pub uniform_panels: bool,
    // Before the units in %: still read from an older settings.json (see `migrate_units`), never written.
    #[serde(skip_serializing)]
    pub pill_height: u32,
    #[serde(skip_serializing)]
    pub banner_width: u32,
    #[serde(skip_serializing)]
    pub banner_height: u32,
    pub banner_when_hidden: bool,
    /// Banners and permission requests come over a maximized window or an app the island steps aside
    /// for (Esconder); full-screen games and videos still go without.
    pub banner_over_apps: bool,
    pub media_controls: bool,
    pub media_always: bool,
    pub media_play_only: bool,
    pub media_lyric_title: bool,
    /// The clock in the middle of the closed island: the items before it in the list on its left,
    /// the ones after it on its right (Mac Island & Dock's islandClockCenter, with islandBarFixedSides).
    pub clock_center: bool,
    pub media_title_width: u32,
    pub lyric_size: u32,
    pub media_marquee: bool,
    pub media_wheel: bool,
    pub agenda_featured_day: bool,
    pub agenda_events: bool,
    pub weather_cards: bool,
    pub timer_on_line: bool,
    pub volume_percent: bool,
    pub mic_button: bool,
    pub tray_grid: bool,
    pub tray_keep_open: bool,
    pub lyric_rgb: (u8, u8, u8),
    pub minimize_hotkey: Hotkey,
    pub notices_hotkey: Hotkey,
    /// The closed island's shape: a floating pill, or a notch hanging from the screen's top edge.
    pub pill_style: PillStyle,
    /// The closed pill's least width (clock and next event), before the other items' room.
    pub pill_width: u32,
    /// Mac Island & Dock's units: the pill's height in % of the usual (islandPillHeight, 70–130), the
    /// banner's size and width in % (islandBannerSize of `BANNER_SIZES`, islandBannerWidth of
    /// `BANNER_WIDTHS`).
    pub pill_height_pct: u32,
    pub banner_size: u32,
    pub banner_width_pct: u32,
    /// The shortcuts as recorded (`shortcut::code`, 0 none); `minimize_hotkey` and `notices_hotkey`
    /// follow them where they are one of the old list.
    pub minimize_keys: u32,
    pub notices_keys: u32,
    /// The page the island opens on (isle opens on Music; `#[serde(default)]` gives older configs it too).
    pub open_tab: crate::state::TabPage,
}

/// Port of kBannerSizes, kBannerWidths and kPillHeightPct* (mac-island 3.1.1).
pub const BANNER_SIZES: [u32; 6] = [50, 75, 100, 125, 150, 200];
pub const BANNER_WIDTHS: [u32; 4] = [80, 100, 130, 160];
pub const PILL_HEIGHT_PCT: std::ops::RangeInclusive<u32> = 70..=130;
/// What 100 % is here: the pill's and the banner's sizes before the units changed.
pub const PILL_HEIGHT_BASE: f32 = 34.0;
pub const BANNER_BASE: (f32, f32) = (440.0, 64.0);

impl PillOptions {
    /// The closed pill's height in pixels (PillHeightScale).
    pub fn pill_height_px(&self) -> f32 {
        (PILL_HEIGHT_BASE * self.pill_height_pct as f32 / 100.0).round()
    }
    /// The banner's scale (BannerSizeFactor), and its size: the scale times its width's share.
    pub fn banner_scale(&self) -> f32 {
        self.banner_size as f32 / 100.0
    }
    pub fn banner_px(&self) -> (f32, f32) {
        let k = self.banner_scale();
        ((BANNER_BASE.0 * k * self.banner_width_pct as f32 / 100.0).round(), (BANNER_BASE.1 * k).round())
    }
}

/// Mac Island & Dock's Island style (the pill, `islandStyle` 0) and its notch (`islandPillNotches`,
/// the middle one), shaped as isle's notch: square against the top edge, round below, the curves
/// the other way where it meets the edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PillStyle {
    #[default]
    Island,
    Notch,
}

impl Default for PillOptions {
    fn default() -> Self {
        Self {
            arrow_pins: false,
            hide_delay_ms: 1500,
            hide_when: HideWhen::MouseAway,
            line_expire_secs: 0,
            line_click: false,
            line_full_width: false,
            hover_open: false,
            follow_mouse: false,
            tooltips: true,
            header_usage: 0,
            hover_highlight: true,
            uniform_panels: false,
            pill_height: 34,
            banner_width: 440,
            banner_height: 64,
            banner_when_hidden: true,
            banner_over_apps: true,
            media_controls: false,
            media_always: false,
            media_play_only: false,
            media_lyric_title: false,
            clock_center: false,
            media_title_width: 150,
            lyric_size: 15,
            media_marquee: false,
            media_wheel: false,
            agenda_featured_day: true,
            agenda_events: true,
            weather_cards: false,
            timer_on_line: false,
            volume_percent: true,
            mic_button: true,
            tray_grid: false,
            tray_keep_open: false,
            lyric_rgb: (245, 241, 235),
            minimize_hotkey: Hotkey::None,
            notices_hotkey: Hotkey::WinN,
            pill_style: PillStyle::Island,
            // Wider than the 220 it was: the closed pill read as a thin bar.
            pill_width: 280,
            pill_height_pct: 100,
            banner_size: 100,
            banner_width_pct: 100,
            minimize_keys: 0,
            notices_keys: crate::shortcut::code(crate::shortcut::WIN, b'N'),
            open_tab: crate::state::TabPage::Media,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hotkey { None, WinJ, WinN, CtrlAltJ, CtrlAltN }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HideWhen { MouseAway, Maximized, Covered }

impl HideWhen {
    pub const ALL: [Self; 3] = [Self::MouseAway, Self::Maximized, Self::Covered];
}

impl Hotkey {
    pub const ALL: [Self; 5] = [Self::None, Self::WinJ, Self::WinN, Self::CtrlAltJ, Self::CtrlAltN];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowPlacement {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub tab: String,
}

/// Writes beside `path` and renames over it: the file is never left half written.
pub fn write_atomic(path: &Path, json: &str) -> std::io::Result<()> {
    // Hook bootstrap may happen after the UI loaded its snapshot. Preserve a saved
    // token when that older snapshot still says null; an explicit new token wins.
    let retained = (|| {
        let mut next = serde_json::from_str::<serde_json::Value>(json).ok()?;
        if next.get("hooks_token")? != &serde_json::Value::Null { return None; }
        let old: serde_json::Value = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
        let token = old.get("hooks_token")?.as_str()?;
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) { return None; }
        next["hooks_token"] = token.into();
        serde_json::to_string_pretty(&next).ok()
    })();
    let json = retained.as_deref().unwrap_or(json);
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    fs::write(&tmp, json)?;
    fs::rename(&tmp, path)
}

impl Default for IsleConfig {
    fn default() -> Self {
        Self {
            icon_pack: Default::default(),
            liquid_glass: false,
            icon_folder: String::new(),
            monitors: vec![r"\\.\DISPLAY1".to_string()],
            all_monitors: true,
            main_monitor: None,
            gap_px: 6,
            background_rgb: (0x1C, 0x1C, 0x1E),
            notch_curve: 75,
            auto_hide: true,
            pomodoro_focus_secs: 1500,
            pomodoro_break_secs: 300,
            timer_default_secs: 300,
            weather: WeatherConfig {
                label: "São Paulo, São Paulo".to_string(),
                latitude: -23.5505,
                longitude: -46.6333,
            },
            weather_fahrenheit: false,
            agenda_reminders: true,
            agenda_reminder_minutes: 0,
            calendar_ics_url: None,
            pinned_tray_apps: Vec::new(),
            folder_pins: Vec::new(),
            ai_claude_enabled: true,
            ai_codex_enabled: true,
            opencode_api_key: None,
            ai_claude_model: String::new(),
            ai_codex_model: String::new(),
            ai_opencode_model: String::new(),
            hook_port: 47830,
            bar_items: vec![BarItem::Clock, BarItem::Next, BarItem::Weather, BarItem::Tray],
            bar_quota_strip: true,
            clock_24h: true,
            clock_seconds: false,
            clock_weekday: true,
            clock_date: true,
            clock_year: false,
            date_order: DateOrder::WeekdayDayMonth,
            month_number: false,
            countdown_events: true,
            countdown_timer: true,
            countdown_pomodoro: true,
            countdown_hours: 12,
            accent_rgb: (241, 139, 99),
            animations: AnimationLevel::System,
            over_fullscreen: false,
            hide_maximized: false,
            hide_apps: Vec::new(),
            modules: Modules::default(),
            quota_alerts: true,
            claude_renew_login: false,
            control_tiles: ControlTile::ALL.to_vec(),
            control_mixer: true,
            control_outputs: true,
            wheel_volume: true,
            holidays: true,
            holidays_country: String::new(),
            timezone_system: true,
            alert_sound: true,
            lyrics: true,
            banners: true,
            banner_secs: 6,
            notifications_per_page: 4,
            muted_apps: Vec::new(),
            notification_icons: true,
            win_n: true,
            hooks_local_only: false,
            hooks_token: None,
            ui: PillOptions::default(),
            clipboard_limit: 20,
            clipboard_images: true,
            clipboard_files: true,
            clipboard_notice: false,
            clipboard_persist: false,
            clipboard_paste: false,
            clipboard_thumbnails: false,
            clipboard_ignored_apps: Vec::new(),
            shelf_persist: false,
            shelf_drag_move: false,
            shelf_drop_pill: true,
            shelf_capacity: 6,
            shelf_qr: true,
            shelf_receive: false,
            shelf_idle_minutes: 10,
            usb_alerts: true,
            download_meter: true,
            download_taskbar: false,
            download_animation: false,
            download_details: true,
            download_recent: true,
            settings_window: None,
            notch_items: vec![BarItem::Clock],
            power: Default::default(),
            check_updates: true,
            start_with_windows: false,
        }
        .with_secure_hooks_default()
    }
}

impl IsleConfig {
    // Phase 9 changes the hook default; kept as an additive override for the shared file.
    fn with_secure_hooks_default(mut self) -> Self { self.hooks_local_only = true; self }

    /// The upstream hooks use the local session bus (agents/hook.rs); HTTP needs a
    /// shared secret. Generate it once from the OS and persist it with settings.
    pub fn ensure_hooks_token(&mut self) -> Result<bool> {
        if let Some(token) = &self.hooks_token {
            anyhow::ensure!(token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()), "Token dos hooks inválido: use 64 caracteres hexadecimais.");
            return Ok(false);
        }
        let mut bytes = [0u8; 32];
        #[cfg(windows)]
        {
            #[link(name = "bcrypt")]
            unsafe extern "system" { fn BCryptGenRandom(algorithm: *mut std::ffi::c_void, buffer: *mut u8, count: u32, flags: u32) -> i32; }
            anyhow::ensure!(unsafe { BCryptGenRandom(std::ptr::null_mut(), bytes.as_mut_ptr(), bytes.len() as u32, 2) } >= 0, "Não foi possível criar o token dos hooks.");
        }
        #[cfg(unix)]
        { use std::io::Read; std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?; }
        #[cfg(not(any(windows, unix)))]
        anyhow::bail!("O sistema não oferece uma fonte aleatória para os hooks.");
        self.hooks_token = Some(bytes.iter().map(|byte| format!("{byte:02x}")).collect());
        Ok(true)
    }

    /// Every value back inside its range, field by field (the rest is kept).
    /// The holidays' country: the chosen one, else Windows' region (`system`), else Brazil.
    pub fn holidays_country_or(&self, system: Option<String>) -> String {
        if self.holidays_country.len() == 2 { self.holidays_country.clone() } else { system.unwrap_or_else(|| "BR".into()) }
    }

    pub fn sanitized(mut self) -> Self {
        let d = Self::default();
        fn within<T: PartialOrd + Copy>(v: &mut T, range: std::ops::RangeInclusive<T>, fallback: T) {
            if !range.contains(v) {
                *v = fallback;
            }
        }
        within(&mut self.hook_port, 1..=65535, d.hook_port);
        within(&mut self.banner_secs, 2..=60, d.banner_secs);
        // The Avisos panel holds four rows.
        within(&mut self.notifications_per_page, 2..=4, d.notifications_per_page);
        within(&mut self.countdown_hours, 1..=48, d.countdown_hours);
        within(&mut self.gap_px, 0..=40, d.gap_px);
        within(&mut self.agenda_reminder_minutes, 0..=1440, d.agenda_reminder_minutes);
        within(&mut self.pomodoro_focus_secs, 60..=86_400, d.pomodoro_focus_secs);
        within(&mut self.pomodoro_break_secs, 60..=86_400, d.pomodoro_break_secs);
        within(&mut self.timer_default_secs, 60..=86_400, d.timer_default_secs);
        within(&mut self.notch_curve, 0..=100, d.notch_curve);
        within(&mut self.ui.hide_delay_ms, 200..=8000, d.ui.hide_delay_ms);
        within(&mut self.ui.line_expire_secs, 0..=600, d.ui.line_expire_secs);
        within(&mut self.ui.pill_height, 28..=56, d.ui.pill_height);
        within(&mut self.ui.banner_width, 280..=620, d.ui.banner_width);
        within(&mut self.ui.banner_height, 56..=120, d.ui.banner_height);
        within(&mut self.ui.media_title_width, 70..=280, d.ui.media_title_width);
        within(&mut self.ui.lyric_size, 12..=32, d.ui.lyric_size);
        within(&mut self.clipboard_limit, 1..=20, d.clipboard_limit);
        within(&mut self.shelf_capacity, 1..=6, d.shelf_capacity);
        within(&mut self.shelf_idle_minutes, 1..=30, d.shelf_idle_minutes);
        if !TabPage::NAV.iter().all(|t| self.modules.order.contains(t)) {
            self.modules.order = TabPage::NAV;
        }
        let mut seen = std::collections::HashSet::new();
        self.bar_items.retain(|i| seen.insert(*i));
        if self.bar_items.is_empty() {
            self.bar_items.push(BarItem::Clock);
        }
        let mut seen = std::collections::HashSet::new();
        self.control_tiles.retain(|t| seen.insert(*t as u8));
        within(&mut self.ui.pill_width, 160..=560, d.ui.pill_width);
        within(&mut self.ui.pill_height_pct, PILL_HEIGHT_PCT, d.ui.pill_height_pct);
        if !BANNER_SIZES.contains(&self.ui.banner_size) { self.ui.banner_size = d.ui.banner_size; }
        if !BANNER_WIDTHS.contains(&self.ui.banner_width_pct) { self.ui.banner_width_pct = d.ui.banner_width_pct; }
        // Read once by `migrate_units`, then unused: kept at their defaults so nothing differs by them.
        (self.ui.pill_height, self.ui.banner_width, self.ui.banner_height) = (d.ui.pill_height, d.ui.banner_width, d.ui.banner_height);
        // The keyboard hook and the old registration read the list; the rest is registered apart.
        self.ui.minimize_hotkey = crate::shortcut::to_hotkey(self.ui.minimize_keys);
        self.ui.notices_hotkey = crate::shortcut::to_hotkey(self.ui.notices_keys);
        let mut seen = std::collections::HashSet::new();
        self.notch_items.retain(|i| seen.insert(*i));
        if self.notch_items.is_empty() {
            self.notch_items.push(BarItem::Clock);
        }
        self
    }

    /// A settings.json read key by key over the defaults: a key that does not fit is dropped,
    /// the others kept. Not JSON at all: the defaults.
    pub fn load_lenient(json: &str) -> Self {
        Self::load_checked(json).0
    }

    /// `load_lenient`, and whether every key was kept (false: something was dropped or unreadable).
    pub fn load_checked(json: &str) -> (Self, bool) {
        let mut value = match serde_json::to_value(Self::default()) {
            Ok(v) => v,
            Err(_) => return (Self::default(), false),
        };
        let Ok(serde_json::Value::Object(read)) = serde_json::from_str::<serde_json::Value>(json) else {
            return (Self::default(), false);
        };
        let mut whole = true;
        let raw_ui = read.get("ui").cloned();
        for (key, field) in read {
            let mut trial = value.clone();
            trial[&key] = field;
            if serde_json::from_value::<Self>(trial.clone()).is_ok() {
                value = trial;
            } else {
                whole = false;
            }
        }
        let mut cfg = serde_json::from_value::<Self>(value).unwrap_or_default();
        cfg.migrate_units(raw_ui.as_ref());
        (cfg.sanitized(), whole)
    }
}

impl IsleConfig {
    /// Room above the pill: none for the notch, which hangs from the screen's top edge.
    pub fn top_gap(&self) -> f32 {
        if self.ui.pill_style == PillStyle::Notch { 0.0 } else { self.gap_px.max(0) as f32 }
    }

    /// A settings.json from before the units in % (and the recorded shortcuts): the pill's height and
    /// the banner's size in pixels become the nearest of mac-island's values, the old shortcut list
    /// its keys.
    pub fn migrate_units(&mut self, raw_ui: Option<&serde_json::Value>) {
        let Some(ui) = raw_ui.and_then(|v| v.as_object()) else { return };
        let px = |key: &str| ui.get(key).and_then(|v| v.as_f64()).map(|v| v as f32);
        let nearest = |list: &[u32], v: f32| *list.iter().min_by(|a, b| (**a as f32 - v).abs().total_cmp(&(**b as f32 - v).abs())).unwrap();
        if !ui.contains_key("pill_height_pct") {
            if let Some(h) = px("pill_height") {
                let pct = (h / PILL_HEIGHT_BASE * 100.0 / 5.0).round() as u32 * 5;
                self.ui.pill_height_pct = pct.clamp(*PILL_HEIGHT_PCT.start(), *PILL_HEIGHT_PCT.end());
            }
        }
        if !ui.contains_key("banner_size") && (ui.contains_key("banner_height") || ui.contains_key("banner_width")) {
            let h = px("banner_height").unwrap_or(BANNER_BASE.1);
            let w = px("banner_width").unwrap_or(BANNER_BASE.0);
            self.ui.banner_size = nearest(&BANNER_SIZES, h / BANNER_BASE.1 * 100.0);
            let scale = self.ui.banner_size as f32 / 100.0;
            self.ui.banner_width_pct = nearest(&BANNER_WIDTHS, w / BANNER_BASE.0 * 100.0 / scale);
        }
        if !ui.contains_key("minimize_keys") {
            self.ui.minimize_keys = crate::shortcut::from_hotkey(self.ui.minimize_hotkey);
        }
        if !ui.contains_key("notices_keys") {
            self.ui.notices_keys = crate::shortcut::from_hotkey(self.ui.notices_hotkey);
        }
    }

    /// The closed island's items for its style.
    pub fn bar_shown(&self) -> Vec<BarItem> {
        if self.ui.pill_style == PillStyle::Notch { self.notch_items.clone() } else { self.bar_items.clone() }
    }
}

fn unescape_windhawk_str(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek().copied() {
                Some('n') => {
                    chars.next();
                    out.push('\n');
                }
                Some('t') => {
                    chars.next();
                    out.push('\t');
                }
                Some('\\') => {
                    chars.next();
                    out.push('\\');
                }
                _ => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub(crate) fn derive_display_name(exe_path: &str) -> String {
    let lower = exe_path.to_ascii_lowercase();
    if lower.contains("tailscale") {
        "Tailscale".to_string()
    } else if lower.contains("deskflow") {
        "Deskflow".to_string()
    } else if lower.contains("googledrive") || lower.contains("drive file stream") {
        "Google Drive".to_string()
    } else {
        exe_path
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(exe_path)
            .trim_end_matches(".exe")
            .to_string()
    }
}

impl IsleConfig {
    pub fn from_windhawk_backup(raw: &str) -> Self {
        let mut cfg = Self::default();
        let mut kv: HashMap<&str, &str> = HashMap::new();

        for line in raw.lines() {
            if let Some((k, v)) = line.split_once('=') {
                kv.insert(k.trim(), v.trim());
            }
        }

        if let Some(gap) = kv.get("iislandGap").and_then(|v| v.parse::<i32>().ok()) {
            cfg.gap_px = gap;
        }
        if let Some(curve) = kv.get("iislandNotchCurve").and_then(|v| v.parse::<u32>().ok()) {
            cfg.notch_curve = curve;
        }
        if let Some(ah) = kv.get("iislandAutoHide").and_then(|v| v.parse::<u32>().ok()) {
            cfg.auto_hide = ah != 0;
        }
        if let Some(bg) = kv.get("iislandColorBackground").and_then(|v| v.parse::<u32>().ok()) {
            let r = ((bg >> 16) & 0xFF) as u8;
            let g = ((bg >> 8) & 0xFF) as u8;
            let b = (bg & 0xFF) as u8;
            cfg.background_rgb = (r, g, b);
        }
        if let Some(focus) = kv.get("ipomodoroFocus").and_then(|v| v.parse::<u32>().ok()) {
            cfg.pomodoro_focus_secs = focus;
        }
        if let Some(brk) = kv.get("ipomodoroBreak").and_then(|v| v.parse::<u32>().ok()) {
            cfg.pomodoro_break_secs = brk;
        }
        if let Some(timer) = kv.get("itimerLength").and_then(|v| v.parse::<u32>().ok()) {
            cfg.timer_default_secs = timer;
        }
        if let Some(all) = kv.get("iislandAllMonitors").and_then(|v| v.parse::<u32>().ok()) {
            cfg.all_monitors = all != 0;
        }
        if let Some(claude) = kv.get("iislandAiClaude").and_then(|v| v.parse::<u32>().ok()) {
            cfg.ai_claude_enabled = claude != 0;
        }
        if let Some(codex) = kv.get("iislandAiCodex").and_then(|v| v.parse::<u32>().ok()) {
            cfg.ai_codex_enabled = codex != 0;
        }

        if let Some(raw_monitors) = kv.get("smonitorPlaces") {
            let unescaped = unescape_windhawk_str(raw_monitors);
            let monitors: Vec<String> = unescaped
                .lines()
                .filter_map(|entry| {
                    let parts: Vec<&str> = entry.split('\t').collect();
                    parts.get(2).map(|m| m.trim().to_string()).filter(|m| !m.is_empty())
                })
                .collect();
            if !monitors.is_empty() {
                cfg.monitors = monitors;
            }
        }

        if let Some(main) = kv.get("sislandMonitor").map(|v| unescape_windhawk_str(v)) {
            cfg.main_monitor = Some(main.trim().to_string()).filter(|m| !m.is_empty());
        }

        if let Some(raw_weather) = kv.get("sweatherPlace") {
            let unescaped = unescape_windhawk_str(raw_weather);
            if let Some(first_line) = unescaped.lines().next() {
                let parts: Vec<&str> = first_line.split('\t').collect();
                if parts.len() >= 3 {
                    if let (Ok(lat), Ok(lon)) = (parts[1].parse::<f64>(), parts[2].parse::<f64>()) {
                        cfg.weather = WeatherConfig {
                            label: parts[0].trim().to_string(),
                            latitude: lat,
                            longitude: lon,
                        };
                    }
                }
            }
        }

        if let Some(raw_agendas) = kv.get("sagendas") {
            let unescaped = unescape_windhawk_str(raw_agendas);
            for line in unescaped.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if let Some(url) = parts.get(2).map(|s| s.trim()).filter(|s| s.starts_with("http")) {
                    cfg.calendar_ics_url = Some(url.to_string());
                    break;
                }
            }
        }

        if let Some(raw_tray) = kv.get("strayPinned") {
            let unescaped = unescape_windhawk_str(raw_tray);
            let mut apps = Vec::new();
            for line in unescaped.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let (exe, guid) = match line.split_once('|') {
                    Some((e, g)) => {
                        let guid_opt = if g == "0" || g.is_empty() {
                            None
                        } else {
                            Some(g.to_string())
                        };
                        (e.to_string(), guid_opt)
                    }
                    None => (line.to_string(), None),
                };
                apps.push(PinnedTrayApp {
                    display_name: derive_display_name(&exe),
                    exe_path: exe,
                    guid,
                });
            }
            cfg.pinned_tray_apps = apps;
        }

        if let Some(raw_folders) = kv.get("sfolderPins") {
            let unescaped = unescape_windhawk_str(raw_folders);
            cfg.folder_pins = unescaped
                .lines()
                .filter_map(|l| l.strip_prefix("place:"))
                .map(|s| s.trim().to_string())
                .collect();
        }

        cfg
    }

    pub fn load_or_import(settings_path: &Path, backup_path: Option<&Path>) -> Result<Self> {
        if settings_path.exists() {
            let content = fs::read_to_string(settings_path)?;
            let (cfg, whole) = Self::load_checked(&content);
            if !whole {
                // The next save rewrites it: the original stays beside it, as settings.json.bad.
                let mut bad = settings_path.as_os_str().to_owned();
                bad.push(".bad");
                fs::write(std::path::PathBuf::from(bad), &content)?;
            }
            Ok(cfg)
        } else {
            let cfg = if let Some(bp) = backup_path.filter(|p| p.exists()) {
                let raw = fs::read_to_string(bp)?;
                Self::from_windhawk_backup(&raw)
            } else {
                Self::default()
            };
            if let Some(parent) = settings_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(settings_path, serde_json::to_string_pretty(&cfg)?)?;
            Ok(cfg)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn holidays_country_is_the_chosen_one_else_the_system_one_else_brazil() {
        let mut c = super::IsleConfig::default();
        assert_eq!(c.holidays_country_or(Some("PT".into())), "PT");
        assert_eq!(c.holidays_country_or(None), "BR");
        c.holidays_country = "US".into();
        assert_eq!(c.holidays_country_or(Some("PT".into())), "US");
    }

    #[test]
    fn agent_token_is_generated_once_and_saved_with_the_configuration() {
        let folder = std::env::temp_dir().join(format!("cimori-auth-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let path = folder.join("settings.json");
        let mut first = super::IsleConfig::load_or_import(&path, None).unwrap();
        assert!(first.ensure_hooks_token().unwrap());
        super::write_atomic(&path, &serde_json::to_string(&first).unwrap()).unwrap();
        assert!(first.hooks_local_only);
        let token = first.hooks_token.unwrap();
        assert_eq!(token.len(), 64);
        let again = super::IsleConfig::load_or_import(&path, None).unwrap();
        assert_eq!(again.hooks_token.as_deref(), Some(token.as_str()));
        super::write_atomic(&path, &serde_json::to_string(&super::IsleConfig::default()).unwrap()).unwrap();
        let after_stale_save = super::IsleConfig::load_or_import(&path, None).unwrap();
        assert_eq!(after_stale_save.hooks_token.as_deref(), Some(token.as_str()));
        std::fs::remove_dir_all(folder).unwrap();
    }
    use super::*;

    #[test]
    fn parses_real_windhawk_backup_with_escaped_tabs_and_newlines() {
        // A real backup is private (it holds a calendar's secret address): read when a developer has one
        // beside the workspace, skipped elsewhere (CI). Only its shape is checked, never its values.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Mac Island & Dock 2026-11-10.txt");
        let Ok(raw) = std::fs::read_to_string(&path) else { return };
        let cfg = IsleConfig::from_windhawk_backup(&raw);
        assert!(!cfg.monitors.is_empty() && cfg.monitors.iter().all(|m| m.starts_with(r"\\.\DISPLAY")));
        assert!(!cfg.weather.label.is_empty() && cfg.weather.label.contains(", "));
        assert!(cfg.calendar_ics_url.as_deref().is_some_and(|url| url.starts_with("https://")));
        assert!(!cfg.pinned_tray_apps.is_empty() && cfg.pinned_tray_apps.iter().all(|a| !a.display_name.contains('\t')));
        assert!(cfg.folder_pins.iter().all(|p| !p.contains('\n')));
    }

    #[test]
    fn settings_saved_before_a_field_existed_still_load() {
        let old = r#"{"monitors": ["\\\\.\\DISPLAY1"], "gap_px": 6}"#;
        let cfg: IsleConfig = serde_json::from_str(old).unwrap();
        assert_eq!(cfg.gap_px, 6);
        assert!(cfg.all_monitors);
        assert_eq!(cfg.hook_port, 47830);
    }
}
