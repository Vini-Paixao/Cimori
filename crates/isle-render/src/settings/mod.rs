//! The settings window (docs/superpowers/specs/2026-11-11-configuracoes-design.md): its state, and
//! the geometry that both drawing and clicks use.
mod layout;

pub use layout::{CardBox, Hit, Layout, NavEntry, RowBox, action_label, chip_items, control_rect, desc_lines, hit, layout, monitor_rects, name_room, slider_value};
pub use layout::DESC;
pub use layout::{HEADER, NAV_NARROW, NAV_W};
pub use layout::{listed, strip_pref};

use crate::window::MonitorPlace;
use isle_core::prefs::{ActionId, SettingsTab};

/// What the window shows and what is being done in it.
#[derive(Debug, Clone)]
pub struct SettingsView {
    pub tab: SettingsTab,
    pub search: String,
    pub scroll: f32,
    pub nav_scroll: f32,
    pub focus: Option<Focus>,
    pub hover: Option<(f32, f32)>,
    /// The text row being typed in, and its text so far.
    pub editing: Option<(&'static str, String)>,
    pub drag: Option<ChipDrag>,
    /// An action that asks again before it runs ("Confirmar?").
    pub confirm: Option<ActionId>,
    pub monitors: Vec<MonitorPlace>,
    pub city_results: Vec<(String, f64, f64)>,
    /// Chips whose items come from what the island saw: (row id, [(key, label)]).
    pub dynamic: Vec<(&'static str, Vec<(String, String)>)>,
    /// Read-only texts of the Info rows: (row id, text).
    pub info: Vec<(&'static str, String)>,
    /// A short message under the page title (an error, "Exportado").
    pub status: Option<String>,
    /// The shortcut row waiting for keys ("Clique e pressione as teclas").
    pub recording: Option<&'static str>,
    /// Keys just recorded: the character they typed (Ctrl+H, Esc) is not also typed into the window.
    pub just_recorded: bool,
    /// The blinking caret of the field being typed in: whether this frame draws it.
    pub caret: bool,
    /// A newer Cimori found on GitHub (its version): Sobre shows "Baixar".
    pub update: Option<String>,
    /// The welcome's step (`prefs::WELCOME`).
    pub welcome_step: usize,
}

impl Default for SettingsView {
    fn default() -> Self {
        Self {
            tab: SettingsTab::General,
            search: String::new(),
            scroll: 0.0,
            nav_scroll: 0.0,
            focus: None,
            hover: None,
            editing: None,
            drag: None,
            confirm: None,
            monitors: Vec::new(),
            city_results: Vec::new(),
            dynamic: Vec::new(),
            info: Vec::new(),
            status: None,
            recording: None,
            just_recorded: false,
            caret: true,
            update: None,
            welcome_step: 0,
        }
    }
}

impl SettingsView {
    pub fn dynamic(&self, id: &str) -> &[(String, String)] {
        self.dynamic.iter().find(|(d, _)| *d == id).map(|(_, v)| v.as_slice()).unwrap_or(&[])
    }
    pub fn info(&self, id: &str) -> &str {
        self.info.iter().find(|(d, _)| *d == id).map(|(_, v)| v.as_str()).unwrap_or("")
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    Nav(usize),
    Search,
    Row(&'static str),
}

/// A chip being dragged: its row, where it was, and where the pointer is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChipDrag {
    pub pref: &'static str,
    pub from: usize,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsAction {
    Tab(SettingsTab),
    FocusSearch,
    ClearSearch,
    Toggle(&'static str),
    Choose(&'static str, usize),
    /// The value under the pointer, already stepped and clamped.
    Slide(&'static str, f32),
    Pick(&'static str, (u8, u8, u8)),
    Edit(&'static str),
    /// Puts an item in, or takes it out (its key).
    ChipToggle(&'static str, String),
    /// Starts dragging the n-th item in.
    ChipGrab(&'static str, usize),
    Run(ActionId),
    PickCity(usize),
    Monitor(String),
    MainMonitor(String),
    /// A shortcut row: wait for its keys (again: stop waiting), or set it to none.
    Record(&'static str),
    ClearShortcut(&'static str),
}
