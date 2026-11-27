use isle_core::config::IsleConfig;
use isle_core::prefs::{self, PrefKind, PrefValue, SettingsTab};

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

/// A value other than `v` that the row could be given from the settings window.
fn another(p: &prefs::Pref, v: &prefs::PrefValue) -> Option<prefs::PrefValue> {
    use prefs::PrefValue::*;
    Some(match (&p.kind, v) {
        (PrefKind::Switch, Bool(b)) => Bool(!b),
        (PrefKind::Choice(options), Index(i)) => Index((i + 1) % options.len()),
        (PrefKind::Slider { min, max, step, .. }, Number(n)) => Number(if (n - min).abs() > step / 2.0 { *min } else { (min + step).min(*max) }),
        (PrefKind::Color(list), Rgb(r, g, b)) => {
            let (r, g, b) = *list.iter().find(|c| **c != (*r, *g, *b))?;
            Rgb(r, g, b)
        }
        (PrefKind::Text { .. }, Text(_)) if p.id == "hooks_token" => Text("ab".repeat(32)),
        (PrefKind::Text { .. }, Text(t)) if p.id == "holidays_country" => Text(if t == "PT" { "US".into() } else { "PT".into() }),
        (PrefKind::Text { .. }, Text(t)) => Text(if t == "12345" { "6789".into() } else { "12345".into() }),
        (PrefKind::Chips, List(l)) => {
            let choices = prefs::chip_choices(p.id);
            if choices.is_empty() {
                List(vec!["teste.exe".into()])
            } else if l.len() > 1 {
                List(l.iter().rev().cloned().collect())
            } else {
                List(choices.iter().map(|(k, _)| k.to_string()).filter(|k| !l.contains(k)).take(2).chain(l.clone()).collect())
            }
        }
        (PrefKind::Monitors, List(_)) => List(vec![r"\.\DISPLAY2".into()]),
        (PrefKind::Monitors, Text(_)) => Text(r"\.\DISPLAY2".into()),
        (PrefKind::Shortcut, Index(i)) => {
            use isle_core::shortcut::{code, ALT, CTRL};
            let (a, b) = (code(CTRL | ALT, b'K'), code(CTRL | ALT, b'L'));
            Index(if *i == a as usize { b } else { a } as usize)
        }
        (PrefKind::CitySearch, List(_)) => List(vec!["Recife, Pernambuco".into(), "-8.05".into(), "-34.9".into()]),
        (PrefKind::CitySearch, Text(t)) => Text(if t == "Recife" { "Natal".into() } else { "Recife".into() }),
        _ => return None,
    })
}

/// Every row, changed as the settings window changes it, keeps its new value through what the app does
/// with it: `sanitized`, settings.json written and read back.
#[test]
fn every_pref_keeps_a_changed_value_through_settings_json() {
    let mut failures = Vec::new();
    for p in prefs::all() {
        if matches!(p.kind, PrefKind::Action(_) | PrefKind::Info) {
            continue;
        }
        let mut c = IsleConfig::default();
        let before = (p.get)(&c);
        let Some(value) = another(p, &before) else {
            failures.push(format!("{}: no other value for {:?}", p.id, before));
            continue;
        };
        (p.set)(&mut c, value.clone());
        let set = (p.get)(&c);
        if set == before {
            failures.push(format!("{}: set {value:?} left {before:?}", p.id));
            continue;
        }
        let c = c.sanitized();
        if (p.get)(&c) != set {
            failures.push(format!("{}: sanitized turned {set:?} into {:?}", p.id, (p.get)(&c)));
            continue;
        }
        let json = serde_json::to_string_pretty(&c).unwrap();
        let (read, whole) = IsleConfig::load_checked(&json);
        if !whole {
            failures.push(format!("{}: settings.json with {set:?} did not read back whole", p.id));
        }
        if (p.get)(&read) != set {
            failures.push(format!("{}: saved {set:?}, read back {:?}", p.id, (p.get)(&read)));
        }
        if read != c {
            failures.push(format!("{}: other settings changed on the way back", p.id));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn tab_order_moves_the_shown_tabs_and_ignores_adding_or_removing() {
    use isle_core::state::TabPage;
    let p = prefs::by_id("modules.order").unwrap();
    let mut c = isle_core::config::IsleConfig::default();
    let PrefValue::List(mut names) = (p.get)(&c) else { panic!() };
    assert_eq!(names.first().map(String::as_str), Some("media"));
    let uso = names.pop().unwrap();
    names.insert(0, uso);
    (p.set)(&mut c, PrefValue::List(names.clone()));
    assert_eq!((p.get)(&c), PrefValue::List(names.clone()));
    // The hidden tabs (Prateleira, off) keep their places among all twelve.
    assert_eq!(c.modules.order.iter().position(|t| *t == TabPage::Shelf), TabPage::NAV.iter().position(|t| *t == TabPage::Shelf));
    // Dragged out, or a stranger added: nothing changes.
    let before = c.modules.order;
    (p.set)(&mut c, PrefValue::List(names[1..].to_vec()));
    let mut more = names.clone();
    more.push("shelf".into());
    (p.set)(&mut c, PrefValue::List(more));
    assert_eq!(c.modules.order, before);
    // Each chip shows its tab's name.
    for t in TabPage::NAV {
        let key = prefs::tab_key(t);
        assert_eq!(prefs::chip_choices("modules.order").iter().find(|c| c.0 == key).map(|c| c.1), Some(t.label()));
    }
    // A broken order on disk comes back as isle's.
    c.modules.order = [TabPage::Media; 12];
    assert_eq!(c.sanitized().modules.order, TabPage::NAV);
}
