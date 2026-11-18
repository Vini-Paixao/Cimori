// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
//! Port of IconInfo/kIcons/kOnlineIconPacks (mac-island 3.1.1, 32608-33525).
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Wifi,
    Bluetooth,
    NightLight,
    Project,
    Cast,
    Captions,
    Accessibility,
    TaskManager,
    Explorer,

    Spark,
    Controls,
    Calendar,
    Sun,
    Music,
    Bell,
    Timer,
    Tray,
    Up,
    Down,
    Left,
    Right,
    Close,
    Plus,
    Send,
    Stop,
    Reset,
    Trash,
    Mic,
    MicOff,
    Volume,
    VolumeOff,
    Play,
    Pause,
    Prev,
    Next,
    Message,
    Folder,
    Clipboard,
    Usb,
    Pill,
    Palette,
    Monitor,
    EyeOff,
    Wave,
    Keyboard,
    Download,
    Info,
    Search,
    Gear,
    Power,
    Moon,
    Restart,
    Lock,
    SignOut,
    Airplane,
    Hotspot,
    Contrast,
    Crop,
    Calculator,
    Smile,
    Desktop,
    WinKey,
    Battery,
    Gauge,
    Minus,
}
pub struct IconInfo { pub glyph: u32, pub name: &'static str, pub names: [Option<&'static str>; 9] }
impl Icon {
    pub const ALL: [Self; 65] = [Self::Wifi,Self::Bluetooth,Self::NightLight,Self::Project,Self::Cast,Self::Captions,Self::Accessibility,Self::TaskManager,Self::Explorer,Self::Spark,Self::Controls,Self::Calendar,Self::Sun,Self::Music,Self::Bell,Self::Timer,Self::Tray,Self::Up,Self::Down,Self::Left,Self::Right,Self::Close,Self::Plus,Self::Send,Self::Stop,Self::Reset,Self::Trash,Self::Mic,Self::MicOff,Self::Volume,Self::VolumeOff,Self::Play,Self::Pause,Self::Prev,Self::Next,Self::Message,Self::Folder,Self::Clipboard,Self::Usb,Self::Pill,Self::Palette,Self::Monitor,Self::EyeOff,Self::Wave,Self::Keyboard,Self::Download,Self::Info,Self::Search,Self::Gear,Self::Power,Self::Moon,Self::Restart,Self::Lock,Self::SignOut,Self::Airplane,Self::Hotspot,Self::Contrast,Self::Crop,Self::Calculator,Self::Smile,Self::Desktop,Self::WinKey,Self::Battery,Self::Gauge,Self::Minus];
    // Port of FindIcon (mac-island 3.1.1); UI variants use the nearest existing glyph.
    pub fn info(self) -> &'static IconInfo { match self {
        Self::Wifi => &IconInfo { glyph: 0xE701, name: "wifi", names: [Some("wifi_1"),Some("wifi-outline"),Some("wifi"),Some("wifi"),Some("wifi-high"),Some("wifi"),Some("wifi"),Some("wifi"),Some("wifi")] },
        Self::Bluetooth => &IconInfo { glyph: 0xE702, name: "bluetooth", names: [Some("bluetooth"),Some("bluetooth-outline"),Some("bluetooth"),Some("bluetooth"),Some("bluetooth"),Some("bluetooth"),Some("bluetooth"),Some("bluetooth"),None] },
        Self::NightLight => &IconInfo { glyph: 0xF08C, name: "night-light", names: [Some("weather_moon"),Some("moon-outline"),Some("sun-moon"),Some("sun-moon"),Some("moon-stars"),Some("moon_stars"),Some("moon-stars"),Some("half-moon"),Some("moon")] },
        Self::Project => &IconInfo { glyph: 0xEBC6, name: "project", names: [Some("cast"),Some("tv-outline"),Some("monitor-up"),Some("screen-share"),Some("monitor"),Some("screen_share"),Some("cast"),Some("tv"),Some("tv")] },
        Self::Cast => &IconInfo { glyph: 0xEC15, name: "cast", names: [Some("cast"),Some("tv-outline"),Some("cast"),Some("cast"),Some("screencast"),Some("cast"),Some("cast"),Some("airplay"),Some("tv")] },
        Self::Captions => &IconInfo { glyph: 0xE7F0, name: "captions", names: [Some("closed_caption"),Some("chatbox-outline"),Some("captions"),Some("badge-cc"),Some("closed-captioning"),Some("closed_caption"),Some("badge-cc"),None,None] },
        Self::Accessibility => &IconInfo { glyph: 0xE776, name: "accessibility", names: [Some("accessibility"),Some("accessibility-outline"),Some("accessibility"),Some("accessible"),Some("person-arms-spread"),Some("accessibility"),Some("universal-access"),Some("accessibility"),None] },
        Self::TaskManager => &IconInfo { glyph: 0xE9D9, name: "task-manager", names: [Some("pulse"),Some("pulse-outline"),Some("activity"),Some("activity"),Some("pulse"),Some("monitoring"),Some("activity"),Some("activity"),Some("chart-bar")] },
        Self::Explorer => &IconInfo { glyph: 0xEC50, name: "explorer", names: [Some("folder"),Some("folder-outline"),Some("folder"),Some("folder"),Some("folder"),Some("folder"),Some("folder"),Some("folder"),Some("folder")] },

        Self::Spark => &IconInfo { glyph: 0xE734, name: "favorite-star", names: [Some("star"),Some("star-outline"),Some("star"),Some("star"),Some("star"),Some("star"),Some("star"),Some("star"),Some("star")] },
        Self::Controls => &IconInfo { glyph: 0xE9E9, name: "control-center", names: [Some("options"),Some("options-outline"),Some("sliders-horizontal"),Some("adjustments-horizontal"),Some("sliders-horizontal"),Some("sliders"),Some("sliders"),Some("control-slider"),Some("adjustments-horizontal")] },
        Self::Calendar => &IconInfo { glyph: 0xE787, name: "calendar", names: [Some("calendar"),Some("calendar-outline"),Some("calendar"),Some("calendar"),Some("calendar"),Some("calendar_today"),Some("calendar"),Some("calendar"),Some("calendar")] },
        Self::Sun => &IconInfo { glyph: 0xE706, name: "brightness", names: [Some("weather_sunny"),Some("sunny-outline"),Some("sun"),Some("sun"),Some("sun"),Some("brightness_5"),Some("sun"),Some("sun-light"),Some("sun")] },
        Self::Music => &IconInfo { glyph: 0xE8D6, name: "music", names: [Some("music_note_2"),Some("musical-notes-outline"),Some("music"),Some("music"),Some("music-notes"),Some("music_note_2"),Some("music-note"),Some("music-note"),Some("musical-note")] },
        Self::Bell => &IconInfo { glyph: 0xEA8F, name: "bell", names: [Some("alert"),Some("notifications-outline"),Some("bell"),Some("bell"),Some("bell"),Some("notifications"),Some("bell"),Some("bell"),Some("bell")] },
        Self::Timer => &IconInfo { glyph: 0xE916, name: "timer", names: [Some("timer"),Some("timer-outline"),Some("timer"),Some("hourglass"),Some("timer"),Some("timer"),Some("hourglass"),Some("timer"),Some("clock")] },
        Self::Tray => &IconInfo { glyph: 0xE71D, name: "apps", names: [Some("apps"),Some("apps-outline"),Some("layout-grid"),Some("layout-grid"),Some("grid-four"),Some("apps"),Some("grid"),Some("view-grid"),Some("squares-2x2")] },
        Self::Up => &IconInfo { glyph: 0xE70E, name: "chevron-up", names: [Some("chevron_up"),Some("chevron-up-outline"),Some("chevron-up"),Some("chevron-up"),Some("caret-up"),Some("keyboard_arrow_up"),Some("chevron-up"),Some("nav-arrow-up"),Some("chevron-up")] },
        Self::Down => &IconInfo { glyph: 0xE70D, name: "chevron-down", names: [Some("chevron_down"),Some("chevron-down-outline"),Some("chevron-down"),Some("chevron-down"),Some("caret-down"),Some("keyboard_arrow_down"),Some("chevron-down"),Some("nav-arrow-down"),Some("chevron-down")] },
        Self::Left => &IconInfo { glyph: 0xE76B, name: "chevron-left", names: [Some("chevron_left"),Some("chevron-back-outline"),Some("chevron-left"),Some("chevron-left"),Some("caret-left"),Some("chevron_left"),Some("chevron-left"),Some("nav-arrow-left"),Some("chevron-left")] },
        Self::Right => &IconInfo { glyph: 0xE76C, name: "chevron-right", names: [Some("chevron_right"),Some("chevron-forward-outline"),Some("chevron-right"),Some("chevron-right"),Some("caret-right"),Some("chevron_right"),Some("chevron-right"),Some("nav-arrow-right"),Some("chevron-right")] },
        Self::Close => &IconInfo { glyph: 0xE711, name: "close", names: [Some("dismiss"),Some("close-outline"),Some("x"),Some("x"),Some("x"),Some("close"),Some("x"),Some("x"),Some("x-mark")] },
        Self::Plus => &IconInfo { glyph: 0xE710, name: "add", names: [Some("add"),Some("add-outline"),Some("plus"),Some("plus"),Some("plus"),Some("add"),Some("plus"),Some("plus"),Some("plus")] },
        Self::Send => &IconInfo { glyph: 0xE724, name: "send", names: [Some("send"),Some("send-outline"),Some("send"),Some("send"),Some("paper-plane-right"),Some("send"),Some("send"),Some("send"),Some("paper-airplane")] },
        Self::Stop => &IconInfo { glyph: 0xE71A, name: "stop", names: [Some("stop"),Some("stop-outline"),Some("square"),Some("player-stop"),Some("stop"),Some("stop"),Some("stop"),Some("square"),Some("stop")] },
        Self::Reset => &IconInfo { glyph: 0xE72C, name: "reload", names: [Some("arrow_clockwise"),Some("reload-outline"),Some("rotate-cw"),Some("refresh"),Some("arrow-clockwise"),Some("refresh"),Some("arrow-clockwise"),Some("refresh"),Some("arrow-path")] },
        Self::Trash => &IconInfo { glyph: 0xE74D, name: "recycle-bin", names: [Some("delete"),Some("trash-outline"),Some("trash-2"),Some("trash"),Some("trash"),Some("delete"),Some("trash"),Some("trash"),Some("trash")] },
        Self::Mic => &IconInfo { glyph: 0xE720, name: "microphone", names: [Some("mic"),Some("mic-outline"),Some("mic"),Some("microphone"),Some("microphone"),Some("mic"),Some("mic"),Some("microphone"),Some("microphone")] },
        Self::MicOff => &IconInfo { glyph: 0xF781, name: "microphone-off", names: [Some("mic_off"),Some("mic-off-outline"),Some("mic-off"),Some("microphone-off"),Some("microphone-slash"),Some("mic_off"),Some("mic-mute"),Some("microphone-mute"),None] },
        Self::Volume => &IconInfo { glyph: 0xE995, name: "volume", names: [Some("speaker_2"),Some("volume-high-outline"),Some("volume-2"),Some("volume"),Some("speaker-high"),Some("volume_up"),Some("volume-up"),Some("sound-high"),Some("speaker-wave")] },
        Self::VolumeOff => &IconInfo { glyph: 0xE74F, name: "mute", names: [Some("speaker_mute"),Some("volume-mute-outline"),Some("volume-x"),Some("volume-3"),Some("speaker-x"),Some("volume_off"),Some("volume-mute"),Some("sound-off"),Some("speaker-x-mark")] },
        Self::Play => &IconInfo { glyph: 0xE768, name: "play", names: [Some("play"),Some("play-outline"),Some("play"),Some("player-play"),Some("play"),Some("play_arrow"),Some("play"),Some("play"),Some("play")] },
        Self::Pause => &IconInfo { glyph: 0xE769, name: "pause", names: [Some("pause"),Some("pause-outline"),Some("pause"),Some("player-pause"),Some("pause"),Some("pause"),Some("pause"),Some("pause"),Some("pause")] },
        Self::Prev => &IconInfo { glyph: 0xE892, name: "previous-track", names: [Some("previous"),Some("play-skip-back-outline"),Some("skip-back"),Some("player-track-prev"),Some("skip-back"),Some("skip_previous"),Some("skip-backward"),Some("skip-prev"),Some("backward")] },
        Self::Next => &IconInfo { glyph: 0xE893, name: "next-track", names: [Some("next"),Some("play-skip-forward-outline"),Some("skip-forward"),Some("player-track-next"),Some("skip-forward"),Some("skip_next"),Some("skip-forward"),Some("skip-next"),Some("forward")] },
        Self::Message => &IconInfo { glyph: 0xE8BD, name: "message", names: [Some("chat"),Some("chatbubble-outline"),Some("message-square"),Some("message"),Some("chat"),Some("chat"),Some("chat"),Some("message"),Some("chat-bubble-left")] },
        Self::Folder => &IconInfo { glyph: 0xE8B7, name: "folders", names: [Some("folder_open"),Some("folder-open-outline"),Some("folder-open"),Some("folder-open"),Some("folder-open"),Some("folder_open"),Some("folder2-open"),Some("folder"),Some("folder-open")] },
        Self::Clipboard => &IconInfo { glyph: 0xF0E3, name: "clipboard", names: [Some("clipboard"),Some("clipboard-outline"),Some("clipboard"),Some("clipboard"),Some("clipboard"),Some("content_paste"),Some("clipboard"),Some("clipboard-check"),Some("clipboard")] },
        Self::Usb => &IconInfo { glyph: 0xE88E, name: "drives", names: [Some("usb"),Some("server-outline"),Some("usb"),Some("usb"),Some("usb"),Some("usb"),Some("usb"),Some("usb"),Some("server")] },
        Self::Pill => &IconInfo { glyph: 0xECAA, name: "focused-app", names: [Some("window"),Some("browsers-outline"),Some("app-window"),Some("app-window"),Some("app-window"),Some("window"),Some("window"),Some("app-window"),Some("window")] },
        Self::Palette => &IconInfo { glyph: 0xE790, name: "color-filters", names: [Some("color"),Some("color-filter-outline"),Some("palette"),Some("color-filter"),Some("palette"),Some("palette"),Some("palette"),Some("color-filter"),Some("swatch")] },
        Self::Monitor => &IconInfo { glyph: 0xE7F4, name: "desktop", names: [Some("desktop"),Some("desktop-outline"),Some("monitor"),Some("device-desktop"),Some("monitor"),Some("monitor"),Some("display"),Some("computer"),Some("computer-desktop")] },
        Self::EyeOff => &IconInfo { glyph: 0xED1A, name: "hide", names: [Some("eye_off"),Some("eye-off-outline"),Some("eye-off"),Some("eye-off"),Some("eye-slash"),Some("visibility_off"),Some("eye-slash"),Some("eye-closed"),Some("eye-slash")] },
        Self::Wave => &IconInfo { glyph: 0xE9D9, name: "task-manager", names: [Some("pulse"),Some("pulse-outline"),Some("activity"),Some("activity"),Some("pulse"),Some("monitoring"),Some("activity"),Some("activity"),Some("chart-bar")] },
        Self::Keyboard => &IconInfo { glyph: 0xE765, name: "keyboard", names: [Some("keyboard"),Some("keypad-outline"),Some("keyboard"),Some("keyboard"),Some("keyboard"),Some("keyboard"),Some("keyboard"),None,None] },
        Self::Download => &IconInfo { glyph: 0xE896, name: "download", names: [Some("arrow_download"),Some("download-outline"),Some("download"),Some("download"),Some("download"),Some("download"),Some("download"),Some("download"),Some("arrow-down-tray")] },
        Self::Info => &IconInfo { glyph: 0xE946, name: "info", names: [Some("info"),Some("information-circle-outline"),Some("info"),Some("info-circle"),Some("info"),Some("info"),Some("info"),Some("info-circle"),Some("information-circle")] },
        Self::Search => &IconInfo { glyph: 0xE721, name: "search", names: [Some("search"),Some("search-outline"),Some("search"),Some("search"),Some("magnifying-glass"),Some("search"),Some("search"),Some("search"),Some("magnifying-glass")] },
        Self::Gear => &IconInfo { glyph: 0xE713, name: "settings", names: [Some("settings"),Some("settings-outline"),Some("settings"),Some("settings"),Some("gear"),Some("settings"),Some("gear"),Some("settings"),Some("cog-6-tooth")] },
        Self::Power => &IconInfo { glyph: 0xE7E8, name: "power", names: [Some("power"),Some("power-outline"),Some("power"),Some("power"),Some("power"),Some("power"),Some("power"),None,Some("power")] },
        Self::Moon => &IconInfo { glyph: 0xE708, name: "moon", names: [Some("weather_moon"),Some("moon-outline"),Some("moon"),Some("moon"),Some("moon"),Some("bedtime"),Some("moon"),Some("half-moon"),Some("moon")] },
        Self::Restart => &IconInfo { glyph: 0xE72C, name: "reload", names: [Some("arrow_clockwise"),Some("reload-outline"),Some("rotate-cw"),Some("refresh"),Some("arrow-clockwise"),Some("refresh"),Some("arrow-clockwise"),Some("refresh"),Some("arrow-path")] },
        Self::Lock => &IconInfo { glyph: 0xE72E, name: "lock", names: [Some("lock_closed"),Some("lock-closed-outline"),Some("lock"),Some("lock"),Some("lock"),Some("lock"),Some("lock"),Some("lock"),Some("lock-closed")] },
        Self::SignOut => &IconInfo { glyph: 0xF3B1, name: "sign-out", names: [Some("sign_out"),Some("log-out-outline"),Some("log-out"),Some("logout"),Some("sign-out"),Some("logout"),Some("box-arrow-right"),Some("log-out"),Some("arrow-right-on-rectangle")] },
        Self::Airplane => &IconInfo { glyph: 0xE709, name: "airplane", names: [Some("airplane"),Some("airplane-outline"),Some("plane"),Some("plane"),Some("airplane"),Some("flight"),Some("airplane"),Some("airplane"),Some("paper-airplane")] },
        Self::Hotspot => &IconInfo { glyph: 0xE88A, name: "hotspot", names: [Some("cellular_data_1"),Some("radio-outline"),Some("radio-tower"),Some("antenna-bars-5"),Some("broadcast"),Some("wifi_tethering"),Some("broadcast"),Some("antenna"),Some("radio")] },
        Self::Contrast => &IconInfo { glyph: 0xE7A1, name: "light-and-dark", names: [Some("dark_theme"),Some("invert-mode-outline"),Some("sun-moon"),Some("sun-moon"),Some("circle-half"),Some("contrast"),Some("circle-half"),Some("half-moon"),None] },
        Self::Crop => &IconInfo { glyph: 0xE722, name: "screenshot", names: [Some("screenshot"),Some("crop-outline"),Some("scissors"),Some("screenshot"),Some("scissors"),Some("crop"),Some("scissors"),Some("screenshot"),Some("scissors")] },
        Self::Calculator => &IconInfo { glyph: 0xE8EF, name: "calculator", names: [Some("calculator"),Some("calculator-outline"),Some("calculator"),Some("calculator"),Some("calculator"),Some("calculate"),Some("calculator"),Some("calculator"),Some("calculator")] },
        Self::Smile => &IconInfo { glyph: 0xE76E, name: "emoji", names: [Some("emoji"),Some("happy-outline"),Some("smile"),Some("mood-smile"),Some("smiley"),Some("mood"),Some("emoji-smile"),Some("emoji"),Some("face-smile")] },
        Self::Desktop => &IconInfo { glyph: 0xE7F4, name: "desktop", names: [Some("desktop"),Some("desktop-outline"),Some("monitor"),Some("device-desktop"),Some("monitor"),Some("monitor"),Some("display"),Some("computer"),Some("computer-desktop")] },
        Self::WinKey => &IconInfo { glyph: 0xE770, name: "system", names: [Some("desktop"),Some("desktop-outline"),Some("monitor-cog"),Some("device-desktop-cog"),Some("monitor"),Some("settings_applications"),Some("display"),Some("computer"),Some("computer-desktop")] },
        Self::Battery => &IconInfo { glyph: 0xE83F, name: "battery", names: [Some("battery_10"),Some("battery-full-outline"),Some("battery-full"),Some("battery-4"),Some("battery-full"),Some("battery_full"),Some("battery-full"),Some("battery-full"),Some("battery-100")] },
        Self::Gauge => &IconInfo { glyph: 0xEC48, name: "power-mode", names: [Some("gauge"),Some("speedometer-outline"),Some("gauge"),Some("gauge"),Some("gauge"),Some("speed"),Some("speedometer"),Some("dashboard-speed"),None] },
        Self::Minus => &IconInfo { glyph: 0xE738, name: "remove", names: [Some("subtract"),Some("remove-outline"),Some("minus"),Some("minus"),Some("minus"),Some("remove"),Some("subtract"),Some("minus"),Some("minus")] },
    } }
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum IconPack { #[default] Windows, Fluent, Ionicons, Lucide, Tabler, Phosphor, Material, Bootstrap, Iconoir, Heroicons, Folder }
impl IconPack {
    pub const ALL: [Self;11] = [Self::Windows,Self::Fluent,Self::Ionicons,Self::Lucide,Self::Tabler,Self::Phosphor,Self::Material,Self::Bootstrap,Self::Iconoir,Self::Heroicons,Self::Folder];
    pub const ONLINE: [Self;9] = [Self::Fluent,Self::Ionicons,Self::Lucide,Self::Tabler,Self::Phosphor,Self::Material,Self::Bootstrap,Self::Iconoir,Self::Heroicons];
    // Port of kOnlineIconPacks (mac-island 3.1.1): versioned folders and exact pinned URLs.
    pub fn source(self) -> Option<(&'static str, &'static str, &'static str, usize)> { Some(match self {
        Self::Fluent => ("fluent-1.1.343","https://cdn.jsdelivr.net/npm/@fluentui/svg-icons@1.1.343/icons/","_24_regular.svg",0),
        Self::Ionicons => ("ionicons-7.4.0","https://cdn.jsdelivr.net/npm/ionicons@7.4.0/dist/svg/",".svg",1),
        Self::Lucide => ("lucide-1.40.0","https://cdn.jsdelivr.net/npm/lucide-static@1.40.0/icons/",".svg",2),
        Self::Tabler => ("tabler-3.48.0","https://cdn.jsdelivr.net/npm/@tabler/icons@3.48.0/icons/outline/",".svg",3),
        Self::Phosphor => ("phosphor-2.1.1","https://cdn.jsdelivr.net/npm/@phosphor-icons/core@2.1.1/assets/regular/",".svg",4),
        Self::Material => ("material-symbols-0.47.6","https://cdn.jsdelivr.net/npm/@material-symbols/svg-400@0.47.6/outlined/",".svg",5),
        Self::Bootstrap => ("bootstrap-icons-1.13.1","https://cdn.jsdelivr.net/npm/bootstrap-icons@1.13.1/icons/",".svg",6),
        Self::Iconoir => ("iconoir-7.12.1","https://cdn.jsdelivr.net/npm/iconoir@7.12.1/icons/regular/",".svg",7),
        Self::Heroicons => ("heroicons-2.2.0","https://cdn.jsdelivr.net/npm/heroicons@2.2.0/24/outline/",".svg",8),
        _ => return None,
    }) }
    // Port of DownloadIconPack's URL construction (mac-island 3.1.1).
    pub fn url(self, icon: Icon) -> Option<String> {
        let (_,base,ending,i) = self.source()?;
        Some(format!("{base}{}{ending}", icon.info().names[i]?))
    }
}
/// Immutable pictures loaded off the UI thread; the renderer never reads disk or network.
#[derive(Default)]
pub struct IconImages { pub pack: IconPack, pub folder: String, pub pictures: HashMap<u32, Vec<u8>> }
