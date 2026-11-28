#![allow(clippy::field_reassign_with_default)]
use isle_core::config::{BarItem, IsleConfig, write_atomic};

#[test]
fn an_old_settings_file_loads_with_todays_behavior() {
    let old = r#"{"gap_px":8,"auto_hide":false}"#;
    let cfg: IsleConfig = serde_json::from_str(old).unwrap();
    assert_eq!(cfg.gap_px, 8);
    assert!(!cfg.auto_hide);
    assert_eq!(
        cfg.bar_items,
        vec![BarItem::Clock, BarItem::Next, BarItem::Weather, BarItem::Tray]
    );
    assert_eq!(cfg.banner_secs, 6);
    assert_eq!(cfg.notifications_per_page, 4);
    assert_eq!(cfg.accent_rgb, (241, 139, 99));
}

#[test]
fn out_of_range_values_fall_back_field_by_field() {
    let mut cfg = IsleConfig::default();
    cfg.hook_port = 0;
    cfg.banner_secs = 0;
    cfg.notifications_per_page = 99;
    cfg.countdown_hours = 0;
    cfg.bar_items.clear();
    cfg.gap_px = 900;
    let cfg = cfg.sanitized();
    assert_eq!(cfg.hook_port, 47830);
    assert_eq!(cfg.banner_secs, 6);
    assert_eq!(cfg.notifications_per_page, 4);
    assert_eq!(cfg.countdown_hours, 12);
    assert_eq!(cfg.bar_items, vec![BarItem::Clock]);
    assert_eq!(cfg.gap_px, 6);
}

#[test]
fn repeated_bar_items_keep_the_first() {
    let mut cfg = IsleConfig::default();
    cfg.bar_items = vec![BarItem::Weather, BarItem::Clock, BarItem::Weather];
    assert_eq!(cfg.sanitized().bar_items, vec![BarItem::Weather, BarItem::Clock]);
}

#[test]
fn a_field_with_the_wrong_type_keeps_the_rest() {
    let cfg = IsleConfig::load_lenient(r#"{"gap_px":"oops","auto_hide":false}"#);
    assert!(!cfg.auto_hide);
    assert_eq!(cfg.gap_px, 6);
}

#[test]
fn a_file_that_is_not_json_loads_the_defaults() {
    assert_eq!(IsleConfig::load_lenient("{trunc"), IsleConfig::default());
}

#[test]
fn atomic_write_replaces_the_file_whole() {
    let dir = std::env::temp_dir().join(format!("cimori-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    write_atomic(&path, "{\"a\":1}").unwrap();
    write_atomic(&path, "{\"a\":2}").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"a\":2}");
    assert!(!dir.join("settings.json.tmp").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_settings_file_that_is_not_json_is_kept_aside_before_defaults_replace_it() {
    let dir = std::env::temp_dir().join(format!("cimori-bad-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    std::fs::write(&path, "{\"gap_px\": 8,, oops").unwrap();
    let cfg = IsleConfig::load_or_import(&path, None).unwrap();
    assert_eq!(cfg, IsleConfig::default());
    assert_eq!(std::fs::read_to_string(dir.join("settings.json.bad")).unwrap(), "{\"gap_px\": 8,, oops");
    std::fs::write(&path, "{\"gap_px\": \"x\", \"auto_hide\": false}").unwrap();
    let cfg = IsleConfig::load_or_import(&path, None).unwrap();
    assert!(!cfg.auto_hide);
    assert!(std::fs::read_to_string(dir.join("settings.json.bad")).unwrap().contains("\"x\""), "a dropped key also keeps the original");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_island_opens_on_music_and_the_pref_names_every_tab_in_order() {
    use isle_core::prefs::{PrefKind, PrefValue, by_id};
    use isle_core::state::TabPage;
    let old: IsleConfig = serde_json::from_str(r#"{"ui":{"hover_open":true}}"#).unwrap();
    assert_eq!(old.ui.open_tab, TabPage::Media);
    let pref = by_id("ui.open_tab").unwrap();
    assert!(matches!(pref.kind, PrefKind::Choice(_)));
    let names = pref.choices();
    assert_eq!(names, TabPage::NAV.map(TabPage::label).to_vec());
    let mut cfg = IsleConfig::default();
    (pref.set)(&mut cfg, PrefValue::Index(names.len() - 1));
    assert_eq!(cfg.ui.open_tab, TabPage::AiHub);
    assert_eq!((pref.get)(&cfg), PrefValue::Index(names.len() - 1));
}
