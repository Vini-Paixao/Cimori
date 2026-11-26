#![allow(clippy::collapsible_if)] // the table's setters read as one line each
//! Every option of the settings window, declared once (Mac Island & Dock's kIslandPrefs).
//! The window, its search and the tests read this table; texts are Portuguese, all here.
use crate::config::{AnimationLevel, BarItem, DateOrder, IsleConfig, PinnedTrayApp, WeatherConfig};
use crate::state::ControlTile;

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
    pub fn title(self) -> &'static str {
        match self {
            Self::General => "Geral",
            Self::Bar => "Barra",
            Self::Look => "Aparência",
            Self::Icons => "Ícones",
            Self::Monitors => "Posição e monitores",
            Self::Hiding => "Esconder",
            Self::Animations => "Animações",
            Self::Ai => "IA e agentes",
            Self::Control => "Central de controle",
            Self::Agenda => "Agenda",
            Self::Weather => "Clima",
            Self::Media => "Mídia",
            Self::Notifications => "Avisos",
            Self::Focus => "Foco e timers",
            Self::Tray => "Bandeja",
            Self::Clipboard => "Área de transferência",
            Self::Shelf => "Prateleira",
            Self::Usb => "Dispositivos USB",
            Self::Downloads => "Downloads",
            Self::Shortcuts => "Atalhos",
            Self::Backup => "Backup",
            Self::Updates => "Atualizações",
            Self::About => "Sobre",
            Self::Welcome => "Boas-vindas",
        }
    }
    pub fn subtitle(self) -> &'static str {
        match self {
            Self::General => "A Cimori e o servidor dos agentes",
            Self::Bar => "O que a pílula mostra quando está fechada",
            Self::Look => "Cores da ilha",
            Self::Icons => "Pacotes e imagens dos ícones",
            Self::Monitors => "Onde as ilhas aparecem",
            Self::Hiding => "Quando a pílula vira uma linha",
            Self::Animations => "Movimento da ilha",
            Self::Ai => "Claude Code, Codex e OpenCode",
            Self::Control => "Botões, volume e saídas de áudio",
            Self::Agenda => "Calendário, feriados e lembretes",
            Self::Weather => "Cidade e unidade",
            Self::Media => "O que está tocando",
            Self::Notifications => "Avisos do Windows na ilha",
            Self::Focus => "Pomodoro e timer",
            Self::Tray => "Ícones dos outros apps",
            Self::Clipboard => "Histórico local de itens copiados",
            Self::Shelf => "Arquivos guardados e compartilhamento por QR",
            Self::Usb => "Abrir e ejetar unidades USB",
            Self::Downloads => "Atividade de downloads no Windows",
            Self::Shortcuts => "Teclas da Cimori",
            Self::Backup => "Guardar e restaurar as configurações",
            Self::Updates => "Versões novas da Cimori",
            Self::About => "Versão e créditos",
            Self::Welcome => "Primeiros passos na Cimori",
        }
    }
    pub fn group(self) -> &'static str {
        match self {
            Self::General | Self::Bar | Self::Look | Self::Icons | Self::Monitors | Self::Hiding | Self::Animations => "Ilha",
            Self::Shortcuts | Self::Backup | Self::Updates | Self::About | Self::Welcome => "Cimori",
            _ => "Módulos",
        }
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

/// The welcome's steps: their titles and rows, in order (Mac Island & Dock's SetupStep).
pub const WELCOME: [(&str, &[&str]); 5] = [
    ("Boas-vindas", &["welcome.intro", "welcome.later", "start_with_windows", "welcome.next"]),
    ("Aparência", &["ui.pill_style", "accent_rgb", "ui.open_tab", "welcome.back", "welcome.next"]),
    ("Agentes de IA", &["welcome.agents", "welcome.claude", "welcome.codex", "action.install_hooks", "welcome.back", "welcome.next"]),
    ("Ponte do Windhawk", &["welcome.bridge", "info.bridge", "welcome.bridge1", "welcome.bridge2", "welcome.bridge3", "welcome.bridge4", "welcome.back", "welcome.next"]),
    ("Tudo pronto", &["welcome.done", "welcome.back", "welcome.finish"]),
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
    pub card: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    /// Extra words the search finds it by.
    pub words: &'static str,
    pub kind: PrefKind,
    pub get: fn(&IsleConfig) -> PrefValue,
    pub set: fn(&mut IsleConfig, PrefValue),
    pub enabled: fn(&IsleConfig) -> bool,
    /// Takes effect when Cimori opens again (the row says so).
    pub restart: bool,
}

pub const RESTART_NOTE: &str = "Vale ao reabrir a Cimori";

fn always(_: &IsleConfig) -> bool {
    true
}
fn none(_: &IsleConfig) -> PrefValue {
    PrefValue::Text(String::new())
}
fn ignore(_: &mut IsleConfig, _: PrefValue) {}

macro_rules! switch {
    ($id:literal, $tab:ident, $card:literal, $name:literal, $desc:literal, $words:literal, $($f:ident).+, $en:expr) => {
        Pref { id: $id, tab: SettingsTab::$tab, card: $card, name: $name, desc: $desc, words: $words,
            kind: PrefKind::Switch,
            get: |c| PrefValue::Bool(c.$($f).+),
            set: |c, v| if let PrefValue::Bool(b) = v { c.$($f).+ = b },
            enabled: $en, restart: false }
    };
}
macro_rules! slider {
    ($id:literal, $tab:ident, $card:literal, $name:literal, $desc:literal, $words:literal,
     $min:literal..$max:literal step $step:literal $unit:literal, $($f:ident).+ / $div:literal as $t:ty, $en:expr) => {
        Pref { id: $id, tab: SettingsTab::$tab, card: $card, name: $name, desc: $desc, words: $words,
            kind: PrefKind::Slider { min: $min, max: $max, step: $step, unit: $unit },
            get: |c| PrefValue::Number(c.$($f).+ as f32 / $div as f32),
            set: |c, v| if let PrefValue::Number(n) = v { c.$($f).+ = (n.clamp($min, $max) * $div as f32).round() as $t },
            enabled: $en, restart: false }
    };
}
macro_rules! action {
    ($id:literal, $tab:ident, $card:literal, $name:literal, $desc:literal, $words:literal, $a:ident) => {
        Pref { id: $id, tab: SettingsTab::$tab, card: $card, name: $name, desc: $desc, words: $words,
            kind: PrefKind::Action(ActionId::$a), get: none, set: ignore, enabled: always, restart: false }
    };
}
macro_rules! link {
    ($id:literal, $tab:ident, $card:literal, $name:literal, $desc:literal, $url:expr) => {
        Pref { id: $id, tab: SettingsTab::$tab, card: $card, name: $name, desc: $desc, words: "link site",
            kind: PrefKind::Action(ActionId::OpenLink($url)), get: none, set: ignore, enabled: always, restart: false }
    };
}
macro_rules! info {
    ($id:literal, $tab:ident, $card:literal, $name:literal) => {
        Pref { id: $id, tab: SettingsTab::$tab, card: $card, name: $name, desc: "", words: "",
            kind: PrefKind::Info, get: none, set: ignore, enabled: always, restart: false }
    };
}
macro_rules! module {
    ($id:literal, $tab:ident, $name:literal, $f:ident) => {
        Pref { id: $id, tab: SettingsTab::$tab, card: "Módulo", name: $name,
            desc: "Desligado, some da ilha e da barra e para de buscar dados", words: "módulo mostrar esconder ligar desligar",
            kind: PrefKind::Switch,
            get: |c| PrefValue::Bool(c.modules.$f),
            set: |c, v| if let PrefValue::Bool(b) = v { c.modules.$f = b },
            enabled: always, restart: false }
    };
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
    ("clock", "Relógio"),
    ("next", "Próximo"),
    ("date", "Data"),
    ("media", "Música"),
    ("weather", "Clima"),
    ("notifications", "Avisos"),
    ("timer", "Timer"),
    ("tray", "Bandeja"),
    ("power", "Desligar"),
];
const TILE_CHIPS: &[(&str, &str)] = &[
    ("wifi", "Wi-Fi"),
    ("bluetooth", "Bluetooth"),
    ("dnd", "Não Perturbe"),
    ("night", "Luz noturna"),
    ("project", "Projetar"),
    ("cast", "Transmitir"),
    ("captions", "Legendas"),
    ("accessibility", "Acessibilidade"),
    ("taskmgr", "Gerenciador"),
    ("explorer", "Explorador"),
    ("windowsSettings", "Configurações"),
    ("power", "Desligar"),
    ("hotspot", "Hotspot móvel"),
    ("airplane", "Modo avião"),
    ("darkMode", "Modo escuro"),
    ("screenshot", "Captura de tela"),
    ("lock", "Bloquear"),
    ("calculator", "Calculadora"),
    ("emoji", "Emojis"),
    ("clipboard", "Área de transferência"),
    ("showDesktop", "Mostrar área de trabalho"),
    ("onScreenKeyboard", "Teclado virtual"),
    ("touchKeyboard", "Teclado touch"),
    ("winXMenu", "Menu Win+X"),
    ("microphone", "Microfone"),
    ("display", "Brilho da tela"),
    ("battery", "Bateria"),
    ("powermode", "Energia"),
    ("drives", "Pendrives"),
    ("recycleBin", "Lixeira"),
    ("timer", "Timer"),
    ("pomodoro", "Pomodoro"),
    ("alarms", "Alarmes"),
    ("stopwatch", "Cronômetro"),
    ("weather", "Clima"),
    ("nextEvent", "Próximo evento"),
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

/// (key, label) of a Chips row's possible items; empty: the values are their own labels.
pub fn chip_choices(id: &str) -> &'static [(&'static str, &'static str)] {
    match id {
        "bar_items" => BAR_CHIPS,
        "control_tiles" => TILE_CHIPS,
        "notch_items" => BAR_CHIPS,
        _ => &[],
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
    ($id:literal, $name:literal, $field:ident, $other:ident, $enabled:expr) => {
        Pref { id: $id, tab: SettingsTab::Shortcuts, card: "Teclas", name: $name,
            desc: "Clique e pressione as teclas", words: "atalho teclado win gravar",
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
    switch!("liquid_glass", Look, "Cores", "Liquid Glass", "Vidro com desfoque do Windows, brilho e borda iluminada", "vidro transparência blur glass", liquid_glass, always),
    Pref { id: "icon_pack", tab: SettingsTab::Icons, card: "Pacote", name: "Ícones da interface",
        desc: "Pacotes on-line são baixados uma vez ao escolher. Ícones ausentes usam os do Windows.", words: "lucide fluent ionicons tabler phosphor material bootstrap iconoir heroicons segoe",
        kind: PrefKind::Choice(&["Windows", "Fluent", "Ionicons", "Lucide", "Tabler", "Phosphor", "Material", "Bootstrap", "Iconoir", "Heroicons", "Sua pasta"]),
        get: |c| PrefValue::Index(crate::icons::IconPack::ALL.iter().position(|p|*p==c.icon_pack).unwrap_or(0)),
        set: |c,v| if let PrefValue::Index(i)=v { if let Some(p)=crate::icons::IconPack::ALL.get(i) {c.icon_pack=*p;} }, enabled: always, restart: false },
    Pref { id: "icon_folder", tab: SettingsTab::Icons, card: "Sua pasta", name: "Pasta dos ícones", desc: "PNG ou SVG: nome do ícone ou código Segoe, como E713.svg", words: "caminho imagens pasta",
        kind: PrefKind::Text { secret: false, placeholder: "Pasta com PNG e SVG" }, get: |c| PrefValue::Text(c.icon_folder.clone()),
        set: |c,v| if let PrefValue::Text(t)=v { c.icon_folder=t }, enabled: |c| c.icon_pack==crate::icons::IconPack::Folder, restart: false },
    action!("action.icon_folder", Icons, "Sua pasta", "Escolher pasta", "Abre o seletor de pastas do Windows", "pasta imagens", PickIconFolder),
    action!("action.reload_icons", Icons, "Pacote", "Recarregar ícones", "Atualiza as imagens ou tenta baixar o que faltou", "atualizar tentar novamente", ReloadIcons),
    Pref { enabled: |c| c.icon_pack.source().is_some(), ..action!("action.delete_icon_pack", Icons, "Pacote", "Excluir pacote baixado", "Volta aos ícones do Windows", "remover cache disco", DeleteIconPack) },
    info!("info.icons", Icons, "Pacote", "Download"),
    info!("info.icon_credits", Icons, "Créditos", "Autores e licenças"),
    // Geral
    action!("action.open_folder", General, "Cimori", "Pasta das configurações", "Onde ficam o settings.json e os registros", "arquivo json log pasta", OpenFolder),
    switch!("start_with_windows", General, "Cimori", "Iniciar com o Windows", "Abre a Cimori ao entrar na sua conta", "inicializar login automático startup", start_with_windows, always),
    action!("action.quit", General, "Cimori", "Sair da Cimori", "Fecha a ilha", "fechar encerrar sair", Quit),
    // The page opened first (isle's _defaultPage falls back to Music); the names in TabPage::NAV's order.
    Pref { id: "ui.open_tab", tab: SettingsTab::General, card: "Ilha aberta", name: "Aba ao abrir",
        desc: "A página que a ilha mostra ao abrir", words: "aba página inicial abrir música uso",
        kind: PrefKind::Choice(&["Música", "Avisos", "Foco", "Prateleira", "Histórico", "Downloads", "Agenda", "Clima", "USB", "Controle", "Bandeja", "Uso"]),
        get: |c| PrefValue::Index(crate::state::TabPage::NAV.iter().position(|t| *t == c.ui.open_tab).unwrap_or(0)),
        set: |c, v| if let PrefValue::Index(i) = v { if let Some(t) = crate::state::TabPage::NAV.get(i) { c.ui.open_tab = *t } },
        enabled: always, restart: false },
    Pref { id: "hook_port", tab: SettingsTab::General, card: "Servidor dos agentes", name: "Porta",
        desc: "Onde Claude Code e Codex avisam a ilha", words: "hooks porta rede tailscale",
        kind: PrefKind::Text { secret: false, placeholder: "47830" },
        get: |c| PrefValue::Text(c.hook_port.to_string()),
        set: |c, v| if let PrefValue::Text(t) = v { if let Ok(p) = t.trim().parse::<u16>() { if p > 0 { c.hook_port = p } } },
        enabled: always, restart: true },
    Pref { restart: true, ..switch!("hooks_local_only", General, "Servidor dos agentes", "Só desta máquina e do Tailscale",
        "Recusa conexões de outras máquinas da rede", "segurança rede local tailscale", hooks_local_only, always) },
    // Barra
    Pref { id: "bar_items", tab: SettingsTab::Bar, card: "Itens", name: "Na barra",
        desc: "Arraste para mudar a ordem, para fora para tirar", words: "ordem itens ícones pílula barra",
        kind: PrefKind::Chips,
        get: |c| PrefValue::List(c.bar_items.iter().map(|i| bar_key(*i).to_string()).collect()),
        set: |c, v| if let PrefValue::List(keys) = v {
            let items: Vec<_> = keys.iter().filter_map(|k| bar_from_key(k)).collect();
            if !items.is_empty() { c.bar_items = items }
        },
        enabled: always, restart: false },
    // Port of IslandPref::ClockCenter (mac-island 3.1.1).
    switch!("ui.clock_center", Bar, "Itens", "Relógio no meio", "Os outros itens se reorganizam dos dois lados dele, na ordem da lista", "relógio centro meio centralizar alinhar", ui.clock_center, always),
    switch!("bar_quota_strip", Bar, "Itens", "Barras de cota da IA", "Embaixo da pílula", "uso ia claude codex limite", bar_quota_strip, always),
    Pref { id: "clock_24h", tab: SettingsTab::Bar, card: "Relógio", name: "Formato", desc: "", words: "relógio hora am pm",
        kind: PrefKind::Choice(&["24 h", "12 h"]),
        get: |c| PrefValue::Index(if c.clock_24h { 0 } else { 1 }),
        set: |c, v| if let PrefValue::Index(i) = v { c.clock_24h = i == 0 },
        enabled: always, restart: false },
    switch!("clock_seconds", Bar, "Relógio", "Segundos", "", "relógio hora", clock_seconds, always),
    switch!("clock_weekday", Bar, "Relógio", "Dia da semana", "", "relógio data", clock_weekday, always),
    switch!("clock_date", Bar, "Relógio", "Dia e mês", "", "relógio data", clock_date, always),
    switch!("clock_year", Bar, "Relógio", "Ano", "", "relógio data", clock_year, with_date),
    Pref { id: "date_order", tab: SettingsTab::Bar, card: "Relógio", name: "Ordem da data", desc: "", words: "relógio data",
        kind: PrefKind::Choice(&["Dia e mês", "Mês e dia"]),
        get: |c| PrefValue::Index(if c.date_order == DateOrder::MonthDay { 1 } else { 0 }),
        set: |c, v| if let PrefValue::Index(i) = v {
            c.date_order = if i == 1 { DateOrder::MonthDay } else { DateOrder::WeekdayDayMonth }
        },
        enabled: with_date, restart: false },
    switch!("month_number", Bar, "Relógio", "Mês em número", "11/11 em vez de 11 nov", "relógio data", month_number, with_date),
    switch!("countdown_events", Bar, "Próximo", "Conta até os eventos", "Da agenda", "contagem próximo compromisso", countdown_events, always),
    switch!("countdown_timer", Bar, "Próximo", "Conta até o fim do timer", "", "contagem próximo", countdown_timer, always),
    switch!("countdown_pomodoro", Bar, "Próximo", "Conta até a próxima parte do Pomodoro", "", "contagem próximo foco", countdown_pomodoro, always),
    slider!("countdown_hours", Bar, "Próximo", "Eventos a partir de", "Antes disso, a barra mostra a música ou a data", "contagem horas",
        1.0..48.0 step 1.0 "h", countdown_hours / 1 as u32, always),
    // Aparência
    Pref { id: "accent_rgb", tab: SettingsTab::Look, card: "Cores", name: "Cor de destaque", desc: "Seleções, sliders e anéis",
        words: "cor tema damasco destaque", kind: PrefKind::Color(ACCENTS),
        get: |c| { let (r, g, b) = c.accent_rgb; PrefValue::Rgb(r, g, b) },
        set: |c, v| if let PrefValue::Rgb(r, g, b) = v { c.accent_rgb = (r, g, b) },
        enabled: always, restart: false },
    Pref { id: "background_rgb", tab: SettingsTab::Look, card: "Cores", name: "Fundo da ilha", desc: "",
        words: "cor tema fundo escuro", kind: PrefKind::Color(BACKGROUNDS),
        get: |c| { let (r, g, b) = c.background_rgb; PrefValue::Rgb(r, g, b) },
        set: |c, v| if let PrefValue::Rgb(r, g, b) = v { c.background_rgb = (r, g, b) },
        enabled: always, restart: false },
    slider!("gap_px", Look, "Tamanho", "Distância do topo", "", "espaço margem topo", 0.0..40.0 step 1.0 "px", gap_px / 1 as i32, always),
    switch!("ui.tooltips", Look, "Interação", "Dicas sob os ícones", "", "dicas tooltip hover", ui.tooltips, always),
    switch!("ui.hover_highlight", Look, "Interação", "Destacar controles sob o mouse", "", "realce hover", ui.hover_highlight, always),
    switch!("ui.uniform_panels", Look, "Tamanho", "Painéis na largura da ilha", "Painéis podem acompanhar uma pílula mais larga", "painel largura", ui.uniform_panels, always),
    slider!("ui.pill_height_pct", Look, "Tamanho", "Altura da pílula fechada", "Em % do normal", "pílula altura", 70.0..130.0 step 5.0 "%", ui.pill_height_pct / 1 as u32, always),
    slider!("notch_curve", Look, "Tamanho", "Arredondamento", "", "cantos curva", 0.0..100.0 step 1.0 "%", notch_curve / 1 as u32, always),
    // Posição e monitores
    switch!("all_monitors", Monitors, "Monitores", "Em todos os monitores", "Uma ilha em cada tela", "telas monitores", all_monitors, always),
    Pref { id: "monitors", tab: SettingsTab::Monitors, card: "Monitores", name: "Ilhas nestes monitores",
        desc: "Clique numa tela para pôr ou tirar a ilha", words: "telas monitores", kind: PrefKind::Monitors,
        get: |c| PrefValue::List(c.monitors.clone()),
        set: |c, v| if let PrefValue::List(m) = v { if !m.is_empty() { c.monitors = m } },
        enabled: |c| !c.all_monitors, restart: false },
    Pref { id: "main_monitor", tab: SettingsTab::Monitors, card: "Monitores", name: "Monitor principal",
        desc: "Avisos, banners e cards aparecem nele", words: "telas monitores principal", kind: PrefKind::Monitors,
        get: |c| text_opt(&c.main_monitor), set: |c, v| set_opt(&mut c.main_monitor, v), enabled: always, restart: false },
    switch!("over_fullscreen", Monitors, "Tela cheia", "Ficar sobre apps em tela cheia sem borda",
        "Desligado, a ilha some na frente de jogos e vídeos", "jogo vídeo tela cheia", over_fullscreen, always),
    switch!("ui.follow_mouse", Monitors, "Monitores", "Acompanhar o monitor do cursor", "Mostra uma ilha na tela em uso", "mouse monitor", ui.follow_mouse, always),
    // Esconder
    switch!("auto_hide", Hiding, "Esconder", "Esconder na linha até o mouse chegar nela", "", "auto esconder linha minimizar", auto_hide, always),
    Pref { id: "ui.hide_when", tab: SettingsTab::Hiding, card: "Esconder", name: "Quando esconder",
        desc: "", words: "mouse janela maximizada encobrir",
        kind: PrefKind::Choice(&["Mouse longe", "Janela maximizada", "Janela por cima"]),
        get: |c| PrefValue::Index(c.ui.hide_when as usize),
        set: |c, v| if let PrefValue::Index(i) = v {
            if let Some(value) = crate::config::HideWhen::ALL.get(i) { c.ui.hide_when = *value; }
        },
        enabled: |c| c.auto_hide, restart: false },
    switch!("hide_maximized", Hiding, "Sumir", "Sumir com janela maximizada",
        "A ilha some de vez, como em jogos e vídeos em tela cheia, enquanto houver uma janela maximizada no monitor dela, como um navegador com as abas no topo. Os avisos seguem \"Mostrar com a ilha escondida por um app\", em Avisos",
        "esconder sumir maximizada navegador chrome abas tela cheia", hide_maximized, always),
    Pref { id: "hide_apps", tab: SettingsTab::Hiding, card: "Sumir", name: "Sumir só na frente destes apps",
        desc: "Toque num app para marcar. A ilha some quando ele é a janela maximizada da frente, como um navegador com as abas no topo; nos outros ela fica. Lista os apps abertos agora",
        words: "esconder sumir app aplicativo navegador chrome edge abas maximizada", kind: PrefKind::Chips,
        get: |c| PrefValue::List(c.hide_apps.clone()),
        set: |c, v| if let PrefValue::List(apps) = v { c.hide_apps = apps },
        enabled: |c| !c.hide_maximized, restart: false },
    slider!("ui.hide_delay_ms", Hiding, "Recolhimento", "Tempo até recolher", "Depois que o mouse sai", "esconder atraso", 0.2..8.0 step 0.1 "s", ui.hide_delay_ms / 1000 as u32, always),
    slider!("ui.line_expire_secs", Hiding, "Linha minimizada", "Apagar a linha depois de", "Zero mantém a linha visível", "esconder linha", 0.0..600.0 step 5.0 "s", ui.line_expire_secs / 1 as u32, |c| c.auto_hide),
    switch!("ui.line_click", Hiding, "Linha minimizada", "Reabrir a linha com clique", "Passe o mouse para indicar onde clicar", "esconder clicar mouse", ui.line_click, |c| c.auto_hide),
    switch!("ui.line_full_width", Hiding, "Linha minimizada", "Zona da borda na largura da pílula", "", "esconder borda largura", ui.line_full_width, |c| c.auto_hide),
    switch!("ui.hover_open", Hiding, "Recolhimento", "Abrir painel ao passar o mouse", "", "hover abrir painel", ui.hover_open, always),
    // Animações
    Pref { id: "animations", tab: SettingsTab::Animations, card: "Animações", name: "Animações", desc: "",
        words: "animação movimento efeitos mola", kind: PrefKind::Choice(&["Seguir o Windows", "Ligadas", "Desligadas"]),
        get: |c| PrefValue::Index(match c.animations { AnimationLevel::System => 0, AnimationLevel::On => 1, AnimationLevel::Off => 2 }),
        set: |c, v| if let PrefValue::Index(i) = v {
            c.animations = [AnimationLevel::System, AnimationLevel::On, AnimationLevel::Off][i.min(2)]
        },
        enabled: always, restart: false },
    // IA
    module!("modules.ai", Ai, "Mostrar a IA", ai),
    switch!("ai_claude_enabled", Ai, "Agentes", "Claude Code", "Cotas na ilha e na aba Uso", "claude anthropic", ai_claude_enabled, on_ai),
    switch!("ai_codex_enabled", Ai, "Agentes", "Codex", "Cotas na ilha e na aba Uso", "codex openai", ai_codex_enabled, on_ai),
    Pref { id: "opencode_api_key", tab: SettingsTab::Ai, card: "Agentes", name: "Chave do OpenCode Go", desc: "",
        words: "opencode api chave", kind: PrefKind::Text { secret: true, placeholder: "sk-..." },
        get: |c| text_opt(&c.opencode_api_key), set: |c, v| set_opt(&mut c.opencode_api_key, v), enabled: on_ai, restart: false },
    switch!("claude_renew_login", Ai, "Cotas", "Renovar o login do Claude sozinha",
        "Ligado: quando o login do Claude Code vence neste PC, a Cimori o renova no arquivo do Claude Code, e as cotas seguem certas mesmo usando o Claude Code só em outro computador (SSH). Desligado: as cotas param até você abrir o Claude Code aqui",
        "uso cota login renovar ssh token vencido", claude_renew_login, on_ai),
    switch!("quota_alerts", Ai, "Cotas", "Avisar perto do limite", "Em 80% e 95%, e \"de volta!\" na renovação", "uso cota limite", quota_alerts, on_ai),
    info!("info.hooks", Ai, "Hooks", "Estado dos hooks"),
    Pref { id: "hooks_token", tab: SettingsTab::Ai, card: "Hooks", name: "Token compartilhado", desc: "64 caracteres hexadecimais; reinstale os hooks após mudar", words: "hooks token segurança", kind: PrefKind::Text { secret: true, placeholder: "Gerado automaticamente" }, get: |c| text_opt(&c.hooks_token), set: |c, v| if let PrefValue::Text(text) = v { let text = text.trim(); if text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()) { c.hooks_token = Some(text.into()); } }, enabled: on_ai, restart: true },
    action!("action.install_hooks", Ai, "Hooks", "Instalar os hooks", "Claude Code e Codex avisam a ilha", "hooks permissão", InstallHooks),
    action!("action.remove_hooks", Ai, "Hooks", "Remover os hooks", "", "hooks", RemoveHooks),
    // Central de controle
    module!("modules.control", Control, "Mostrar a central de controle", control),
    Pref { id: "control_tiles", tab: SettingsTab::Control, card: "Botões", name: "Botões da central",
        desc: "Arraste para mudar a ordem, para fora para tirar", words: "wifi bluetooth botões", kind: PrefKind::Chips,
        get: |c| PrefValue::List(c.control_tiles.iter().map(|t| tile_key(*t).to_string()).collect()),
        set: |c, v| if let PrefValue::List(keys) = v { c.control_tiles = keys.iter().filter_map(|k| tile_from_key(k)).collect() },
        enabled: on_control, restart: false },
    switch!("control_mixer", Control, "Som", "Volume por aplicativo", "", "mixer volume apps", control_mixer, on_control),
    switch!("control_outputs", Control, "Som", "Saídas de áudio", "Fone, alto-falantes", "áudio saída fone", control_outputs, on_control),
    switch!("wheel_volume", Control, "Som", "A roda do mouse na ilha muda o volume", "", "volume roda scroll", wheel_volume, on_control),
    switch!("ui.volume_percent", Control, "Som", "Porcentagem do volume", "", "porcentagem volume", ui.volume_percent, on_control),
    switch!("ui.mic_button", Control, "Som", "Botão do microfone", "", "microfone mudo", ui.mic_button, on_control),
    // Agenda
    module!("modules.agenda", Agenda, "Mostrar a agenda", agenda),
    Pref { id: "calendar_ics_url", tab: SettingsTab::Agenda, card: "Calendário", name: "Endereço da agenda (.ics)",
        desc: "O endereço secreto do Google Agenda ou do Outlook", words: "google outlook ics calendário",
        kind: PrefKind::Text { secret: true, placeholder: "https://..." },
        get: |c| text_opt(&c.calendar_ics_url), set: |c, v| set_opt(&mut c.calendar_ics_url, v), enabled: on_agenda, restart: false },
    switch!("holidays", Agenda, "Calendário", "Feriados", "Acessa a internet (Nager.Date)", "feriado", holidays, on_agenda),
    Pref { id: "holidays_country", tab: SettingsTab::Agenda, card: "Calendário", name: "País dos feriados",
        desc: "Código de duas letras; vazio usa a região do Windows", words: "feriado país", kind: PrefKind::Text { secret: false, placeholder: "Região do Windows" },
        get: |c| PrefValue::Text(c.holidays_country.clone()),
        set: |c, v| if let PrefValue::Text(t) = v {
            let t = t.trim().to_uppercase();
            if t.is_empty() || (t.len() == 2 && t.chars().all(|ch| ch.is_ascii_alphabetic())) { c.holidays_country = t }
        },
        enabled: |c| c.modules.agenda && c.holidays, restart: false },
    switch!("timezone_system", Agenda, "Calendário", "Fuso horário do Windows", "Desligado: Brasília (UTC-3)", "fuso horário hora", timezone_system, on_agenda),
    switch!("alert_sound", Agenda, "Lembretes", "Som nos avisos", "Lembretes, timers e eventos tocam um aviso", "som alarme lembrete timer", alert_sound, on_agenda),
    switch!("agenda_reminders", Agenda, "Lembretes", "Lembrar dos eventos", "Um aviso na ilha", "lembrete alarme", agenda_reminders, on_agenda),
    slider!("agenda_reminder_minutes", Agenda, "Lembretes", "Antecedência", "", "lembrete minutos",
        0.0..120.0 step 5.0 "min", agenda_reminder_minutes / 1 as u32, |c| c.modules.agenda && c.agenda_reminders),
    switch!("ui.agenda_featured_day", Agenda, "Calendário", "Dia em destaque", "", "calendário dia grande agenda", ui.agenda_featured_day, on_agenda),
    switch!("ui.agenda_events", Agenda, "Calendário", "Eventos no calendário", "", "calendário lista compromissos agenda", ui.agenda_events, on_agenda),
    // Clima
    module!("modules.weather", Weather, "Mostrar o clima", weather),
    Pref { id: "weather", tab: SettingsTab::Weather, card: "Clima", name: "Cidade", desc: "Busque pelo nome",
        words: "cidade local previsão", kind: PrefKind::CitySearch,
        get: |c| PrefValue::List(vec![c.weather.label.clone(), c.weather.latitude.to_string(), c.weather.longitude.to_string()]),
        set: |c, v| if let PrefValue::List(p) = v {
            if let [label, lat, lon] = p.as_slice() {
                if let (Ok(latitude), Ok(longitude)) = (lat.parse(), lon.parse()) {
                    c.weather = WeatherConfig { label: label.clone(), latitude, longitude };
                }
            }
        },
        enabled: on_weather, restart: false },
    Pref { id: "weather_fahrenheit", tab: SettingsTab::Weather, card: "Clima", name: "Unidade de temperatura", desc: "",
        words: "celsius fahrenheit temperatura", kind: PrefKind::Choice(&["°C", "°F"]),
        get: |c| PrefValue::Index(c.weather_fahrenheit as usize),
        set: |c, v| if let PrefValue::Index(i) = v { c.weather_fahrenheit = i == 1 },
        enabled: on_weather, restart: false },
    switch!("ui.weather_cards", Weather, "Clima", "Previsão em cartões", "", "clima previsão layout", ui.weather_cards, on_weather),
    // Mídia
    module!("modules.media", Media, "Mostrar a mídia", media),
    switch!("lyrics", Media, "Letras", "Letras das músicas", "Acessa a internet (lrclib.net)", "letra lrclib", lyrics, on_media),
    switch!("ui.media_controls", Media, "Na pílula", "Controles de reprodução", "", "botões anterior próxima música", ui.media_controls, on_media),
    switch!("ui.media_always", Media, "Na pílula", "Sempre mostrar a música", "Inclui a música enquanto houver sessão de mídia", "título música", ui.media_always, on_media),
    switch!("ui.media_play_only", Media, "Na pílula", "Só o botão de tocar e pausar", "", "música pausa", ui.media_play_only, |c| c.modules.media && c.ui.media_controls),
    switch!("ui.media_lyric_title", Media, "Na pílula", "Letra no lugar do título", "", "letra título", ui.media_lyric_title, |c| c.modules.media && c.lyrics),
    slider!("ui.media_title_width", Media, "Na pílula", "Largura máxima do título", "", "música largura", 70.0..280.0 step 5.0 "px", ui.media_title_width / 1 as u32, on_media),
    slider!("ui.lyric_size", Media, "Letras", "Tamanho da letra", "", "música fonte", 12.0..32.0 step 1.0 "px", ui.lyric_size / 1 as u32, |c| c.modules.media && c.lyrics),
    switch!("ui.media_marquee", Media, "Na pílula", "Rolar títulos longos", "", "música marquee título", ui.media_marquee, on_media),
    switch!("ui.media_wheel", Media, "Na pílula", "Trocar de faixa com a roda", "Sobre a música na pílula", "música scroll roda", ui.media_wheel, on_media),
    Pref { id: "ui.lyric_rgb", tab: SettingsTab::Media, card: "Letras", name: "Cor da letra",
        desc: "", words: "música letra cor", kind: PrefKind::Color(&[(245,241,235), (241,139,99), (122,162,247), (158,206,106)]),
        get: |c| { let (r, g, b) = c.ui.lyric_rgb; PrefValue::Rgb(r, g, b) },
        set: |c, v| if let PrefValue::Rgb(r, g, b) = v { c.ui.lyric_rgb = (r, g, b); },
        enabled: |c| c.modules.media && c.lyrics, restart: false },
    // Avisos
    module!("modules.notifications", Notifications, "Mostrar os avisos", notifications),
    switch!("banners", Notifications, "Banners", "Avisos na pílula", "Um banner quando chega um aviso", "banner notificação", banners, on_notifications),
    switch!("ui.banner_over_apps", Notifications, "Banners", "Mostrar com a ilha escondida por um app",
        "Quando a ilha some na frente de uma janela maximizada ou de um app marcado em Esconder, o banner e os pedidos de permissão aparecem mesmo assim. Jogos e vídeos em tela cheia continuam sem",
        "banner aviso escondida oculta sumir chrome maximizada", ui.banner_over_apps, on_notifications),
    slider!("banner_secs", Notifications, "Banners", "Tempo na tela", "", "banner segundos",
        2.0..30.0 step 1.0 "s", banner_secs / 1 as u32, |c| c.modules.notifications && c.banners),
    Pref { restart: true, ..switch!("notification_icons", Notifications, "Lista", "Ícones dos apps", "", "ícone app", notification_icons, on_notifications) },
    slider!("notifications_per_page", Notifications, "Lista", "Avisos por página", "", "página lista",
        2.0..4.0 step 1.0 "", notifications_per_page / 1 as u32, on_notifications),
    Pref { id: "muted_apps", tab: SettingsTab::Notifications, card: "Apps silenciados", name: "Não vão para a ilha",
        desc: "Toque num app para silenciar ou voltar", words: "silenciar ignorar app", kind: PrefKind::Chips,
        get: |c| PrefValue::List(c.muted_apps.clone()),
        set: |c, v| if let PrefValue::List(apps) = v { c.muted_apps = apps },
        enabled: on_notifications, restart: false },
    action!("action.windows_banners", Notifications, "Banners do Windows", "Esconder os banners do Windows",
        "Desligue \"Mostrar faixas de notificação\" em cada app; o aviso continua chegando à ilha", "faixa banner windows", OpenWindowsBanners),
    Pref { id: "ui.banner_size", tab: SettingsTab::Notifications, card: "Banners", name: "Tamanho", desc: "",
        words: "banner tamanho altura", kind: PrefKind::Choice(&["50%", "75%", "100%", "125%", "150%", "200%"]),
        get: |c| PrefValue::Index(crate::config::BANNER_SIZES.iter().position(|s| *s == c.ui.banner_size).unwrap_or(2)),
        set: |c, v| if let PrefValue::Index(i) = v { if let Some(s) = crate::config::BANNER_SIZES.get(i) { c.ui.banner_size = *s } },
        enabled: on_notifications, restart: false },
    Pref { id: "ui.banner_width_pct", tab: SettingsTab::Notifications, card: "Banners", name: "Largura do banner", desc: "",
        words: "banner largura", kind: PrefKind::Choice(&["80%", "100%", "130%", "160%"]),
        get: |c| PrefValue::Index(crate::config::BANNER_WIDTHS.iter().position(|s| *s == c.ui.banner_width_pct).unwrap_or(1)),
        set: |c, v| if let PrefValue::Index(i) = v { if let Some(s) = crate::config::BANNER_WIDTHS.get(i) { c.ui.banner_width_pct = *s } },
        enabled: on_notifications, restart: false },
    switch!("ui.banner_when_hidden", Notifications, "Banners", "Avisar com a pílula minimizada", "", "avisos minimizada", ui.banner_when_hidden, on_notifications),
    // Foco
    module!("modules.timer", Focus, "Mostrar Foco e timers", timer),
    slider!("pomodoro_focus_secs", Focus, "Pomodoro", "Foco", "", "pomodoro foco", 5.0..180.0 step 5.0 "min", pomodoro_focus_secs / 60 as u32, on_timer),
    slider!("pomodoro_break_secs", Focus, "Pomodoro", "Pausa", "", "pomodoro pausa", 1.0..60.0 step 1.0 "min", pomodoro_break_secs / 60 as u32, on_timer),
    slider!("timer_default_secs", Focus, "Timer", "Tempo padrão", "", "timer contagem", 1.0..120.0 step 1.0 "min", timer_default_secs / 60 as u32, on_timer),
    switch!("ui.timer_on_line", Focus, "Timer", "Timer na linha minimizada", "", "timer linha minimizar", ui.timer_on_line, on_timer),
    // Bandeja
    module!("modules.tray", Tray, "Mostrar os ícones dos outros apps", tray),
    Pref { id: "pinned_tray_apps", tab: SettingsTab::Tray, card: "Fixados", name: "Na pílula",
        desc: "Arraste para fixar, desfixar e ordenar", words: "fixar bandeja ícones", kind: PrefKind::Chips,
        get: |c| PrefValue::List(c.pinned_tray_apps.iter().map(|a| a.exe_path.clone()).collect()),
        set: |c, v| if let PrefValue::List(paths) = v {
            let old = std::mem::take(&mut c.pinned_tray_apps);
            c.pinned_tray_apps = paths.into_iter().map(|p| old.iter().find(|a| a.exe_path == p).cloned()
                .unwrap_or_else(|| PinnedTrayApp { display_name: crate::config::derive_display_name(&p), exe_path: p, guid: None }))
                .collect();
        },
        enabled: on_tray, restart: false },
    info!("info.bridge", Tray, "Ponte do Windhawk", "Estado da ponte"),
    action!("action.bridge_readme", Tray, "Ponte do Windhawk", "Como instalar a ponte", "Clica nos ícones como a barra do Windows", "windhawk ponte", OpenBridgeReadme),
    switch!("ui.tray_grid", Tray, "Lista", "Mostrar os ícones em grade", "", "bandeja grade", ui.tray_grid, on_tray),
    switch!("ui.tray_keep_open", Tray, "Lista", "Manter o painel aberto ao tirar o mouse", "", "bandeja manter aberto", ui.tray_keep_open, on_tray),
    module!("modules.clipboard", Clipboard, "Mostrar o histórico da área de transferência", clipboard),
    module!("modules.shelf", Shelf, "Mostrar a prateleira de arquivos", shelf),
    module!("modules.usb", Usb, "Mostrar dispositivos USB", usb),
    module!("modules.downloads", Downloads, "Mostrar downloads", downloads),
    slider!("clipboard_limit", Clipboard, "Histórico", "Limite de itens recentes", "Exclui os fixados do limite", "histórico quantidade", 1.0..20.0 step 1.0 "", clipboard_limit / 1 as u32, |c| c.modules.clipboard),
    switch!("clipboard_images", Clipboard, "Histórico", "Guardar imagens", "", "clipboard imagem", clipboard_images, |c| c.modules.clipboard),
    switch!("clipboard_files", Clipboard, "Histórico", "Guardar arquivos copiados", "", "clipboard arquivos", clipboard_files, |c| c.modules.clipboard),
    switch!("clipboard_notice", Clipboard, "Aviso", "Mostrar o que foi copiado", "Um aviso na pílula a cada cópia, mesmo sem o histórico. Nada é guardado", "copiado aviso clipboard", clipboard_notice, |_| true),
    Pref { restart: true, ..switch!("clipboard_persist", Clipboard, "Histórico", "Salvar histórico em disco", "Inclui texto e imagens; retoma após reiniciar", "clipboard salvar persistir", clipboard_persist, |c| c.modules.clipboard) },
    switch!("clipboard_paste", Clipboard, "Histórico", "Colar ao escolher uma cópia", "Ctrl+V; Ctrl+Shift+V em terminais", "clipboard colar", clipboard_paste, |c| c.modules.clipboard),
    switch!("clipboard_thumbnails", Clipboard, "Histórico", "Mostrar miniaturas das imagens", "", "clipboard miniatura imagem", clipboard_thumbnails, |c| c.modules.clipboard),
    Pref { id: "clipboard_ignored_apps", tab: SettingsTab::Clipboard, card: "Privacidade", name: "Apps ignorados", desc: "Executáveis separados por vírgula", words: "clipboard ignorar senha app", kind: PrefKind::Text { secret: false, placeholder: "KeePassXC.exe, 1Password.exe" }, get: |c| PrefValue::Text(c.clipboard_ignored_apps.join(", ")), set: |c, v| if let PrefValue::Text(text) = v { c.clipboard_ignored_apps = text.split([',', ';']).map(str::trim).filter(|s| !s.is_empty() && !s.contains(['/', '\\'])).map(str::to_owned).collect() }, enabled: always, restart: false },
    Pref { restart: true, ..switch!("shelf_persist", Shelf, "Prateleira", "Salvar prateleira em disco", "Retoma após reiniciar; os arquivos ficam em seus locais", "shelf salvar persistir", shelf_persist, |c| c.modules.shelf) },
    switch!("shelf_drag_move", Shelf, "Prateleira", "Permitir mover ao arrastar para fora", "Desligado, o arraste apenas copia", "shelf arrastar mover", shelf_drag_move, |c| c.modules.shelf),
    switch!("shelf_drop_pill", Shelf, "Prateleira", "Soltar na pílula", "Arraste arquivos até a ilha fechada para guardar", "shelf soltar arrastar pílula", shelf_drop_pill, |c| c.modules.shelf),
    slider!("shelf_capacity", Shelf, "Prateleira", "Máximo de arquivos", "", "shelf arquivos limite", 1.0..6.0 step 1.0 "", shelf_capacity / 1 as u32, |c| c.modules.shelf),
    switch!("shelf_qr", Shelf, "Compartilhamento", "Compartilhar pelo celular", "O servidor local só é iniciado ao abrir o QR", "qr celular rede", shelf_qr, |c| c.modules.shelf),
    switch!("shelf_receive", Shelf, "Compartilhamento", "Receber arquivos do celular", "Após conferir o código; salva na pasta própria da Cimori", "qr receber upload", shelf_receive, |c| c.modules.shelf && c.shelf_qr),
    slider!("shelf_idle_minutes", Shelf, "Compartilhamento", "Encerrar após inatividade", "", "qr tempo minutos", 1.0..30.0 step 1.0 "min", shelf_idle_minutes / 1 as u32, |c| c.modules.shelf && c.shelf_qr),
    switch!("usb_alerts", Usb, "Unidades", "Avisar quando conectar", "", "usb notificar banner", usb_alerts, |c| c.modules.usb),
    switch!("download_meter", Downloads, "Progresso", "Preencher a pílula durante downloads", "", "download progresso barra", download_meter, |c| c.modules.downloads),
    switch!("download_taskbar", Downloads, "Progresso", "Ler o progresso da barra de tarefas", "Requer a ponte Windhawk", "download ponte barra", download_taskbar, |c| c.modules.downloads),
    switch!("download_animation", Downloads, "Progresso", "Animar início e fim", "Respeita a opção de animações", "download animação", download_animation, |c| c.modules.downloads),
    switch!("download_details", Downloads, "Lista", "Detalhar cada download", "Nome, bytes e velocidade dos arquivos parciais", "download lista velocidade", download_details, |c| c.modules.downloads),
    switch!("download_recent", Downloads, "Lista", "Downloads recentes", "Os últimos arquivos da pasta Downloads", "download recentes arquivos pasta", download_recent, |c| c.modules.downloads),
    // Atalhos
    hotkey!("ui.minimize_hotkey", "Minimizar e restaurar", minimize_keys, notices_keys, always),
    hotkey!("ui.notices_hotkey", "Abrir avisos", notices_keys, minimize_keys, |c| c.win_n && c.modules.notifications),
    switch!("win_n", Shortcuts, "Teclas", "Atalho dos avisos", "", "atalho teclado notificações", win_n, on_notifications),
    // Backup
    action!("action.export", Backup, "Backup", "Exportar", "Configurações e lembretes num arquivo", "salvar backup", Export),
    action!("action.import", Backup, "Backup", "Importar", "De um arquivo exportado", "restaurar backup", Import),
    action!("action.restore", Backup, "Padrão", "Restaurar o padrão", "Volta todas as opções ao início", "resetar padrão", Restore),
    // Sobre
    Pref { desc: "Ilha dinâmica para o Windows 11", words: "versão sobre", ..info!("info.version", About, "Cimori", "Cimori") },
    link!("link.repo", About, "Cimori", "Código-fonte", "github.com/Vini-Paixao/Cimori", REPO),
    link!("link.issues", About, "Cimori", "Reportar um problema", "Bugs e ideias, pelas issues do GitHub", ISSUES),
    Pref { desc: "Software livre: use, estude, mude e compartilhe", words: "licença gpl", ..link!("link.license", About, "Cimori", "Licença GPL-3.0", "", LICENSE) },
    action!("action.show_welcome", About, "Cimori", "Mostrar boas-vindas de novo", "Aparência, hooks dos agentes e a ponte do Windhawk", "boas-vindas primeiros passos tutorial", ShowWelcome),
    // Atualizações
    Pref { words: "atualização versão", ..info!("info.update", Updates, "Versão", "Estado") },
    action!("action.check_updates", Updates, "Versão", "Buscar atualizações", "Consulta o GitHub agora", "atualização versão buscar verificar", CheckUpdates),
    action!("action.install_update", Updates, "Versão", "Baixar e instalar",
        "Baixa o instalador da versão nova no GitHub, confere o SHA-256 e instala. A Cimori fecha e abre de novo sozinha, com as suas configurações",
        "atualização baixar instalar atualizar", InstallUpdate),
    action!("action.download_update", Updates, "Versão", "Novidades da versão", "Abre a página da versão no GitHub", "atualização novidades changelog", DownloadUpdate),
    switch!("check_updates", Updates, "Automático", "Buscar atualizações sozinha",
        "Ao abrir e uma vez por dia. Com uma versão nova, um aviso na ilha leva a esta aba; nada é instalado sem você pedir",
        "atualização versão nova github automático aviso", check_updates, always),
    link!("link.isle", About, "Créditos", "isle", "zickrian · GPL-3.0 · base da ilha e dos agentes", "https://github.com/zickrian/isle"),
    link!("link.mac_island", About, "Créditos", "Mac Island & Dock", "caliberda · GPL-3.0 · opções e módulos", "https://island.caliberda.com.br"),
    Pref { desc: "Fluent, Ionicons, Tabler, Phosphor, Bootstrap, Iconoir, Heroicons", words: "ícones licença", ..info!("info.credits_mit", About, "Créditos", "Pacotes de ícones (MIT)") },
    Pref { desc: "Lucide: ISC · Material Symbols: Apache 2.0", words: "ícones licença", ..info!("info.credits_icons", About, "Créditos", "Mais ícones") },
    Pref { desc: "tokio, reqwest, serde, tiny-skia, resvg, windows-rs e outras", words: "bibliotecas crates licença", ..info!("info.credits_crates", About, "Créditos", "Bibliotecas Rust (MIT, Apache 2.0 e outras)") },
    // Estilo: a pílula ou o entalhe (Mac Island & Dock's islandStyle/islandPillNotches; isle's notch).
    Pref { id: "ui.pill_style", tab: SettingsTab::Look, card: "Estilo", name: "Formato da ilha",
        desc: "O entalhe fica preso à borda de cima e abre ao passar o mouse", words: "estilo entalhe notch pílula ilha formato",
        kind: PrefKind::Choice(&["Pílula", "Entalhe"]),
        get: |c| PrefValue::Index((c.ui.pill_style == crate::config::PillStyle::Notch) as usize),
        set: |c, v| if let PrefValue::Index(i) = v {
            c.ui.pill_style = if i == 1 { crate::config::PillStyle::Notch } else { crate::config::PillStyle::Island }
        },
        enabled: always, restart: false },
    slider!("ui.pill_width", Look, "Tamanho", "Largura mínima da ilha fechada", "Ela cresce com os itens da barra", "pílula entalhe largura",
        160.0..560.0 step 10.0 "px", ui.pill_width / 1 as u32, always),
    Pref { id: "notch_items", tab: SettingsTab::Bar, card: "Itens", name: "No entalhe",
        desc: "O que o entalhe fechado mostra; arraste para mudar a ordem", words: "entalhe notch itens relógio",
        kind: PrefKind::Chips,
        get: |c| PrefValue::List(c.notch_items.iter().map(|i| bar_key(*i).to_string()).collect()),
        set: |c, v| if let PrefValue::List(keys) = v {
            let items: Vec<_> = keys.iter().filter_map(|k| bar_from_key(k)).collect();
            if !items.is_empty() { c.notch_items = items }
        },
        enabled: |c| c.ui.pill_style == crate::config::PillStyle::Notch, restart: false },
    // Botão de desligar (Mac Island & Dock's islandPower*; PowerActionOn, PowerAsks).
    switch!("power.sleep", Control, "Botão de desligar", "Suspender", "", "energia desligar dormir suspender", power.sleep, always),
    switch!("power.hibernate", Control, "Botão de desligar", "Hibernar", "Só quando o Windows permite", "energia desligar hibernar", power.hibernate, always),
    switch!("power.restart", Control, "Botão de desligar", "Reiniciar", "", "energia desligar reiniciar", power.restart, always),
    switch!("power.shut_down", Control, "Botão de desligar", "Desligar", "", "energia desligar", power.shut_down, always),
    switch!("power.lock", Control, "Botão de desligar", "Bloquear", "", "energia desligar bloquear", power.lock, always),
    switch!("power.sign_out", Control, "Botão de desligar", "Sair", "Encerra a sessão", "energia desligar sair sessão", power.sign_out, always),
    switch!("power.confirm", Control, "Botão de desligar", "Perguntar antes", "Reiniciar, desligar e sair pedem confirmação", "energia desligar confirmar", power.confirm, always),
    switch!("ui.arrow_pins", Hiding, "Interação", "Seta fixa a ilha aberta", "Com auto-ocultar ligado. Clique de novo para soltar; o atalho minimiza.", "fixar prateleira arrastar arquivos pin", ui.arrow_pins, |c| c.auto_hide),
    // Boas-vindas (WELCOME): rows of their own; the others come from their tabs.
    Pref { desc: "Passe o mouse ou clique nela para abrir; clique com o direito para as configurações", ..info!("welcome.intro", Welcome, "Boas-vindas", "A Cimori é a ilha no topo da tela") },
    Pref { desc: "Aqui nas configurações, pelas abas ao lado ou pela busca", ..info!("welcome.later", Welcome, "Boas-vindas", "Tudo pode ser mudado depois") },
    Pref { desc: "Com os hooks, Claude Code e Codex avisam a ilha ao terminar e pedem permissão nela", ..info!("welcome.agents", Welcome, "Agentes", "Agentes de IA na ilha") },
    info!("welcome.claude", Welcome, "Agentes", "Claude Code"),
    info!("welcome.codex", Welcome, "Agentes", "Codex"),
    Pref { desc: "Mostra e clica todos os ícones da bandeja e lê o progresso dos downloads na barra", ..info!("welcome.bridge", Welcome, "Ponte", "Ponte do Windhawk (opcional)") },
    link!("welcome.bridge1", Welcome, "Ponte", "1. Instale o Windhawk", "windhawk.net", "https://windhawk.net"),
    info!("welcome.bridge2", Welcome, "Ponte", "2. No Windhawk, clique em Criar um novo mod"),
    action!("welcome.bridge3", Welcome, "Ponte", "3. Apague o exemplo e cole o código", "", "windhawk ponte copiar", CopyBridge),
    Pref { desc: "A ponte fica ativa e o estado acima passa a Ligada", ..info!("welcome.bridge4", Welcome, "Ponte", "4. Compilar o mod, depois Sair do modo de edição") },
    Pref { desc: "Para rever estes passos: Sobre → Mostrar boas-vindas de novo", ..info!("welcome.done", Welcome, "Fim", "A Cimori está pronta") },
    action!("welcome.back", Welcome, "Passos", "Passo anterior", "", "", WelcomeBack),
    action!("welcome.next", Welcome, "Passos", "Próximo passo", "", "", WelcomeNext),
    action!("welcome.finish", Welcome, "Passos", "Concluir", "Vai para as configurações", "", WelcomeFinish),
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
            let hay = fold(&format!("{} {} {} {} {}", p.name, p.desc, p.words, p.card, p.tab.title()));
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .collect()
}
