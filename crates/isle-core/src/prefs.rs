#![allow(clippy::collapsible_if)] // the table's setters read as one line each
//! Every option of the settings window, declared once (Mac Island & Dock's kIslandPrefs).
//! The window, its search and the tests read this table; texts are Portuguese, all here.
use crate::config::{AnimationLevel, BarItem, DateOrder, IsleConfig, PinnedTrayApp, WeatherConfig};
use crate::i18n::tr;
use crate::state::{ControlTile, TabPage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsTab {
    General,
    Bar,
    Look,
    Icons,
    Monitors,
    Hiding,
    Animations,
    Ai,
    Control,
    Agenda,
    Weather,
    Media,
    Notifications,
    Focus,
    Tray,
    Clipboard,
    Shelf,
    Usb,
    Downloads,
    Shortcuts,
    Backup,
    Updates,
    About,
    /// The first run's steps (Mac Island & Dock's setup assistant): `WELCOME`.
    Welcome,
}

impl SettingsTab {
    pub const ALL: [Self; 24] = [
        Self::General, Self::Bar, Self::Look, Self::Monitors, Self::Hiding, Self::Animations,
        Self::Icons,
        Self::Ai, Self::Control, Self::Agenda, Self::Weather, Self::Media, Self::Notifications,
        Self::Focus, Self::Tray, Self::Clipboard, Self::Shelf, Self::Usb, Self::Downloads, Self::Shortcuts, Self::Backup, Self::Updates, Self::About, Self::Welcome,
    ];
    /// Its texts are `settings_tab.<key>.title`, `.subtitle` and `.group` in locales/.
    fn text(self, field: &str) -> &'static str {
        crate::i18n::lookup(&format!("settings_tab.{}.{field}", self.key())).unwrap_or("")
    }
    pub fn title(self) -> &'static str {
        self.text("title")
    }
    pub fn subtitle(self) -> &'static str {
        self.text("subtitle")
    }
    pub fn group(self) -> &'static str {
        tr(match self {
            Self::General | Self::Bar | Self::Look | Self::Icons | Self::Monitors | Self::Hiding | Self::Animations => "settings.group_island",
            Self::Shortcuts | Self::Backup | Self::Updates | Self::About | Self::Welcome => "settings.group_cimori",
            _ => "settings.group_modules",
        })
    }
    /// The module behind a module tab, as its switch reads it.
    pub fn module(self) -> Option<fn(&IsleConfig) -> bool> {
        Some(match self {
            Self::Ai => |c| c.modules.ai,
            Self::Control => |c| c.modules.control,
            Self::Agenda => |c| c.modules.agenda,
            Self::Weather => |c| c.modules.weather,
            Self::Media => |c| c.modules.media,
            Self::Notifications => |c| c.modules.notifications,
            Self::Focus => |c| c.modules.timer,
            Self::Tray => |c| c.modules.tray,
            Self::Clipboard => |c| c.modules.clipboard,
            Self::Shelf => |c| c.modules.shelf,
            Self::Usb => |c| c.modules.usb,
            Self::Downloads => |c| c.modules.downloads,
            _ => return None,
        })
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Bar => "bar",
            Self::Look => "look",
            Self::Icons => "icons",
            Self::Monitors => "monitors",
            Self::Hiding => "hiding",
            Self::Animations => "animations",
            Self::Ai => "ai",
            Self::Control => "control",
            Self::Agenda => "agenda",
            Self::Weather => "weather",
            Self::Media => "media",
            Self::Notifications => "notifications",
            Self::Focus => "focus",
            Self::Tray => "tray",
            Self::Clipboard => "clipboard",
            Self::Shelf => "shelf",
            Self::Usb => "usb",
            Self::Downloads => "downloads",
            Self::Shortcuts => "shortcuts",
            Self::Backup => "backup",
            Self::Updates => "updates",
            Self::About => "about",
            Self::Welcome => "welcome",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.key() == key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionId {
    OpenFolder,
    Quit,
    InstallHooks,
    RemoveHooks,
    Export,
    Import,
    Restore,
    OpenWindowsBanners,
    OpenBridgeReadme,
    PickIconFolder,
    DeleteIconPack,
    ReloadIcons,
    /// Opens this address in the browser.
    OpenLink(&'static str),
    CheckUpdates,
    DownloadUpdate,
    InstallUpdate,
    WelcomeBack,
    WelcomeNext,
    WelcomeFinish,
    ShowWelcome,
    /// The Windhawk bridge's source to the clipboard.
    CopyBridge,
}

/// The welcome's steps: their titles' keys and rows, in order (Mac Island & Dock's SetupStep).
pub const WELCOME: [(&str, &[&str]); 5] = [
    ("welcome_step.hello", &["welcome.intro", "welcome.later", "language", "start_with_windows", "welcome.next"]),
    ("welcome_step.look", &["ui.pill_style", "accent_rgb", "ui.open_tab", "welcome.back", "welcome.next"]),
    ("welcome_step.agents", &["welcome.agents", "welcome.claude", "welcome.codex", "action.install_hooks", "welcome.back", "welcome.next"]),
    ("welcome_step.bridge", &["welcome.bridge", "info.bridge", "welcome.bridge1", "welcome.bridge2", "welcome.bridge3", "welcome.bridge4", "welcome.back", "welcome.next"]),
    ("welcome_step.done", &["welcome.done", "welcome.back", "welcome.finish"]),
];

/// The public repository and its pages.
pub const REPO: &str = "https://github.com/Vini-Paixao/Cimori";
pub const ISSUES: &str = "https://github.com/Vini-Paixao/Cimori/issues";
pub const LICENSE: &str = "https://github.com/Vini-Paixao/Cimori/blob/main/LICENSE";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrefKind {
    Switch,
    Choice(&'static [&'static str]),
    Slider { min: f32, max: f32, step: f32, unit: &'static str },
    Color(&'static [(u8, u8, u8)]),
    Text { secret: bool, placeholder: &'static str },
    /// Items that come in or go out and change order (`chip_choices`; none listed: the values
    /// themselves, e.g. apps seen at run time).
    Chips,
    Action(ActionId),
    /// The monitors (`monitors`: those with an island; `main_monitor`: the main one).
    Monitors,
    /// `weather`: a name searched online, its results picked.
    CitySearch,
    /// Read-only text the app fills in (version, hooks' state).
    Info,
    /// Keys recorded by a click and a press (`shortcut::code` as `PrefValue::Index`; 0 none).
    Shortcut,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrefValue {
    Bool(bool),
    Index(usize),
    Number(f32),
    Rgb(u8, u8, u8),
    Text(String),
    List(Vec<String>),
}

pub struct Pref {
    pub id: &'static str,
    pub tab: SettingsTab,
    /// The card it sits in (`card.<this>` in locales/).
    pub card: &'static str,
    pub kind: PrefKind,
    pub get: fn(&IsleConfig) -> PrefValue,
    pub set: fn(&mut IsleConfig, PrefValue),
    pub enabled: fn(&IsleConfig) -> bool,
    /// Takes effect when Cimori opens again (the row says so).
    pub restart: bool,
}

/// Its texts are `pref.<id>.name`, `.desc` and `.words` (extra words the search finds it by) in
/// locales/; a module's switch and a link share theirs.
impl Pref {
    fn text(&self, field: &str) -> Option<&'static str> {
        crate::i18n::lookup(&format!("pref.{}.{field}", self.id))
    }
    pub fn name(&self) -> &'static str {
        self.text("name").unwrap_or(self.id)
    }
    pub fn desc(&self) -> &'static str {
        self.text("desc").or_else(|| (self.card == "module").then(|| tr("pref.module.desc"))).unwrap_or("")
    }
    pub fn words(&self) -> &'static str {
        self.text("words")
            .or_else(|| (self.card == "module").then(|| tr("pref.module.words")))
            .or_else(|| matches!(self.kind, PrefKind::Action(ActionId::OpenLink(_))).then(|| tr("pref.link.words")))
            .unwrap_or("")
    }
    pub fn card_title(&self) -> &'static str {
        crate::i18n::lookup(&format!("card.{}", self.card)).unwrap_or(self.card)
    }
    /// A Choice's options as shown: keys turned into texts, labels such as "24 h" as they are.
    pub fn choices(&self) -> Vec<&'static str> {
        match self.kind {
            PrefKind::Choice(_) if self.id == "language" => std::iter::once(tr("settings.language_system"))
                .chain(crate::i18n::LANGUAGES.iter().map(|l| l.name)).collect(),
            PrefKind::Choice(options) => options.iter().map(|o| tr(o)).collect(),
            _ => Vec::new(),
        }
    }
}

pub fn restart_note() -> &'static str {
    tr("settings.restart_note")
}

fn always(_: &IsleConfig) -> bool {
    true
}
fn none(_: &IsleConfig) -> PrefValue {
    PrefValue::Text(String::new())
}
fn ignore(_: &mut IsleConfig, _: PrefValue) {}

macro_rules! switch {
    ($id:literal, $tab:ident, $card:literal, $($f:ident).+, $en:expr) => {
        Pref { id: $id, tab: SettingsTab::$tab, card: $card,
            kind: PrefKind::Switch,
            get: |c| PrefValue::Bool(c.$($f).+),
            set: |c, v| if let PrefValue::Bool(b) = v { c.$($f).+ = b },
            enabled: $en, restart: false }
    };
}
macro_rules! slider {
    ($id:literal, $tab:ident, $card:literal,
     $min:literal..$max:literal step $step:literal $unit:literal, $($f:ident).+ / $div:literal as $t:ty, $en:expr) => {
        Pref { id: $id, tab: SettingsTab::$tab, card: $card,
            kind: PrefKind::Slider { min: $min, max: $max, step: $step, unit: $unit },
            get: |c| PrefValue::Number(c.$($f).+ as f32 / $div as f32),
            set: |c, v| if let PrefValue::Number(n) = v { c.$($f).+ = (n.clamp($min, $max) * $div as f32).round() as $t },
            enabled: $en, restart: false }
    };
}
macro_rules! action {
    ($id:literal, $tab:ident, $card:literal, $a:ident) => {
        Pref { id: $id, tab: SettingsTab::$tab, card: $card,
            kind: PrefKind::Action(ActionId::$a), get: none, set: ignore, enabled: always, restart: false }
    };
}
macro_rules! link {
    ($id:literal, $tab:ident, $card:literal, $url:expr) => {
        Pref { id: $id, tab: SettingsTab::$tab, card: $card,
            kind: PrefKind::Action(ActionId::OpenLink($url)), get: none, set: ignore, enabled: always, restart: false }
    };
}
macro_rules! info {
    ($id:literal, $tab:ident, $card:literal) => {
        Pref { id: $id, tab: SettingsTab::$tab, card: $card,
            kind: PrefKind::Info, get: none, set: ignore, enabled: always, restart: false }
    };
}
macro_rules! module {
    ($id:literal, $tab:ident, $f:ident) => {
        Pref { id: $id, tab: SettingsTab::$tab, card: "module",
            kind: PrefKind::Switch,
            get: |c| PrefValue::Bool(c.modules.$f),
            set: |c, v| if let PrefValue::Bool(b) = v { c.modules.$f = b },
            enabled: always, restart: false }
    };
}

/// The panel's tabs in the user's order.
fn shown_tabs(c: &IsleConfig) -> impl Iterator<Item = TabPage> + '_ {
    c.modules.order.into_iter().filter(|t| c.modules.shows(*t))
}

fn on_ai(c: &IsleConfig) -> bool { c.modules.ai }
fn on_control(c: &IsleConfig) -> bool { c.modules.control }
fn on_agenda(c: &IsleConfig) -> bool { c.modules.agenda }
fn on_weather(c: &IsleConfig) -> bool { c.modules.weather }
fn on_media(c: &IsleConfig) -> bool { c.modules.media }
fn on_notifications(c: &IsleConfig) -> bool { c.modules.notifications }
fn on_timer(c: &IsleConfig) -> bool { c.modules.timer }
fn on_tray(c: &IsleConfig) -> bool { c.modules.tray }
fn with_date(c: &IsleConfig) -> bool { c.clock_date }

const ACCENTS: &[(u8, u8, u8)] = &[(241, 139, 99), (122, 162, 247), (158, 206, 106), (187, 154, 247), (224, 224, 224)];
const BACKGROUNDS: &[(u8, u8, u8)] = &[(0x1C, 0x1C, 0x1E), (32, 35, 42), (12, 12, 14), (40, 36, 48)];

const BAR_CHIPS: &[(&str, &str)] = &[
    ("clock", "bar.clock"),
    ("next", "bar.next"),
    ("date", "bar.date"),
    ("media", "bar.media"),
    ("weather", "bar.weather"),
    ("notifications", "bar.notifications"),
    ("timer", "bar.timer"),
    ("tray", "bar.tray"),
    ("power", "bar.power"),
];
/// The panel's tabs, in `TabPage::NAV`'s order, by the names they show.
const TAB_CHIPS: &[(&str, &str)] = &[
    ("media", "tab.media"), ("notifications", "tab.notifications"), ("timer", "tab.timer"), ("shelf", "tab.shelf"),
    ("clipboard", "tab.clipboard"), ("downloads", "tab.downloads"), ("agenda", "tab.agenda"), ("weather", "tab.weather"),
    ("usb", "tab.usb"), ("control", "tab.control"), ("tray", "tab.tray"), ("ai", "tab.ai"),
];
pub fn tab_key(t: TabPage) -> &'static str {
    TabPage::NAV.iter().position(|n| *n == t).map_or("", |i| TAB_CHIPS[i].0)
}
const TILE_CHIPS: &[(&str, &str)] = &[
    ("wifi", "tile.wifi"),
    ("bluetooth", "tile.bluetooth"),
    ("dnd", "tile.dnd"),
    ("night", "tile.night"),
    ("project", "tile.project"),
    ("cast", "tile.cast"),
    ("captions", "tile.captions"),
    ("accessibility", "tile.accessibility"),
    ("taskmgr", "tile.taskmgr"),
    ("explorer", "tile.explorer"),
    ("windowsSettings", "tile.windowsSettings"),
    ("power", "tile.power"),
    ("hotspot", "tile.hotspot"),
    ("airplane", "tile.airplane"),
    ("darkMode", "tile.darkMode"),
    ("screenshot", "tile.screenshot"),
    ("lock", "tile.lock"),
    ("calculator", "tile.calculator"),
    ("emoji", "tile.emoji"),
    ("clipboard", "tile.clipboard"),
    ("showDesktop", "tile.showDesktop"),
    ("onScreenKeyboard", "tile.onScreenKeyboard"),
    ("touchKeyboard", "tile.touchKeyboard"),
    ("winXMenu", "tile.winXMenu"),
    ("microphone", "tile.microphone"),
    ("display", "tile.display"),
    ("battery", "tile.battery"),
    ("powermode", "tile.powermode"),
    ("drives", "tile.drives"),
    ("recycleBin", "tile.recycleBin"),
    ("timer", "tile.timer"),
    ("pomodoro", "tile.pomodoro"),
    ("alarms", "tile.alarms"),
    ("stopwatch", "tile.stopwatch"),
    ("weather", "tile.weather"),
    ("nextEvent", "tile.nextEvent"),
];

pub fn bar_key(i: BarItem) -> &'static str {
    BAR_CHIPS[match i {
        BarItem::Clock => 0,
        BarItem::Next => 1,
        BarItem::Date => 2,
        BarItem::Media => 3,
        BarItem::Weather => 4,
        BarItem::Notifications => 5,
        BarItem::Timer => 6,
        BarItem::Tray => 7,
        BarItem::Power => 8,
    }]
    .0
}
pub fn bar_from_key(k: &str) -> Option<BarItem> {
    use BarItem::*;
    [Clock, Next, Date, Media, Weather, Notifications, Timer, Tray, Power].into_iter().find(|i| bar_key(*i) == k)
}
pub fn tile_key(t: ControlTile) -> &'static str {
    TILE_CHIPS[ControlTile::CATALOG.iter().position(|x| *x == t).unwrap_or(0)].0
}
pub fn tile_from_key(k: &str) -> Option<ControlTile> {
    ControlTile::CATALOG.into_iter().find(|t| tile_key(*t) == k)
}

/// (key, label's key) of a Chips row's possible items; empty: the values are their own labels.
pub fn chip_choices(id: &str) -> &'static [(&'static str, &'static str)] {
    match id {
        "bar_items" => BAR_CHIPS,
        "control_tiles" => TILE_CHIPS,
        "notch_items" => BAR_CHIPS,
        "modules.order" => TAB_CHIPS,
        _ => &[],
    }
}

/// A model row's chips: one marked at most (the chosen model), none for the CLI's default.
fn one_model(model: &str) -> PrefValue {
    PrefValue::List(if model.is_empty() { vec![] } else { vec![model.to_owned()] })
}
/// A chip touched: the one added (the last) is the model now; the marked one touched again, the default.
fn pick_model(slot: &mut String, v: PrefValue) {
    if let PrefValue::List(list) = v {
        *slot = list.into_iter().rfind(|m| *m != *slot).unwrap_or_default();
    }
}

fn text_opt(v: &Option<String>) -> PrefValue {
    PrefValue::Text(v.clone().unwrap_or_default())
}
fn set_opt(slot: &mut Option<String>, v: PrefValue) {
    if let PrefValue::Text(t) = v {
        let t = t.trim().to_string();
        *slot = (!t.is_empty()).then_some(t);
    }
}

// Port of the mod's recorded shortcuts (islandMinimizeShortcut; ControlPanel::StartShortcutRecording):
// any keys with Win, Ctrl or Alt; the same keys leave the other shortcut.
macro_rules! hotkey {
    ($id:literal, $field:ident, $other:ident, $enabled:expr) => {
        Pref { id: $id, tab: SettingsTab::Shortcuts, card: "keys",
            kind: PrefKind::Shortcut,
            get: |c| PrefValue::Index(c.ui.$field as usize),
            set: |c, v| if let PrefValue::Index(i) = v {
                let code = i as u32;
                if code == 0 || crate::shortcut::accepts(code) {
                    c.ui.$field = code;
                    if code != 0 && c.ui.$other == code { c.ui.$other = 0; }
                    c.ui.minimize_hotkey = crate::shortcut::to_hotkey(c.ui.minimize_keys);
                    c.ui.notices_hotkey = crate::shortcut::to_hotkey(c.ui.notices_keys);
                }
            },
            enabled: $enabled, restart: false }
    };
}

static PREFS: &[Pref] = &[
    switch!("liquid_glass", Look, "colors", liquid_glass, always),
    Pref { id: "icon_pack", tab: SettingsTab::Icons, card: "pack", kind: PrefKind::Choice(&["Windows", "Fluent", "Ionicons", "Lucide", "Tabler", "Phosphor", "Material", "Bootstrap", "Iconoir", "Heroicons", "choice.your_folder"]),
        get: |c| PrefValue::Index(crate::icons::IconPack::ALL.iter().position(|p|*p==c.icon_pack).unwrap_or(0)),
        set: |c,v| if let PrefValue::Index(i)=v { if let Some(p)=crate::icons::IconPack::ALL.get(i) {c.icon_pack=*p;} }, enabled: always, restart: false },
    Pref { id: "icon_folder", tab: SettingsTab::Icons, card: "your_folder", kind: PrefKind::Text { secret: false, placeholder: "placeholder.icon_folder" }, get: |c| PrefValue::Text(c.icon_folder.clone()),
        set: |c,v| if let PrefValue::Text(t)=v { c.icon_folder=t }, enabled: |c| c.icon_pack==crate::icons::IconPack::Folder, restart: false },
    action!("action.icon_folder", Icons, "your_folder", PickIconFolder),
    action!("action.reload_icons", Icons, "pack", ReloadIcons),
    Pref { enabled: |c| c.icon_pack.source().is_some(), ..action!("action.delete_icon_pack", Icons, "pack", DeleteIconPack) },
    info!("info.icons", Icons, "pack"),
    info!("info.icon_credits", Icons, "credits"),
    // Geral
    // "" follows Windows' display language; the options come from i18n::LANGUAGES (Pref::choices).
    Pref { id: "language", tab: SettingsTab::General, card: "language", kind: PrefKind::Choice(&[]),
        get: |c| PrefValue::Index(crate::i18n::LANGUAGES.iter().position(|l| l.code == c.language).map_or(0, |i| i + 1)),
        set: |c, v| if let PrefValue::Index(i) = v {
            c.language = i.checked_sub(1).and_then(|i| crate::i18n::LANGUAGES.get(i)).map_or(String::new(), |l| l.code.to_owned())
        },
        enabled: always, restart: false },
    action!("action.open_folder", General, "cimori", OpenFolder),
    switch!("start_with_windows", General, "cimori", start_with_windows, always),
    action!("action.quit", General, "cimori", Quit),
    // The page opened first (isle's _defaultPage falls back to Music); the names in TabPage::NAV's order.
    Pref { id: "ui.open_tab", tab: SettingsTab::General, card: "open_island", kind: PrefKind::Choice(&["tab.media", "tab.notifications", "tab.timer", "tab.shelf", "tab.clipboard", "tab.downloads", "tab.agenda", "tab.weather", "tab.usb", "tab.control", "tab.tray", "tab.ai"]),
        get: |c| PrefValue::Index(crate::state::TabPage::NAV.iter().position(|t| *t == c.ui.open_tab).unwrap_or(0)),
        set: |c, v| if let PrefValue::Index(i) = v { if let Some(t) = crate::state::TabPage::NAV.get(i) { c.ui.open_tab = *t } },
        enabled: always, restart: false },
    Pref { id: "hook_port", tab: SettingsTab::General, card: "agent_server", kind: PrefKind::Text { secret: false, placeholder: "47830" },
        get: |c| PrefValue::Text(c.hook_port.to_string()),
        set: |c, v| if let PrefValue::Text(t) = v { if let Ok(p) = t.trim().parse::<u16>() { if p > 0 { c.hook_port = p } } },
        enabled: always, restart: true },
    Pref { restart: true, ..switch!("hooks_local_only", General, "agent_server", hooks_local_only, always) },
    // Barra
    Pref { id: "bar_items", tab: SettingsTab::Bar, card: "items", kind: PrefKind::Chips,
        get: |c| PrefValue::List(c.bar_items.iter().map(|i| bar_key(*i).to_string()).collect()),
        set: |c, v| if let PrefValue::List(keys) = v {
            let items: Vec<_> = keys.iter().filter_map(|k| bar_from_key(k)).collect();
            if !items.is_empty() { c.bar_items = items }
        },
        enabled: always, restart: false },
    // Port of IslandPref::ClockCenter (mac-island 3.1.1).
    switch!("ui.clock_center", Bar, "items", ui.clock_center, always),
    switch!("bar_quota_strip", Bar, "items", bar_quota_strip, always),
    Pref { id: "clock_24h", tab: SettingsTab::Bar, card: "clock", kind: PrefKind::Choice(&["24 h", "12 h"]),
        get: |c| PrefValue::Index(if c.clock_24h { 0 } else { 1 }),
        set: |c, v| if let PrefValue::Index(i) = v { c.clock_24h = i == 0 },
        enabled: always, restart: false },
    switch!("clock_seconds", Bar, "clock", clock_seconds, always),
    switch!("clock_weekday", Bar, "clock", clock_weekday, always),
    switch!("clock_date", Bar, "clock", clock_date, always),
    switch!("clock_year", Bar, "clock", clock_year, with_date),
    Pref { id: "date_order", tab: SettingsTab::Bar, card: "clock", kind: PrefKind::Choice(&["choice.day_month", "choice.month_day"]),
        get: |c| PrefValue::Index(if c.date_order == DateOrder::MonthDay { 1 } else { 0 }),
        set: |c, v| if let PrefValue::Index(i) = v {
            c.date_order = if i == 1 { DateOrder::MonthDay } else { DateOrder::WeekdayDayMonth }
        },
        enabled: with_date, restart: false },
    switch!("month_number", Bar, "clock", month_number, with_date),
    switch!("countdown_events", Bar, "next", countdown_events, always),
    switch!("countdown_timer", Bar, "next", countdown_timer, always),
    switch!("countdown_pomodoro", Bar, "next", countdown_pomodoro, always),
    slider!("countdown_hours", Bar, "next",
        1.0..48.0 step 1.0 "h", countdown_hours / 1 as u32, always),
    // Aparência
    Pref { id: "accent_rgb", tab: SettingsTab::Look, card: "colors", kind: PrefKind::Color(ACCENTS),
        get: |c| { let (r, g, b) = c.accent_rgb; PrefValue::Rgb(r, g, b) },
        set: |c, v| if let PrefValue::Rgb(r, g, b) = v { c.accent_rgb = (r, g, b) },
        enabled: always, restart: false },
    Pref { id: "background_rgb", tab: SettingsTab::Look, card: "colors", kind: PrefKind::Color(BACKGROUNDS),
        get: |c| { let (r, g, b) = c.background_rgb; PrefValue::Rgb(r, g, b) },
        set: |c, v| if let PrefValue::Rgb(r, g, b) = v { c.background_rgb = (r, g, b) },
        enabled: always, restart: false },
    slider!("gap_px", Look, "size", 0.0..40.0 step 1.0 "px", gap_px / 1 as i32, always),
    switch!("ui.tooltips", Look, "interaction", ui.tooltips, always),
    switch!("ui.hover_highlight", Look, "interaction", ui.hover_highlight, always),
    // The chips are the tabs whose module is on; dragging them reorders those, the hidden ones
    // keep their places. Adding or taking one out is the modules' job: a list of other tabs is ignored.
    Pref { id: "modules.order", tab: SettingsTab::Look, card: "open_panel", kind: PrefKind::Chips,
        get: |c| PrefValue::List(shown_tabs(c).map(|t| tab_key(t).to_string()).collect()),
        set: |c, v| if let PrefValue::List(names) = v {
            let tabs: Vec<TabPage> = names.iter().filter_map(|n| TabPage::NAV.into_iter().find(|t| tab_key(*t) == n)).collect();
            let shown: Vec<_> = shown_tabs(c).collect();
            if tabs.len() != shown.len() || !shown.iter().all(|t| tabs.contains(t)) { return; }
            let (m, mut next) = (c.modules, tabs.into_iter());
            for slot in c.modules.order.iter_mut().filter(|t| m.shows(**t)) {
                *slot = next.next().unwrap_or(*slot);
            }
        },
        enabled: always, restart: false },
    switch!("ui.uniform_panels", Look, "size", ui.uniform_panels, always),
    slider!("ui.pill_height_pct", Look, "size", 70.0..130.0 step 5.0 "%", ui.pill_height_pct / 1 as u32, always),
    slider!("notch_curve", Look, "size", 0.0..100.0 step 1.0 "%", notch_curve / 1 as u32, always),
    // Posição e monitores
    switch!("all_monitors", Monitors, "monitors", all_monitors, always),
    Pref { id: "monitors", tab: SettingsTab::Monitors, card: "monitors", kind: PrefKind::Monitors,
        get: |c| PrefValue::List(c.monitors.clone()),
        set: |c, v| if let PrefValue::List(m) = v { if !m.is_empty() { c.monitors = m } },
        enabled: |c| !c.all_monitors, restart: false },
    Pref { id: "main_monitor", tab: SettingsTab::Monitors, card: "monitors", kind: PrefKind::Monitors,
        get: |c| text_opt(&c.main_monitor), set: |c, v| set_opt(&mut c.main_monitor, v), enabled: always, restart: false },
    switch!("over_fullscreen", Monitors, "fullscreen", over_fullscreen, always),
    switch!("ui.follow_mouse", Monitors, "monitors", ui.follow_mouse, always),
    // Esconder
    switch!("auto_hide", Hiding, "hide", auto_hide, always),
    Pref { id: "ui.hide_when", tab: SettingsTab::Hiding, card: "hide", kind: PrefKind::Choice(&["choice.mouse_away", "choice.window_maximized", "choice.window_over"]),
        get: |c| PrefValue::Index(c.ui.hide_when as usize),
        set: |c, v| if let PrefValue::Index(i) = v {
            if let Some(value) = crate::config::HideWhen::ALL.get(i) { c.ui.hide_when = *value; }
        },
        enabled: |c| c.auto_hide, restart: false },
    switch!("hide_maximized", Hiding, "vanish", hide_maximized, always),
    Pref { id: "hide_apps", tab: SettingsTab::Hiding, card: "vanish", kind: PrefKind::Chips,
        get: |c| PrefValue::List(c.hide_apps.clone()),
        set: |c, v| if let PrefValue::List(apps) = v { c.hide_apps = apps },
        enabled: |c| !c.hide_maximized, restart: false },
    slider!("ui.hide_delay_ms", Hiding, "collapse", 0.2..8.0 step 0.1 "s", ui.hide_delay_ms / 1000 as u32, always),
    slider!("ui.line_expire_secs", Hiding, "minimized_line", 0.0..600.0 step 5.0 "s", ui.line_expire_secs / 1 as u32, |c| c.auto_hide),
    switch!("ui.line_click", Hiding, "minimized_line", ui.line_click, |c| c.auto_hide),
    switch!("ui.line_full_width", Hiding, "minimized_line", ui.line_full_width, |c| c.auto_hide),
    switch!("ui.hover_open", Hiding, "collapse", ui.hover_open, always),
    // Animações
    Pref { id: "animations", tab: SettingsTab::Animations, card: "animations", kind: PrefKind::Choice(&["choice.follow_windows", "choice.on", "choice.off"]),
        get: |c| PrefValue::Index(match c.animations { AnimationLevel::System => 0, AnimationLevel::On => 1, AnimationLevel::Off => 2 }),
        set: |c, v| if let PrefValue::Index(i) = v {
            c.animations = [AnimationLevel::System, AnimationLevel::On, AnimationLevel::Off][i.min(2)]
        },
        enabled: always, restart: false },
    // IA
    module!("modules.ai", Ai, ai),
    Pref { id: "ui.header_usage", tab: SettingsTab::Ai, card: "panel_top", kind: PrefKind::Choice(&["Claude", "Codex", "choice.both", "choice.neither"]),
        get: |c| PrefValue::Index(c.ui.header_usage.min(3) as usize),
        set: |c, v| if let PrefValue::Index(i) = v { c.ui.header_usage = i.min(3) as u8 },
        enabled: on_ai, restart: false },
    switch!("ai_claude_enabled", Ai, "agents", ai_claude_enabled, on_ai),
    switch!("ai_codex_enabled", Ai, "agents", ai_codex_enabled, on_ai),
    Pref { id: "ai_claude_model", tab: SettingsTab::Ai, card: "chat_models", kind: PrefKind::Choice(&["choice.default", "Opus", "Sonnet", "Haiku"]),
        get: |c| PrefValue::Index(["", "opus", "sonnet", "haiku"].iter().position(|m| *m == c.ai_claude_model).unwrap_or(0)),
        set: |c, v| if let PrefValue::Index(i) = v { c.ai_claude_model = ["", "opus", "sonnet", "haiku"].get(i).copied().unwrap_or("").into() },
        enabled: on_ai, restart: false },
    Pref { id: "ai_codex_model", tab: SettingsTab::Ai, card: "chat_models", kind: PrefKind::Chips, get: |c| one_model(&c.ai_codex_model), set: |c, v| pick_model(&mut c.ai_codex_model, v), enabled: on_ai, restart: false },
    Pref { id: "ai_opencode_model", tab: SettingsTab::Ai, card: "chat_models", kind: PrefKind::Chips, get: |c| one_model(&c.ai_opencode_model), set: |c, v| pick_model(&mut c.ai_opencode_model, v), enabled: on_ai, restart: false },
    Pref { id: "opencode_api_key", tab: SettingsTab::Ai, card: "agents", kind: PrefKind::Text { secret: true, placeholder: "sk-..." },
        get: |c| text_opt(&c.opencode_api_key), set: |c, v| set_opt(&mut c.opencode_api_key, v), enabled: on_ai, restart: false },
    switch!("claude_renew_login", Ai, "quotas", claude_renew_login, on_ai),
    switch!("quota_alerts", Ai, "quotas", quota_alerts, on_ai),
    info!("info.hooks", Ai, "hooks"),
    Pref { id: "hooks_token", tab: SettingsTab::Ai, card: "hooks", kind: PrefKind::Text { secret: true, placeholder: "placeholder.generated" }, get: |c| text_opt(&c.hooks_token), set: |c, v| if let PrefValue::Text(text) = v { let text = text.trim(); if text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()) { c.hooks_token = Some(text.into()); } }, enabled: on_ai, restart: true },
    action!("action.install_hooks", Ai, "hooks", InstallHooks),
    action!("action.remove_hooks", Ai, "hooks", RemoveHooks),
    // Central de controle
    module!("modules.control", Control, control),
    Pref { id: "control_tiles", tab: SettingsTab::Control, card: "buttons", kind: PrefKind::Chips,
        get: |c| PrefValue::List(c.control_tiles.iter().map(|t| tile_key(*t).to_string()).collect()),
        set: |c, v| if let PrefValue::List(keys) = v { c.control_tiles = keys.iter().filter_map(|k| tile_from_key(k)).collect() },
        enabled: on_control, restart: false },
    switch!("control_mixer", Control, "sound", control_mixer, on_control),
    switch!("control_outputs", Control, "sound", control_outputs, on_control),
    switch!("wheel_volume", Control, "sound", wheel_volume, on_control),
    switch!("ui.volume_percent", Control, "sound", ui.volume_percent, on_control),
    switch!("ui.mic_button", Control, "sound", ui.mic_button, on_control),
    // Agenda
    module!("modules.agenda", Agenda, agenda),
    Pref { id: "calendar_ics_url", tab: SettingsTab::Agenda, card: "calendar", kind: PrefKind::Text { secret: true, placeholder: "https://..." },
        get: |c| text_opt(&c.calendar_ics_url), set: |c, v| set_opt(&mut c.calendar_ics_url, v), enabled: on_agenda, restart: false },
    switch!("holidays", Agenda, "calendar", holidays, on_agenda),
    Pref { id: "holidays_country", tab: SettingsTab::Agenda, card: "calendar", kind: PrefKind::Text { secret: false, placeholder: "placeholder.windows_region" },
        get: |c| PrefValue::Text(c.holidays_country.clone()),
        set: |c, v| if let PrefValue::Text(t) = v {
            let t = t.trim().to_uppercase();
            if t.is_empty() || (t.len() == 2 && t.chars().all(|ch| ch.is_ascii_alphabetic())) { c.holidays_country = t }
        },
        enabled: |c| c.modules.agenda && c.holidays, restart: false },
    switch!("timezone_system", Agenda, "calendar", timezone_system, on_agenda),
    switch!("alert_sound", Agenda, "reminders", alert_sound, on_agenda),
    switch!("agenda_reminders", Agenda, "reminders", agenda_reminders, on_agenda),
    slider!("agenda_reminder_minutes", Agenda, "reminders",
        0.0..120.0 step 5.0 "min", agenda_reminder_minutes / 1 as u32, |c| c.modules.agenda && c.agenda_reminders),
    switch!("ui.agenda_featured_day", Agenda, "calendar", ui.agenda_featured_day, on_agenda),
    switch!("ui.agenda_events", Agenda, "calendar", ui.agenda_events, on_agenda),
    // Clima
    module!("modules.weather", Weather, weather),
    Pref { id: "weather", tab: SettingsTab::Weather, card: "weather", kind: PrefKind::CitySearch,
        get: |c| PrefValue::List(vec![c.weather.label.clone(), c.weather.latitude.to_string(), c.weather.longitude.to_string()]),
        set: |c, v| if let PrefValue::List(p) = v {
            if let [label, lat, lon] = p.as_slice() {
                if let (Ok(latitude), Ok(longitude)) = (lat.parse(), lon.parse()) {
                    c.weather = WeatherConfig { label: label.clone(), latitude, longitude };
                }
            }
        },
        enabled: on_weather, restart: false },
    Pref { id: "weather_fahrenheit", tab: SettingsTab::Weather, card: "weather", kind: PrefKind::Choice(&["°C", "°F"]),
        get: |c| PrefValue::Index(c.weather_fahrenheit as usize),
        set: |c, v| if let PrefValue::Index(i) = v { c.weather_fahrenheit = i == 1 },
        enabled: on_weather, restart: false },
    switch!("ui.weather_cards", Weather, "weather", ui.weather_cards, on_weather),
    // Mídia
    module!("modules.media", Media, media),
    switch!("lyrics", Media, "lyrics", lyrics, on_media),
    switch!("ui.media_controls", Media, "in_pill", ui.media_controls, on_media),
    switch!("ui.media_always", Media, "in_pill", ui.media_always, on_media),
    switch!("ui.media_play_only", Media, "in_pill", ui.media_play_only, |c| c.modules.media && c.ui.media_controls),
    switch!("ui.media_lyric_title", Media, "in_pill", ui.media_lyric_title, |c| c.modules.media && c.lyrics),
    slider!("ui.media_title_width", Media, "in_pill", 70.0..280.0 step 5.0 "px", ui.media_title_width / 1 as u32, on_media),
    slider!("ui.lyric_size", Media, "lyrics", 12.0..32.0 step 1.0 "px", ui.lyric_size / 1 as u32, |c| c.modules.media && c.lyrics),
    switch!("ui.media_marquee", Media, "in_pill", ui.media_marquee, on_media),
    switch!("ui.media_wheel", Media, "in_pill", ui.media_wheel, on_media),
    Pref { id: "ui.lyric_rgb", tab: SettingsTab::Media, card: "lyrics", kind: PrefKind::Color(&[(245,241,235), (241,139,99), (122,162,247), (158,206,106)]),
        get: |c| { let (r, g, b) = c.ui.lyric_rgb; PrefValue::Rgb(r, g, b) },
        set: |c, v| if let PrefValue::Rgb(r, g, b) = v { c.ui.lyric_rgb = (r, g, b); },
        enabled: |c| c.modules.media && c.lyrics, restart: false },
    // Avisos
    module!("modules.notifications", Notifications, notifications),
    switch!("banners", Notifications, "banners", banners, on_notifications),
    switch!("ui.banner_over_apps", Notifications, "banners", ui.banner_over_apps, on_notifications),
    slider!("banner_secs", Notifications, "banners",
        2.0..30.0 step 1.0 "s", banner_secs / 1 as u32, |c| c.modules.notifications && c.banners),
    Pref { restart: true, ..switch!("notification_icons", Notifications, "list", notification_icons, on_notifications) },
    slider!("notifications_per_page", Notifications, "list",
        2.0..4.0 step 1.0 "", notifications_per_page / 1 as u32, on_notifications),
    Pref { id: "muted_apps", tab: SettingsTab::Notifications, card: "muted_apps", kind: PrefKind::Chips,
        get: |c| PrefValue::List(c.muted_apps.clone()),
        set: |c, v| if let PrefValue::List(apps) = v { c.muted_apps = apps },
        enabled: on_notifications, restart: false },
    action!("action.windows_banners", Notifications, "windows_banners", OpenWindowsBanners),
    Pref { id: "ui.banner_size", tab: SettingsTab::Notifications, card: "banners", kind: PrefKind::Choice(&["50%", "75%", "100%", "125%", "150%", "200%"]),
        get: |c| PrefValue::Index(crate::config::BANNER_SIZES.iter().position(|s| *s == c.ui.banner_size).unwrap_or(2)),
        set: |c, v| if let PrefValue::Index(i) = v { if let Some(s) = crate::config::BANNER_SIZES.get(i) { c.ui.banner_size = *s } },
        enabled: on_notifications, restart: false },
    Pref { id: "ui.banner_width_pct", tab: SettingsTab::Notifications, card: "banners", kind: PrefKind::Choice(&["80%", "100%", "130%", "160%"]),
        get: |c| PrefValue::Index(crate::config::BANNER_WIDTHS.iter().position(|s| *s == c.ui.banner_width_pct).unwrap_or(1)),
        set: |c, v| if let PrefValue::Index(i) = v { if let Some(s) = crate::config::BANNER_WIDTHS.get(i) { c.ui.banner_width_pct = *s } },
        enabled: on_notifications, restart: false },
    switch!("ui.banner_when_hidden", Notifications, "banners", ui.banner_when_hidden, on_notifications),
    // Foco
    module!("modules.timer", Focus, timer),
    slider!("pomodoro_focus_secs", Focus, "pomodoro", 5.0..180.0 step 5.0 "min", pomodoro_focus_secs / 60 as u32, on_timer),
    slider!("pomodoro_break_secs", Focus, "pomodoro", 1.0..60.0 step 1.0 "min", pomodoro_break_secs / 60 as u32, on_timer),
    slider!("timer_default_secs", Focus, "timer", 1.0..120.0 step 1.0 "min", timer_default_secs / 60 as u32, on_timer),
    switch!("ui.timer_on_line", Focus, "timer", ui.timer_on_line, on_timer),
    // Bandeja
    module!("modules.tray", Tray, tray),
    Pref { id: "pinned_tray_apps", tab: SettingsTab::Tray, card: "pinned", kind: PrefKind::Chips,
        get: |c| PrefValue::List(c.pinned_tray_apps.iter().map(|a| a.exe_path.clone()).collect()),
        set: |c, v| if let PrefValue::List(paths) = v {
            let old = std::mem::take(&mut c.pinned_tray_apps);
            c.pinned_tray_apps = paths.into_iter().map(|p| old.iter().find(|a| a.exe_path == p).cloned()
                .unwrap_or_else(|| PinnedTrayApp { display_name: crate::config::derive_display_name(&p), exe_path: p, guid: None }))
                .collect();
        },
        enabled: on_tray, restart: false },
    info!("info.bridge", Tray, "windhawk_bridge"),
    action!("action.bridge_readme", Tray, "windhawk_bridge", OpenBridgeReadme),
    switch!("ui.tray_grid", Tray, "list", ui.tray_grid, on_tray),
    switch!("ui.tray_keep_open", Tray, "list", ui.tray_keep_open, on_tray),
    module!("modules.clipboard", Clipboard, clipboard),
    module!("modules.shelf", Shelf, shelf),
    module!("modules.usb", Usb, usb),
    module!("modules.downloads", Downloads, downloads),
    slider!("clipboard_limit", Clipboard, "history", 1.0..20.0 step 1.0 "", clipboard_limit / 1 as u32, |c| c.modules.clipboard),
    switch!("clipboard_images", Clipboard, "history", clipboard_images, |c| c.modules.clipboard),
    switch!("clipboard_files", Clipboard, "history", clipboard_files, |c| c.modules.clipboard),
    switch!("clipboard_notice", Clipboard, "notice", clipboard_notice, |_| true),
    Pref { restart: true, ..switch!("clipboard_persist", Clipboard, "history", clipboard_persist, |c| c.modules.clipboard) },
    switch!("clipboard_paste", Clipboard, "history", clipboard_paste, |c| c.modules.clipboard),
    switch!("clipboard_thumbnails", Clipboard, "history", clipboard_thumbnails, |c| c.modules.clipboard),
    Pref { id: "clipboard_ignored_apps", tab: SettingsTab::Clipboard, card: "privacy", kind: PrefKind::Text { secret: false, placeholder: "KeePassXC.exe, 1Password.exe" }, get: |c| PrefValue::Text(c.clipboard_ignored_apps.join(", ")), set: |c, v| if let PrefValue::Text(text) = v { c.clipboard_ignored_apps = text.split([',', ';']).map(str::trim).filter(|s| !s.is_empty() && !s.contains(['/', '\\'])).map(str::to_owned).collect() }, enabled: always, restart: false },
    Pref { restart: true, ..switch!("shelf_persist", Shelf, "shelf", shelf_persist, |c| c.modules.shelf) },
    switch!("shelf_drag_move", Shelf, "shelf", shelf_drag_move, |c| c.modules.shelf),
    switch!("shelf_drop_pill", Shelf, "shelf", shelf_drop_pill, |c| c.modules.shelf),
    slider!("shelf_capacity", Shelf, "shelf", 1.0..6.0 step 1.0 "", shelf_capacity / 1 as u32, |c| c.modules.shelf),
    switch!("shelf_qr", Shelf, "sharing", shelf_qr, |c| c.modules.shelf),
    switch!("shelf_receive", Shelf, "sharing", shelf_receive, |c| c.modules.shelf && c.shelf_qr),
    slider!("shelf_idle_minutes", Shelf, "sharing", 1.0..30.0 step 1.0 "min", shelf_idle_minutes / 1 as u32, |c| c.modules.shelf && c.shelf_qr),
    switch!("usb_alerts", Usb, "drives", usb_alerts, |c| c.modules.usb),
    switch!("download_meter", Downloads, "progress", download_meter, |c| c.modules.downloads),
    switch!("download_taskbar", Downloads, "progress", download_taskbar, |c| c.modules.downloads),
    switch!("download_animation", Downloads, "progress", download_animation, |c| c.modules.downloads),
    switch!("download_details", Downloads, "list", download_details, |c| c.modules.downloads),
    switch!("download_recent", Downloads, "list", download_recent, |c| c.modules.downloads),
    // Atalhos
    hotkey!("ui.minimize_hotkey", minimize_keys, notices_keys, always),
    hotkey!("ui.notices_hotkey", notices_keys, minimize_keys, |c| c.win_n && c.modules.notifications),
    switch!("win_n", Shortcuts, "keys", win_n, on_notifications),
    // Backup
    action!("action.export", Backup, "backup", Export),
    action!("action.import", Backup, "backup", Import),
    action!("action.restore", Backup, "defaults", Restore),
    // Sobre
    Pref { ..info!("info.version", About, "cimori") },
    link!("link.repo", About, "cimori", REPO),
    link!("link.issues", About, "cimori", ISSUES),
    Pref { ..link!("link.license", About, "cimori", LICENSE) },
    action!("action.show_welcome", About, "cimori", ShowWelcome),
    // Atualizações
    Pref { ..info!("info.update", Updates, "version") },
    action!("action.check_updates", Updates, "version", CheckUpdates),
    action!("action.install_update", Updates, "version", InstallUpdate),
    action!("action.download_update", Updates, "version", DownloadUpdate),
    switch!("check_updates", Updates, "automatic", check_updates, always),
    link!("link.isle", About, "credits", "https://github.com/zickrian/isle"),
    link!("link.mac_island", About, "credits", "https://island.caliberda.com.br"),
    Pref { ..info!("info.credits_mit", About, "credits") },
    Pref { ..info!("info.credits_icons", About, "credits") },
    Pref { ..info!("info.credits_crates", About, "credits") },
    // Estilo: a pílula ou o entalhe (Mac Island & Dock's islandStyle/islandPillNotches; isle's notch).
    Pref { id: "ui.pill_style", tab: SettingsTab::Look, card: "style", kind: PrefKind::Choice(&["choice.pill", "choice.notch"]),
        get: |c| PrefValue::Index((c.ui.pill_style == crate::config::PillStyle::Notch) as usize),
        set: |c, v| if let PrefValue::Index(i) = v {
            c.ui.pill_style = if i == 1 { crate::config::PillStyle::Notch } else { crate::config::PillStyle::Island }
        },
        enabled: always, restart: false },
    slider!("ui.pill_width", Look, "size",
        160.0..560.0 step 10.0 "px", ui.pill_width / 1 as u32, always),
    Pref { id: "notch_items", tab: SettingsTab::Bar, card: "items", kind: PrefKind::Chips,
        get: |c| PrefValue::List(c.notch_items.iter().map(|i| bar_key(*i).to_string()).collect()),
        set: |c, v| if let PrefValue::List(keys) = v {
            let items: Vec<_> = keys.iter().filter_map(|k| bar_from_key(k)).collect();
            if !items.is_empty() { c.notch_items = items }
        },
        enabled: |c| c.ui.pill_style == crate::config::PillStyle::Notch, restart: false },
    // Botão de desligar (Mac Island & Dock's islandPower*; PowerActionOn, PowerAsks).
    switch!("power.sleep", Control, "power_button", power.sleep, always),
    switch!("power.hibernate", Control, "power_button", power.hibernate, always),
    switch!("power.restart", Control, "power_button", power.restart, always),
    switch!("power.shut_down", Control, "power_button", power.shut_down, always),
    switch!("power.lock", Control, "power_button", power.lock, always),
    switch!("power.sign_out", Control, "power_button", power.sign_out, always),
    switch!("power.confirm", Control, "power_button", power.confirm, always),
    switch!("ui.arrow_pins", Hiding, "interaction", ui.arrow_pins, |c| c.auto_hide),
    // Boas-vindas (WELCOME): rows of their own; the others come from their tabs.
    Pref { ..info!("welcome.intro", Welcome, "welcome") },
    Pref { ..info!("welcome.later", Welcome, "welcome") },
    Pref { ..info!("welcome.agents", Welcome, "agents") },
    info!("welcome.claude", Welcome, "agents"),
    info!("welcome.codex", Welcome, "agents"),
    Pref { ..info!("welcome.bridge", Welcome, "bridge") },
    link!("welcome.bridge1", Welcome, "bridge", "https://windhawk.net"),
    info!("welcome.bridge2", Welcome, "bridge"),
    action!("welcome.bridge3", Welcome, "bridge", CopyBridge),
    Pref { ..info!("welcome.bridge4", Welcome, "bridge") },
    Pref { ..info!("welcome.done", Welcome, "done") },
    action!("welcome.back", Welcome, "steps", WelcomeBack),
    action!("welcome.next", Welcome, "steps", WelcomeNext),
    action!("welcome.finish", Welcome, "steps", WelcomeFinish),
];

pub fn all() -> &'static [Pref] {
    PREFS
}

pub fn by_id(id: &str) -> Option<&'static Pref> {
    PREFS.iter().find(|p| p.id == id)
}

/// Lowercase, without accents, for the search.
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'ê' | 'è' | 'ë' => 'e',
            'í' | 'î' | 'ì' | 'ï' => 'i',
            'ó' | 'ô' | 'õ' | 'ò' | 'ö' => 'o',
            'ú' | 'û' | 'ù' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

/// The rows whose name, description, words or tab title hold every word of `query`.
pub fn search(query: &str) -> Vec<&'static Pref> {
    let words: Vec<String> = fold(query).split_whitespace().map(str::to_string).collect();
    if words.is_empty() {
        return Vec::new();
    }
    PREFS
        .iter()
        .filter(|p| p.tab != SettingsTab::Welcome)
        .filter(|p| {
            let hay = fold(&format!("{} {} {} {} {}", p.name(), p.desc(), p.words(), p.card_title(), p.tab.title()));
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .collect()
}
