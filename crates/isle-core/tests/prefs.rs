use isle_core::config::IsleConfig;
use isle_core::prefs::{self, PrefKind, SettingsTab};

#[test]
fn every_pref_round_trips_through_get_and_set() {
    for p in prefs::all() {
        if matches!(p.kind, PrefKind::Action(_) | PrefKind::Info) {
            continue;
        }
        let mut c = IsleConfig::default();
        let v = (p.get)(&c);
        (p.set)(&mut c, v.clone());
        assert_eq!((p.get)(&c), v, "{}", p.id);
        assert_eq!(c, IsleConfig::default(), "{} changes nothing when set to itself", p.id);
    }
}

#[test]
fn ids_are_unique_and_every_tab_has_a_row() {
    let mut ids: Vec<_> = prefs::all().iter().map(|p| p.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), prefs::all().len());
    for t in SettingsTab::ALL {
        assert!(prefs::all().iter().any(|p| p.tab == t), "{t:?}");
    }
}

#[test]
fn search_ignores_accents_and_case() {
    assert!(prefs::search("animacao").iter().any(|p| p.tab == SettingsTab::Animations));
    assert!(prefs::search("SEGUNDOS").iter().any(|p| p.id == "clock_seconds"));
    assert!(prefs::search("fahrenheit").iter().any(|p| p.id == "weather_fahrenheit"));
    assert!(prefs::search("zzzz").is_empty());
    assert!(prefs::search("").is_empty());
}

#[test]
fn every_word_of_the_search_must_match() {
    let found = prefs::search("relógio segundos");
    assert!(found.iter().any(|p| p.id == "clock_seconds"));
    assert!(found.iter().all(|p| p.id != "clock_year"));
}

#[test]
fn second_wave_preferences_are_available() {
    for id in ["notch_curve", "ui.media_lyric_title", "ui.tray_keep_open", "ui.minimize_hotkey"] {
        assert!(prefs::by_id(id).is_some(), "{id}");
    }
}

#[test]
fn setting_a_module_off_disables_its_other_rows() {
    let p = prefs::by_id("weather_fahrenheit").unwrap();
    let mut c = IsleConfig::default();
    assert!((p.enabled)(&c));
    c.modules.weather = false;
    assert!(!(p.enabled)(&c));
    assert!((prefs::by_id("modules.weather").unwrap().enabled)(&c), "the module switch itself stays");
}

#[test]
fn chips_list_only_known_keys() {
    for p in prefs::all().iter().filter(|p| matches!(p.kind, PrefKind::Chips)) {
        let choices = prefs::chip_choices(p.id);
        if let isle_core::prefs::PrefValue::List(keys) = (p.get)(&IsleConfig::default()) {
            for k in keys {
                assert!(choices.iter().any(|(c, _)| *c == k), "{} {k}", p.id);
            }
        }
    }
}

#[test]
fn bar_chips_set_the_order() {
    let p = prefs::by_id("bar_items").unwrap();
    let mut c = IsleConfig::default();
    (p.set)(&mut c, prefs::PrefValue::List(vec!["weather".into(), "clock".into()]));
    use isle_core::config::BarItem;
    assert_eq!(c.bar_items, vec![BarItem::Weather, BarItem::Clock]);
}

#[test]
fn phase8_modules_are_explicit_opt_in_and_have_live_settings() {
    use isle_core::prefs::{by_id, PrefValue};
    let mut config = isle_core::config::IsleConfig::default();
    for id in ["modules.clipboard", "modules.shelf", "modules.usb", "modules.downloads"] {
        let pref = by_id(id).unwrap_or_else(|| panic!("missing preference {id}"));
        assert!(!pref.restart);
        assert_eq!((pref.get)(&config), PrefValue::Bool(false));
        (pref.set)(&mut config, PrefValue::Bool(true));
        assert_eq!((pref.get)(&config), PrefValue::Bool(true));
    }
}
