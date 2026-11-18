//! Phase 9 (Claude track): the notch, the power button, the editable control center, units and keys.
use isle_core::config::{BarItem, IsleConfig, PillStyle};
use isle_core::prefs::{self, PrefValue};

#[test]
fn a_settings_file_from_before_the_notch_keeps_the_pill() {
    let cfg = IsleConfig::load_lenient(r#"{"gap_px": 9, "bar_items": ["Weather"]}"#);
    assert_eq!(cfg.ui.pill_style, PillStyle::Island);
    assert_eq!(cfg.notch_items, vec![BarItem::Clock]);
    assert_eq!(cfg.top_gap(), 9.0);
    assert_eq!(cfg.bar_shown(), vec![BarItem::Weather]);
}

#[test]
fn the_notch_hangs_from_the_top_and_shows_its_own_items() {
    let mut cfg = IsleConfig::default();
    (prefs::by_id("ui.pill_style").unwrap().set)(&mut cfg, PrefValue::Index(1));
    assert_eq!(cfg.ui.pill_style, PillStyle::Notch);
    assert_eq!(cfg.top_gap(), 0.0);
    assert_eq!(cfg.bar_shown(), vec![BarItem::Clock]);
    let row = prefs::by_id("notch_items").unwrap();
    assert!((row.enabled)(&cfg));
    (row.set)(&mut cfg, PrefValue::List(vec!["weather".into(), "clock".into()]));
    assert_eq!(cfg.bar_shown(), vec![BarItem::Weather, BarItem::Clock]);
    assert!(!(row.enabled)(&IsleConfig::default()), "only for the notch");
}

#[test]
fn notch_items_and_width_are_kept_in_range() {
    let mut cfg = IsleConfig::default();
    cfg.notch_items = vec![BarItem::Clock, BarItem::Clock];
    cfg.ui.pill_width = 5;
    let cfg = cfg.sanitized();
    assert_eq!(cfg.notch_items, vec![BarItem::Clock]);
    assert_eq!(cfg.ui.pill_width, IsleConfig::default().ui.pill_width);
    let mut empty = IsleConfig::default();
    empty.notch_items.clear();
    assert_eq!(empty.sanitized().notch_items, vec![BarItem::Clock]);
}

#[test]
fn the_closed_pill_is_wider_than_it_was() {
    assert!(IsleConfig::default().ui.pill_width > 220);
}

#[test]
fn an_old_settings_file_gets_the_mods_units_and_its_shortcuts_as_keys() {
    use isle_core::config::Hotkey;
    use isle_core::shortcut::{ALT, CTRL, code};
    let cfg = IsleConfig::load_lenient(r#"{"ui": {"pill_height": 40, "banner_width": 600, "banner_height": 80,
        "minimize_hotkey": "CtrlAltJ", "notices_hotkey": "None"}}"#);
    assert_eq!(cfg.ui.pill_height_pct, 120, "40 of 34 px");
    assert_eq!(cfg.ui.banner_size, 125, "80 of 64 px");
    assert_eq!(cfg.ui.banner_width_pct, 100, "600 of 440 px at 125 %");
    assert_eq!(cfg.ui.minimize_keys, code(CTRL | ALT, b'J'));
    assert_eq!(cfg.ui.minimize_hotkey, Hotkey::CtrlAltJ);
    assert_eq!(cfg.ui.notices_keys, 0);
    // Written again, only the new units are kept.
    let json = serde_json::to_string(&cfg).unwrap();
    assert!(json.contains("pill_height_pct") && !json.contains("\"pill_height\""));
    assert_eq!(IsleConfig::load_lenient(&json), cfg);
}

#[test]
fn units_stay_on_the_mods_values() {
    let mut cfg = IsleConfig::default();
    assert_eq!(cfg.ui.pill_height_px(), 34.0);
    assert_eq!(cfg.ui.banner_px(), (440.0, 64.0));
    cfg.ui.banner_size = 110;
    cfg.ui.banner_width_pct = 90;
    cfg.ui.pill_height_pct = 200;
    let cfg = cfg.sanitized();
    assert_eq!((cfg.ui.banner_size, cfg.ui.banner_width_pct, cfg.ui.pill_height_pct), (100, 100, 100));
    let size = prefs::by_id("ui.banner_size").unwrap();
    let mut c = IsleConfig::default();
    (size.set)(&mut c, PrefValue::Index(5));
    assert_eq!(c.ui.banner_size, 200);
    assert_eq!(c.ui.banner_px().1, 128.0);
}

#[test]
fn a_shortcut_row_takes_only_keys_with_win_ctrl_or_alt() {
    use isle_core::shortcut::{SHIFT, WIN, code};
    let row = prefs::by_id("ui.minimize_hotkey").unwrap();
    let mut c = IsleConfig::default();
    (row.set)(&mut c, PrefValue::Index(code(SHIFT, b'A') as usize));
    assert_eq!(c.ui.minimize_keys, 0);
    (row.set)(&mut c, PrefValue::Index(code(WIN | SHIFT, b'A') as usize));
    assert_eq!((row.get)(&c), PrefValue::Index(code(WIN | SHIFT, b'A') as usize));
}
