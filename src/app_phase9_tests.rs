//! Phase 9 (Claude track) in the app: the notch, the power button, the editable control center.
use super::*;
use isle_core::config::PillStyle;
use std::time::{Duration, Instant};

fn notch_app() -> IsleApp {
    let mut cfg = IsleConfig::default();
    cfg.ui.pill_style = PillStyle::Notch;
    IsleApp::new(cfg)
}

#[test]
fn the_notch_opens_as_the_mouse_reaches_it_and_closes_once_it_leaves() {
    // With "Abrir painel ao passar o mouse" on; off, it waits for a click like the pill.
    let mut app = notch_app();
    app.config.ui.hover_open = true;
    app.on_hover(true);
    app.on_deadlines(Instant::now() + Duration::from_secs(1));
    assert!(matches!(app.islands[0].mode, PillMode::Expanded(_)), "{:?}", app.islands[0].mode);
    app.on_hover(false);
    let t = Instant::now() + Duration::from_secs(2);
    app.on_deadlines(t);
    app.on_deadlines(t + Duration::from_secs(5));
    assert_eq!(app.islands[0].mode, PillMode::CompactBar);
}

#[test]
fn the_pill_still_waits_for_a_click() {
    let mut app = IsleApp::new(IsleConfig::default());
    app.on_hover(true);
    app.on_deadlines(Instant::now() + Duration::from_secs(1));
    assert_eq!(app.islands[0].mode, PillMode::CompactBar);
}

#[test]
fn the_closed_notch_has_its_own_items_and_corners() {
    let mut app = notch_app();
    assert_eq!(app.state.bar, vec![isle_core::config::BarItem::Clock]);
    app.sync_tray_panel();
    let r = app.islands[0].geom.corner_radius.target;
    assert_eq!(r, isle_render::renderer::notch_radius(app.config.ui.pill_height_px(), app.config.notch_curve));
    // Back to the pill: the bar's items and a round pill again.
    let old = app.config.clone();
    app.config.ui.pill_style = PillStyle::Island;
    app.apply_config(&old, false);
    assert_eq!(app.state.bar, app.config.bar_items);
    app.sync_tray_panel();
    assert_eq!(app.islands[0].geom.corner_radius.target, app.config.ui.pill_height_px() * 0.5);
}

use isle_core::power::PowerAction;

fn ran() -> Vec<PowerAction> {
    POWER_RAN.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

#[test]
fn the_power_button_opens_its_menu_and_asks_before_shutting_down() {
    ran();
    let mut app = IsleApp::new(IsleConfig::default());
    app.apply_ui_action(UiAction::OpenPower);
    assert_eq!(app.islands[0].mode, PillMode::Expanded(TabPage::Power));
    app.apply_ui_action(UiAction::Power(PowerAction::ShutDown));
    assert_eq!(app.state.power_confirm, Some(PowerAction::ShutDown));
    assert!(ran().is_empty(), "nothing runs before the confirmation");
    // Cancelar closes the question; the menu stays.
    app.apply_ui_action(UiAction::PowerCancel);
    assert_eq!(app.state.power_confirm, None);
    app.apply_ui_action(UiAction::Power(PowerAction::Restart));
    app.apply_ui_action(UiAction::PowerConfirm);
    assert_eq!(ran(), [PowerAction::Restart]);
    assert_eq!(app.islands[0].mode, PillMode::CompactBar, "the menu closes first");
}

#[test]
fn lock_and_sleep_run_at_once_and_confirmation_can_be_turned_off() {
    ran();
    let mut cfg = IsleConfig::default();
    cfg.power.confirm = false;
    let mut app = IsleApp::new(cfg);
    app.apply_ui_action(UiAction::OpenPower);
    app.apply_ui_action(UiAction::Power(PowerAction::Lock));
    app.apply_ui_action(UiAction::OpenPower);
    app.apply_ui_action(UiAction::Power(PowerAction::ShutDown));
    assert_eq!(ran(), [PowerAction::Lock, PowerAction::ShutDown]);
}

#[test]
fn a_single_action_on_is_the_buttons_own_unless_it_asks() {
    ran();
    let mut cfg = IsleConfig::default();
    cfg.power = isle_core::power::PowerOptions { sleep: false, hibernate: false, restart: false, shut_down: false, lock: true, sign_out: false, confirm: true };
    let mut app = IsleApp::new(cfg.clone());
    app.apply_ui_action(UiAction::Control(isle_core::state::ControlTile::Power));
    assert_eq!(ran(), [PowerAction::Lock]);
    assert_eq!(app.islands[0].mode, PillMode::CompactBar);
    cfg.power.lock = false;
    cfg.power.shut_down = true;
    let mut app = IsleApp::new(cfg);
    app.apply_ui_action(UiAction::OpenPower);
    assert_eq!(app.islands[0].mode, PillMode::Expanded(TabPage::Power));
    assert!(ran().is_empty());
}

use isle_core::state::ControlTile;

/// The main island open on the control center, its spring settled.
fn center(app: &mut IsleApp) -> (f32, f32) {
    app.apply_ui_action(UiAction::SwitchTab(TabPage::ControlCenter));
    let (w, h, _) = isle_render::layout::expanded_size(TabPage::ControlCenter);
    let g = &mut app.islands[0].geom;
    (g.width.value, g.height.value) = (w, h);
    (w, h)
}

fn hit(app: &IsleApp, w: f32, h: f32, action: UiAction) -> isle_render::pages::HitBox {
    let mut s = app.state.clone();
    s.mode = app.islands[0].mode.clone();
    compute_hitboxes(CANVAS_MAX_WIDTH as f32, w, h, app.config.top_gap(), &s).into_iter().find(|b| b.action == action)
        .unwrap_or_else(|| panic!("{action:?}"))
}

#[test]
fn editing_the_center_removes_adds_and_keeps_it_in_the_settings() {
    let dir = std::env::temp_dir().join(format!("cimori-edit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    let mut app = IsleApp::new(IsleConfig::default());
    app.settings_path = Some(path.clone());
    let (w, h) = center(&mut app);
    app.apply_ui_action(UiAction::EditControls);
    assert!(app.state.control_editing);
    let minus = hit(&app, w, h, UiAction::ControlRemove(ControlTile::Cast));
    app.apply_input(UiInput::Click { window: 0, x: minus.x + 5., y: minus.y + 5. }).unwrap();
    assert!(!app.config.control_tiles.contains(&ControlTile::Cast));
    assert_eq!(app.state.control_tiles, app.config.control_tiles);
    let add = hit(&app, w, h, UiAction::ControlAdd(ControlTile::WindowsSettings));
    app.apply_input(UiInput::Click { window: 0, x: add.x + 5., y: add.y + 5. }).unwrap();
    assert_eq!(app.config.control_tiles.last(), Some(&ControlTile::WindowsSettings));
    let saved = IsleConfig::load_lenient(&std::fs::read_to_string(&path).unwrap());
    assert_eq!(saved.control_tiles, app.config.control_tiles);
    // Closing the panel ends the edit mode.
    app.apply_ui_action(UiAction::ToggleCollapse);
    app.sync_control_panel();
    assert!(!app.state.control_editing);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_control_dragged_onto_another_takes_its_place() {
    let mut app = IsleApp::new(IsleConfig::default());
    let (w, h) = center(&mut app);
    app.apply_ui_action(UiAction::EditControls);
    let first = hit(&app, w, h, UiAction::ControlGrab(0));
    let fourth = hit(&app, w, h, UiAction::ControlGrab(3));
    let (fx, fy) = (first.x + first.w - 10., first.y + first.h / 2.);
    app.apply_input(UiInput::PointerDown { window: 0, x: fx, y: fy }).unwrap();
    assert_eq!(app.state.control_drag, Some((0, 0)));
    let (tx, ty) = (fourth.x + fourth.w / 2., fourth.y + fourth.h / 2.);
    app.apply_input(UiInput::Drag { window: 0, x: tx, y: ty }).unwrap();
    assert_eq!(app.state.control_drag, Some((0, 3)));
    app.apply_input(UiInput::Click { window: 0, x: tx, y: ty }).unwrap();
    let mut expected = ControlTile::ALL.to_vec();
    let wifi = expected.remove(0);
    expected.insert(3, wifi);
    assert_eq!(app.config.control_tiles, expected);
    assert_eq!(app.state.control_drag, None);
}

#[test]
fn island_controls_open_their_tab_and_shortcuts_close_the_island() {
    let mut app = IsleApp::new(IsleConfig::default());
    center(&mut app);
    app.apply_ui_action(UiAction::Control(ControlTile::Alarms));
    assert_eq!(app.islands[0].mode, PillMode::Expanded(TabPage::Timer));
    assert_eq!(app.state.timers.mode, isle_core::timers::TimerMode::Reminders);
    center(&mut app);
    app.apply_ui_action(UiAction::Control(ControlTile::Screenshot));
    assert_eq!(app.islands[0].mode, PillMode::CompactBar, "the picture is taken without the island");
    center(&mut app);
    app.apply_ui_action(UiAction::Control(ControlTile::DarkMode));
    assert_eq!(app.islands[0].mode, PillMode::Expanded(TabPage::ControlCenter));
    app.config.modules.weather = false;
    app.apply_ui_action(UiAction::Control(ControlTile::Weather));
    assert!(app.state.control_error.is_some());
}

use isle_core::shortcut::{ALT, CTRL, SHIFT, WIN, code};
use isle_render::settings::SettingsAction;
use isle_render::settings_window::SettingsInput;

#[test]
fn a_shortcut_is_recorded_with_any_keys_that_have_win_ctrl_or_alt() {
    let mut app = IsleApp::new(IsleConfig::default());
    app.settings_action(SettingsAction::Record("ui.minimize_hotkey"));
    assert_eq!(app.settings.view.recording, Some("ui.minimize_hotkey"));
    app.settings_input(SettingsInput::Chord(code(SHIFT, b'K')));
    assert_eq!(app.settings.view.recording, Some("ui.minimize_hotkey"), "waits for a key with Win, Ctrl or Alt");
    assert!(app.settings.view.status.is_some());
    app.settings_input(SettingsInput::Chord(code(CTRL | ALT, b'K')));
    assert_eq!(app.config.ui.minimize_keys, code(CTRL | ALT, b'K'));
    assert_eq!(app.settings.view.recording, None);
    // The character Ctrl+Alt+K may type does not reach the search.
    app.settings_input(SettingsInput::Char('k'));
    assert!(app.settings.view.search.is_empty());
    // Not one of the old list: registered apart, as hotkey 3, which minimizes.
    assert_eq!(app.config.ui.minimize_hotkey, isle_core::config::Hotkey::None);
    assert_eq!(app.recorded_shortcuts(), vec![(3, code(CTRL | ALT, b'K'))]);
    assert_eq!(shortcuts::island_hotkey(3), 1);
    assert_eq!(shortcuts::island_hotkey(2), 2);
    // The notices' Win+N goes on through the keyboard hook; taking it for minimize frees the other.
    app.settings_action(SettingsAction::Record("ui.minimize_hotkey"));
    app.settings_input(SettingsInput::Chord(code(WIN, b'N')));
    assert_eq!(app.config.ui.minimize_hotkey, isle_core::config::Hotkey::WinN);
    assert_eq!(app.config.ui.notices_keys, 0);
    assert!(app.recorded_shortcuts().is_empty());
    app.settings_action(SettingsAction::ClearShortcut("ui.minimize_hotkey"));
    assert_eq!(app.config.ui.minimize_keys, 0);
}

#[test]
fn escape_stops_recording_without_closing_the_window() {
    let mut app = IsleApp::new(IsleConfig::default());
    app.settings.open = true;
    app.settings_action(SettingsAction::Record("ui.notices_hotkey"));
    app.settings_input(SettingsInput::Chord(0x1B));
    app.settings_input(SettingsInput::Char('\u{1b}'));
    assert_eq!(app.settings.view.recording, None);
    assert!(app.settings.open);
    assert_eq!(app.config.ui.notices_keys, code(WIN, b'N'));
}

#[test]
fn banners_follow_their_size_and_width_in_percent() {
    let mut cfg = IsleConfig::default();
    cfg.ui.banner_size = 150;
    cfg.ui.banner_width_pct = 80;
    let app = IsleApp::new(cfg);
    let (w, h, r) = app.notification_banner_size();
    assert_eq!((h, r), (96.0, 30.0));
    assert!(w <= 528.0 && w > 0.0, "{w}");
}
