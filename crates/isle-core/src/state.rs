// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Adapted for isle-win from Mac Island & Dock by caliberda v3.1.0 (kControls/MixerApp):
// control-center and NotificationPanel state with Portuguese labels.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AppearanceMode {
    Black,
    Glass { opacity: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TabPage {
    AiHub,
    ControlCenter,
    CalendarWeather,
    /// The forecast (the mod's weather panel).
    Weather,
    Media,
    Notifications,
    Shelf,
    Clipboard,
    Usb,
    Downloads,
    Timer,
    Tray,
    /// The power button's menu (not in the navigation: opened from its button).
    Power,
}

impl TabPage {
    /// The navigation, in isle's tab order (expanded.js TABS: music, notifications, focus, shelf, calendar,
    /// devices, usage): Cimori's own tabs beside their nearest isle page, its extras after, Uso last.
    pub const NAV: [TabPage; 12] = [
        TabPage::Media, TabPage::Notifications, TabPage::Timer, TabPage::Shelf, TabPage::Clipboard, TabPage::Downloads,
        TabPage::CalendarWeather, TabPage::Weather, TabPage::Usb, TabPage::ControlCenter, TabPage::Tray, TabPage::AiHub,
    ];
    /// The name the open tab shows beside its icon.
    pub fn label(self) -> &'static str {
        match self {
            TabPage::AiHub => "Uso",
            TabPage::ControlCenter => "Controle",
            TabPage::CalendarWeather => "Agenda",
            TabPage::Weather => "Clima",
            TabPage::Media => "Música",
            TabPage::Notifications => "Avisos",
            TabPage::Shelf => "Prateleira",
            TabPage::Clipboard => "Histórico",
            TabPage::Usb => "USB",
            TabPage::Downloads => "Downloads",
            TabPage::Timer => "Foco",
            TabPage::Tray => "Bandeja",
            TabPage::Power => "Desligar",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PillMode {
    /// Hidden to a thin line until the mouse reaches it (auto-hide).
    Minimized,
    CompactBar,
    PermissionPrompt,
    AlertBanner { title: String, subtitle: String,
    },
    Expanded(TabPage),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AiProviderKind {
    ClaudeCode,
    Codex,
    OpenCodeGo,
}

impl AiProviderKind {
    pub fn reset_ready_title(self) -> String {
        match self {
            AiProviderKind::ClaudeCode => "Claude de volta!".to_string(),
            AiProviderKind::Codex => "Codex de volta!".to_string(),
            AiProviderKind::OpenCodeGo => "OpenCode de volta!".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaAlertEvent {
    Threshold80(AiProviderKind),
    Threshold95(AiProviderKind),
    QuotaResetReady {
        provider: AiProviderKind,
        title: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotaStatus {
    pub provider: AiProviderKind,
    pub used_pct: f32,
    pub reset_at_unix: Option<u64>,
    pub warned_80: bool,
    pub warned_95: bool,
    pub armed_for_reset: bool,
    pub last_notified_reset_unix: Option<u64>,
}

impl QuotaStatus {
    pub fn new(provider: AiProviderKind) -> Self {
        Self {
            provider,
            used_pct: 0.0,
            reset_at_unix: None,
            warned_80: false,
            warned_95: false,
            armed_for_reset: false,
            last_notified_reset_unix: None,
        }
    }

    pub fn update_reading(
        &mut self,
        used_pct: f32,
        reset_at_unix: Option<u64>,
        now_unix: u64,
    ) -> Option<QuotaAlertEvent> {
        let prev_reset = self.reset_at_unix;
        let prev_used = self.used_pct;

        let window_rolled_over = match (prev_reset, reset_at_unix) {
            (Some(old_r), _) if now_unix >= old_r && self.last_notified_reset_unix != Some(old_r) =>
            {
                true
            }
            _ => used_pct + 25.0 < prev_used && self.armed_for_reset,
        };

        self.used_pct = used_pct.clamp(0.0, 100.0);
        self.reset_at_unix = reset_at_unix;

        if window_rolled_over && self.armed_for_reset {
            self.warned_80 = false;
            self.warned_95 = false;
            self.armed_for_reset = false;
            self.last_notified_reset_unix = prev_reset.or( Some(now_unix) );
            return Some(QuotaAlertEvent::QuotaResetReady {
                provider: self.provider,
                title: self.provider.reset_ready_title(),
            });
        }

        if self.used_pct >= 95.0 {
            self.armed_for_reset = true;
            if !self.warned_95 {
                self.warned_95 = true;
                self.warned_80 = true;
                return Some(QuotaAlertEvent::Threshold95(self.provider));
            }
        } else if self.used_pct >= 80.0 {
            self.armed_for_reset = true;
            if !self.warned_80 {
                self.warned_80 = true;
                return Some(QuotaAlertEvent::Threshold80(self.provider));
            }
        }

        None
    }
}

/// What plays now (Windows' media session), as the pill shows it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MediaInfo {
    #[serde(skip)]
    pub artwork: Option<std::sync::Arc<crate::artwork::Artwork>>,
    pub title: String,
    pub artist: String,
    pub playing: bool,
    pub position_ms: i64,
    pub duration_ms: i64,
    /// Unix ms when `position_ms` was read: while playing, the song has moved on since.
    pub sampled_at_ms: i64,
}

impl MediaInfo {
    /// Where the song is at `now_ms`: moving on while it plays, never past its end.
    pub fn position_at(&self, now_ms: i64) -> i64 {
        let mut position = self.position_ms;
        if self.playing {
            position += (now_ms - self.sampled_at_ms).max(0);
        }
        if self.duration_ms > 0 {
            position = position.min(self.duration_ms);
        }
        position
    }
}

/// One limit of an AI plan: how much is used and when it starts over (zickrian/isle's usage `Window`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageWindow {
    /// "5 horas", "Semana", "Semana · Opus"…
    pub label: String,
    pub percent: f64,
    /// 0 when unknown.
    pub resets_at_ms: i64,
}

/// One AI's limits as last read (zickrian/isle's usage `Provider`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageProvider {
    /// "claude" or "codex".
    pub id: String,
    pub name: String,
    pub windows: Vec<UsageWindow>,
    /// The subscription as the CLI names it ("Pro", "Max"); empty when unknown.
    #[serde(default)]
    pub plan: String,
    /// When the numbers were read; 0 when never.
    pub updated_ms: i64,
    /// Why the numbers may be old or missing ("Abra o Claude Code para renovar o login"); empty when fresh.
    pub note: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PermissionCard {
    pub request_id: String,
    pub agent: String,
    pub tool_name: String,
    pub command: String,
    /// Where the agent runs, to bring its window forward from the card: its hook's parent
    /// processes and its project (named in its window's title, as a fallback).
    pub pids: Vec<u32>,
    pub project: String,
}

impl PermissionCard {
    /// Which card a button belongs to (the UI's actions are `Copy`, so not the id itself).
    pub fn key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.request_id.hash(&mut hasher);
        hasher.finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionDecision {
    Allow,
    Deny,
    Always,
}

/// Controls shown in the same order as Mac Island & Dock's control center.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlTile {
    Wifi, Bluetooth, DoNotDisturb, NightLight, Project, Cast, Captions,
    Accessibility, TaskManager, Explorer,
    // Mac Island & Dock's ControlId catalog, not shown until added (ControlInfo::shownByDefault).
    WindowsSettings, Power,
    Hotspot, Airplane, DarkMode, Screenshot, Lock, Calculator, Emoji, Clipboard, ShowDesktop, OnScreenKeyboard, TouchKeyboard, WinX, Microphone, Brightness, Battery, PowerMode, Drives, RecycleBin, Timer, Pomodoro, Alarms, Stopwatch, Weather, NextEvent,
}

impl ControlTile {
    /// Port of ControlPanel::OnMouseUp's shortcuts that close the panel first (mac-island 3.1.1):
    /// keys pressed for the window in front, a picture taken without the island in it.
    pub fn closes_island(self) -> bool {
        matches!(self, Self::Cast | Self::Screenshot | Self::Lock | Self::Emoji | Self::Clipboard | Self::TaskManager
            | Self::Calculator | Self::Explorer | Self::ShowDesktop | Self::OnScreenKeyboard | Self::TouchKeyboard
            | Self::WindowsSettings | Self::WinX)
    }

    pub const ALL: [Self; 10] = [Self::Wifi, Self::Bluetooth, Self::DoNotDisturb,
        Self::NightLight, Self::Project, Self::Cast, Self::Captions,
        Self::Accessibility, Self::TaskManager, Self::Explorer,
    ];
    /// Every control the center can have: the defaults (`ALL`), then the ones added by hand.
    pub const CATALOG: [Self; 36] = [Self::Wifi, Self::Bluetooth, Self::DoNotDisturb,
        Self::NightLight, Self::Project, Self::Cast, Self::Captions,
        Self::Accessibility, Self::TaskManager, Self::Explorer,
        Self::WindowsSettings, Self::Power,
        Self::Hotspot, Self::Airplane, Self::DarkMode, Self::Screenshot, Self::Lock, Self::Calculator, Self::Emoji, Self::Clipboard, Self::ShowDesktop, Self::OnScreenKeyboard, Self::TouchKeyboard, Self::WinX, Self::Microphone, Self::Brightness, Self::Battery, Self::PowerMode, Self::Drives, Self::RecycleBin, Self::Timer, Self::Pomodoro, Self::Alarms, Self::Stopwatch, Self::Weather, Self::NextEvent,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::WindowsSettings => "Configurações", Self::Power => "Desligar",
            Self::Hotspot => "Hotspot móvel",
            Self::Airplane => "Modo avião",
            Self::DarkMode => "Modo escuro",
            Self::Screenshot => "Captura de tela",
            Self::Lock => "Bloquear",
            Self::Calculator => "Calculadora",
            Self::Emoji => "Emojis",
            Self::Clipboard => "Área de transferência",
            Self::ShowDesktop => "Mostrar área de trabalho",
            Self::OnScreenKeyboard => "Teclado virtual",
            Self::TouchKeyboard => "Teclado touch",
            Self::WinX => "Menu Win+X",
            Self::Microphone => "Microfone",
            Self::Brightness => "Brilho da tela",
            Self::Battery => "Bateria",
            Self::PowerMode => "Energia",
            Self::Drives => "Pendrives",
            Self::RecycleBin => "Lixeira",
            Self::Timer => "Timer",
            Self::Pomodoro => "Pomodoro",
            Self::Alarms => "Alarmes",
            Self::Stopwatch => "Cronômetro",
            Self::Weather => "Clima",
            Self::NextEvent => "Próximo evento",
            Self::Wifi => "Wi-Fi", Self::Bluetooth => "Bluetooth",
            Self::DoNotDisturb => "Não Perturbe", Self::NightLight => "Luz noturna",
            Self::Project => "Projetar", Self::Cast => "Transmitir",
            Self::Captions => "Legendas", Self::Accessibility => "Acessibilidade",
            Self::TaskManager => "Gerenciador", Self::Explorer => "Explorador",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppVolume {
    /// Executable path (or system/PID fallback), stable while the mixer is refreshed.
    pub id: String,
    pub name: String,
    pub volume_pct: u8,
    pub muted: bool,
}

/// A playback device; the default one is where sound goes now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioOutput {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IslandState {
    pub arrow_pinned: bool,
    #[serde(skip)]
    pub ui: crate::config::PillOptions,
    #[serde(skip)]
    pub phase8: crate::phase8::Phase8View,
    #[serde(skip)]
    pub line_hidden: bool,
    /// Files are dragged over this open island: "Solte para pôr na prateleira" over its panel.
    #[serde(skip)]
    pub drop_hint: bool,
    #[serde(skip)]
    pub panel_scroll:f32,
    /// The bar's items in the settings' order (`IsleConfig::bar_items`).
    #[serde(skip, default = "default_bar")]
    pub bar: Vec<crate::config::BarItem>,
    #[serde(skip, default = "yes")]
    pub bar_quota_strip: bool,
    /// What the running Pomodoro, timer or stopwatch shows on the bar's Timer item.
    #[serde(skip)]
    pub timer_text: Option<String>,
    /// Which modules are on (`IsleConfig::modules`): the navigation and the bar follow it.
    #[serde(skip)]
    pub modules: crate::config::Modules,
    /// The tray icons come from the Windhawk bridge.
    #[serde(skip)]
    pub tray_bridge: bool,
    /// The control center's buttons in the settings' order, and its sound sections.
    #[serde(skip, default = "all_tiles")]
    pub control_tiles: Vec<ControlTile>,
    #[serde(skip, default = "yes")]
    pub control_mixer: bool,
    #[serde(skip, default = "yes")]
    pub control_outputs: bool,
    #[serde(default)]
    pub tray: Vec<crate::tray::TrayItem>,
    #[serde(default)]
    pub tray_page: usize,
    #[serde(default)]
    pub tray_error: Option<String>,
    #[serde(default)]
    pub notifications: crate::notifications::NotificationCenter,
    #[serde(default)]
    pub notification_page: usize,
    #[serde(default)]
    pub notification_error: Option<String>,
    /// Snapshot of the toast shown as a banner, surviving dismissal from the Windows database.
    #[serde(skip)]
    pub notification_banner: Option<crate::notifications::Notification>,
    /// The reply being typed in the Avisos tab, if any.
    #[serde(default)]
    pub reply: Option<crate::notifications::Reply>,
    pub appearance: AppearanceMode,
    pub mode: PillMode,
    pub dnd_active: bool,
    /// Unix milliseconds at the last tick (each minute): what countdowns on the pill count from.
    pub now_ms: i64,
    pub clock_text: String,
    pub date_text: String,
    /// Local time minus UTC, in seconds (the Uso tab's "Atualizado 16:54").
    #[serde(default)]
    pub utc_offset_s: i64,
    pub ram_usage_pct: u8,
    pub volume_pct: u8,
    pub muted: bool,
    pub mic_muted: bool,
    pub audio_available: bool,
    pub mic_available: bool,
    pub app_volumes: Vec<AppVolume>,
    pub audio_outputs: Vec<AudioOutput>,
    /// Events and holidays around today (see agenda.rs), by day then time.
    pub agenda: Vec<crate::agenda::AgendaItem>,
    /// Local days since 1970: today, and the day picked in the Agenda tab.
    pub today: i64,
    pub agenda_day: i64,
    pub wifi_name: Option<String>,
    pub wifi_connected: Option<bool>,
    pub bluetooth_on: Option<bool>,
    pub night_light_on: Option<bool>,
    pub control_error: Option<String>,
    pub usb_drives_count: usize,
    /// The forecast (Open-Meteo), and why it couldn't come; the weather panel's chosen day.
    pub weather: Option<crate::weather::Weather>,
    pub weather_error: Option<String>,
    pub weather_day: usize,
    /// The Timer tab: Pomodoro, countdown, stopwatch and reminders.
    #[serde(default)]
    pub timers: crate::timers::TimersView,
    pub fahrenheit: bool,
    pub next_countdown_text: Option<String>,
    pub media_title: Option<String>,
    pub active_lyric_line: Option<String>,
    pub pending_permission: Option<PermissionCard>,
    /// Asks waiting behind `pending_permission`.
    pub permission_waiting: usize,
    pub claude_quota: QuotaStatus,
    pub codex_quota: QuotaStatus,
    pub opencode_quota: QuotaStatus,
    pub last_chat_reply: Option<String>,
    #[serde(default)]
    pub chat: crate::chat::Chat,
    /// What plays now; `None` when nothing does.
    pub media: Option<MediaInfo>,
    /// The power button's options, the action asking to be confirmed, and whether Windows can hibernate.
    #[serde(skip)]
    pub power: crate::power::PowerOptions,
    #[serde(skip)]
    pub power_confirm: Option<crate::power::PowerAction>,
    #[serde(skip)]
    pub hibernate_allowed: bool,
    /// The control center's edit mode (mac-island's ControlPanel::SetEditing), and the control being
    /// dragged in it: (from, over).
    #[serde(skip)]
    pub control_editing: bool,
    #[serde(skip)]
    pub control_drag: Option<(usize, usize)>,
    /// Every AI plan's limits, for the AI tab.
    pub usage: Vec<UsageProvider>,
}

fn default_bar() -> Vec<crate::config::BarItem> {
    crate::config::IsleConfig::default().bar_items
}
fn all_tiles() -> Vec<ControlTile> {
    ControlTile::ALL.to_vec()
}
fn yes() -> bool {
    true
}

impl Default for IslandState {
    fn default() -> Self {
        Self {
            arrow_pinned: false,
            ui: Default::default(),
            phase8: Default::default(),
            line_hidden: false,
            drop_hint: false,
            panel_scroll:0.,
            bar: default_bar(),
            bar_quota_strip: true,
            timer_text: None,
            modules: Default::default(),
            tray_bridge: false,
            control_tiles: all_tiles(),
            control_mixer: true,
            control_outputs: true,
            tray: Vec::new(),
            tray_page: 0,
            tray_error: None,
            notifications: Default::default(),
            notification_page: 0,
            notification_error: None,
            notification_banner: None,
            reply: None,
            appearance: AppearanceMode::Black,
            mode: PillMode::CompactBar,
            dnd_active: false,
            now_ms: 0,
            clock_text: "20:00".to_string(),
            date_text: String::new(),
            utc_offset_s: 0,
            ram_usage_pct: 42,
            volume_pct: 0,
            muted: false,
            mic_muted: false,
            audio_available: false,
            mic_available: false,
            app_volumes: Vec::new(),
            audio_outputs: Vec::new(),
            agenda: Vec::new(),
            today: 0,
            agenda_day: 0,
            wifi_name: None,
            wifi_connected: None,
            bluetooth_on: None,
            night_light_on: None,
            control_error: None,
            usb_drives_count: 0,
            weather: None,
            weather_error: None,
            weather_day: 0,
            timers: Default::default(),
            fahrenheit: false,
            next_countdown_text: None,
            media_title: None,
            active_lyric_line: None,
            pending_permission: None,
            permission_waiting: 0,
            claude_quota: QuotaStatus::new(AiProviderKind::ClaudeCode),
            codex_quota: QuotaStatus::new(AiProviderKind::Codex),
            opencode_quota: QuotaStatus::new(AiProviderKind::OpenCodeGo),
            last_chat_reply: None,
            chat: Default::default(),
            usage: Vec::new(),
            media: None,
            power: Default::default(),
            power_confirm: None,
            hibernate_allowed: true,
            control_editing: false,
            control_drag: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_status_emits_80_95_and_single_native_reset_notification() {
        let mut quota = QuotaStatus::new(AiProviderKind::ClaudeCode);
        assert_eq!(
            quota.update_reading(82.0, Some(2000), 1000),
            Some(QuotaAlertEvent::Threshold80(AiProviderKind::ClaudeCode))
        );
        // Subsequent poll at 84% does not re-trigger 80% warning
        assert_eq!(quota.update_reading(84.0, Some(2000), 1100), None);
        assert_eq!(
            quota.update_reading(96.0, Some(2000), 1200),
            Some(QuotaAlertEvent::Threshold95(AiProviderKind::ClaudeCode))
        );
        // When reset_at (2000) is reached and usage drops, fire native "Claude de volta!" once
        assert_eq!(
            quota.update_reading(0.0, Some(2000 + 18000), 2001),
            Some(QuotaAlertEvent::QuotaResetReady {
                provider: AiProviderKind::ClaudeCode,
                title: "Claude de volta!".to_string(),
            })
        );
        // Next poll in the new window does not repeat the reset alert
        assert_eq!(quota.update_reading(2.0, Some(2000 + 18000), 2060), None);
    }
}
