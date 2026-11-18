use isle_core::config::IsleConfig;
use isle_core::prefs::SettingsTab;
use isle_render::settings::{Hit, SettingsAction, SettingsView, hit, layout};
use isle_render::text::Text;
use isle_render::window::MonitorPlace;

fn view(tab: SettingsTab) -> SettingsView {
    SettingsView { tab, ..Default::default() }
}

fn assert_no_overlap(hits: &[Hit]) {
    for (i, a) in hits.iter().enumerate() {
        for b in &hits[i + 1..] {
            let apart = a.x + a.w <= b.x + 0.01 || b.x + b.w <= a.x + 0.01 || a.y + a.h <= b.y + 0.01 || b.y + b.h <= a.y + 0.01;
            assert!(apart, "{:?} overlaps {:?}", a.action, b.action);
        }
    }
}

#[test]
fn every_tab_lays_out_without_overlaps_at_three_sizes() {
    let text = Text::new();
    for (w, h) in [(1180., 760.), (820., 600.), (600., 500.)] {
        for tab in SettingsTab::ALL {
            let l = layout(&view(tab), &IsleConfig::default(), &text, w, h);
            assert_no_overlap(&l.hits);
            assert!(l.hits.iter().all(|b| b.x >= 0. && b.x + b.w <= w + 0.01), "{tab:?} at {w}");
            assert!(l.rows.iter().all(|r| r.x + r.w <= w + 0.01), "{tab:?} rows at {w}");
        }
    }
}

#[test]
fn narrow_window_keeps_only_icons_in_the_side_bar() {
    let l = layout(&view(SettingsTab::Bar), &IsleConfig::default(), &Text::new(), 600., 500.);
    assert!(l.narrow);
    assert!(l.nav.iter().all(|n| n.x + n.w <= 64.01));
    let wide = layout(&view(SettingsTab::Bar), &IsleConfig::default(), &Text::new(), 1180., 760.);
    assert!(!wide.narrow);
}

#[test]
fn a_click_on_a_switch_toggles_that_pref() {
    let l = layout(&view(SettingsTab::Bar), &IsleConfig::default(), &Text::new(), 1180., 760.);
    let r = l.rows.iter().find(|r| r.pref.id == "clock_seconds").unwrap();
    assert_eq!(hit(&l, r.x + r.w - 20., r.y + r.h / 2.), Some(SettingsAction::Toggle("clock_seconds")));
}

#[test]
fn a_click_on_the_slider_track_picks_the_value_under_it() {
    let l = layout(&view(SettingsTab::Notifications), &IsleConfig::default(), &Text::new(), 1180., 760.);
    let track = l.hits.iter().find(|b| matches!(b.action, SettingsAction::Slide("banner_secs", _))).unwrap();
    // 2..30 s: the far right end is 30, the left end 2.
    assert_eq!(hit(&l, track.x + track.w - 0.5, track.y + track.h / 2.), Some(SettingsAction::Slide("banner_secs", 30.)));
    assert_eq!(hit(&l, track.x + 0.5, track.y + track.h / 2.), Some(SettingsAction::Slide("banner_secs", 2.)));
}

#[test]
fn search_lists_matches_from_every_tab() {
    let v = SettingsView { search: "volume".into(), ..view(SettingsTab::Weather) };
    let l = layout(&v, &IsleConfig::default(), &Text::new(), 1180., 760.);
    assert!(l.rows.iter().any(|r| r.pref.tab == SettingsTab::Control));
    assert!(l.rows.iter().all(|r| r.pref.tab != SettingsTab::Weather));
    assert!(l.cards.iter().any(|c| c.title.contains("Central de controle")));
}

#[test]
fn disabled_rows_are_not_clickable() {
    let mut cfg = IsleConfig::default();
    cfg.modules.weather = false;
    let l = layout(&view(SettingsTab::Weather), &cfg, &Text::new(), 1180., 760.);
    assert!(l.rows.iter().any(|r| r.pref.id == "weather_fahrenheit"), "still shown");
    assert!(!l.hits.iter().any(|b| matches!(b.action, SettingsAction::Choose("weather_fahrenheit", _))));
    assert!(l.hits.iter().any(|b| b.action == SettingsAction::Toggle("modules.weather")));
}

#[test]
fn monitors_tab_needs_two_monitors() {
    let place = |d: &str| MonitorPlace { device: d.into(), left: 0, top: 0, width: 1920, scale: 1.0, primary: false };
    let one = SettingsView { monitors: vec![place("A")], ..view(SettingsTab::Bar) };
    let l = layout(&one, &IsleConfig::default(), &Text::new(), 1180., 760.);
    assert!(!l.nav.iter().any(|n| n.action == SettingsAction::Tab(SettingsTab::Monitors)));
    let two = SettingsView { monitors: vec![place("A"), place("B")], ..view(SettingsTab::Monitors) };
    let l = layout(&two, &IsleConfig::default(), &Text::new(), 1180., 760.);
    assert!(l.nav.iter().any(|n| n.action == SettingsAction::Tab(SettingsTab::Monitors)));
    assert!(l.hits.iter().any(|b| b.action == SettingsAction::MainMonitor("B".into())));
}

#[test]
fn scrolling_moves_rows_and_drops_hidden_ones() {
    let top = layout(&view(SettingsTab::Bar), &IsleConfig::default(), &Text::new(), 820., 600.);
    let v = SettingsView { scroll: 300., ..view(SettingsTab::Bar) };
    let l = layout(&v, &IsleConfig::default(), &Text::new(), 820., 600.);
    let content_x = if l.narrow { 64. } else { 236. };
    assert!(l.hits.iter().filter(|b| b.x >= content_x).all(|b| b.y >= 64.));
    assert!(top.content_h > 600., "the Bar tab is taller than the window");
    let r0 = top.rows.iter().find(|r| r.pref.id == "clock_year").unwrap();
    let r1 = l.rows.iter().find(|r| r.pref.id == "clock_year").unwrap();
    assert!((r0.y - r1.y - 300.).abs() < 0.01);
}

#[test]
fn the_bar_tab_puts_its_items_under_the_preview() {
    let l = layout(&view(SettingsTab::Bar), &IsleConfig::default(), &Text::new(), 1180., 760.);
    let (_, py, _, ph) = l.preview.unwrap();
    let grabs: Vec<_> = l.hits.iter().filter(|b| matches!(b.action, SettingsAction::ChipGrab("bar_items", _))).collect();
    assert_eq!(grabs.len(), 4, "the four default items");
    assert!(grabs.iter().all(|b| b.y >= py && b.y + b.h <= py + ph));
    assert!(l.hits.iter().any(|b| b.action == SettingsAction::ChipToggle("bar_items", "date".into())), "date is available");
}

#[test]
fn searching_clears_with_its_button() {
    let v = SettingsView { search: "relógio".into(), ..view(SettingsTab::Bar) };
    let l = layout(&v, &IsleConfig::default(), &Text::new(), 1180., 760.);
    assert!(l.hits.iter().any(|b| b.action == SettingsAction::ClearSearch));
}
