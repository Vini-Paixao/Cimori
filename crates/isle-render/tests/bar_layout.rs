#![allow(clippy::field_reassign_with_default)]
use isle_core::config::BarItem;
use isle_core::state::{IslandState, MediaInfo, PillMode};
use isle_render::layout::compact_slots;
use isle_render::pages::{UiAction, compute_hitboxes};
use isle_render::text::Text;

fn state() -> IslandState {
    let mut s = IslandState::default();
    s.mode = PillMode::CompactBar;
    s.clock_text = "22:41".into();
    s.date_text = "qua 11 nov".into();
    s.weather = Some(isle_core::weather::Weather { temperature: 24.0, ..Default::default() });
    s.media = Some(MediaInfo { title: "Bohemian Rhapsody".into(), playing: true, ..Default::default() });
    s
}

fn items(slots: &[(BarItem, f32, f32)]) -> Vec<BarItem> {
    slots.iter().map(|s| s.0).collect()
}

#[test]
fn default_order_draws_like_today() {
    let s = state();
    let slots = compact_slots(&Text::new(), 100., 400., &s);
    assert_eq!(items(&slots), [BarItem::Clock, BarItem::Next, BarItem::Weather, BarItem::Tray]);
    assert_eq!(slots[0].1, 116.0, "the clock sits 16 px in");
    let tray = slots.last().unwrap();
    assert!((tray.1 + tray.2 - 488.0).abs() < 0.5, "the tray ends 12 px before the right edge");
    for pair in slots.windows(2) {
        assert!(pair[0].1 + pair[0].2 <= pair[1].1 + 0.5, "{pair:?} overlap");
    }
}

#[test]
fn the_song_shows_only_where_musica_is_chosen_or_always_on_and_once() {
    let mut s = state();
    s.ui.media_controls = true;
    let media = |s: &IslandState| items(&compact_slots(&Text::new(), 0., 600., s)).iter().filter(|i| **i == BarItem::Media).count();
    for notch in [false, true] {
        if notch { s.ui.pill_style = isle_core::config::PillStyle::Notch; s.bar = vec![BarItem::Clock]; }
        assert_eq!(media(&s), 0, "Música not chosen, always off (notch {notch})");
        assert_eq!(isle_render::layout::next_text(&s), "qua 11 nov", "Próximo is not the song");
        s.ui.media_always = true;
        assert_eq!(media(&s), 1, "always on: once (notch {notch})");
        s.bar.push(BarItem::Media);
        assert_eq!(media(&s), 1, "chosen and always on: still once (notch {notch})");
        s.ui.media_always = false;
        assert_eq!(media(&s), 1, "chosen (notch {notch})");
    }
}

#[test]
fn order_follows_the_list_and_empty_items_take_no_room() {
    let mut s = state();
    s.bar = vec![BarItem::Weather, BarItem::Clock, BarItem::Notifications];
    let slots = compact_slots(&Text::new(), 0., 400., &s);
    assert_eq!(items(&slots), [BarItem::Weather, BarItem::Clock], "no unread notification: no bell");
    assert_eq!(slots[0].1, 16.0);
}

#[test]
fn items_that_do_not_fit_drop_from_the_end_but_the_first_stays() {
    let mut s = state();
    s.bar = vec![BarItem::Clock, BarItem::Date, BarItem::Weather, BarItem::Timer, BarItem::Tray, BarItem::Next];
    s.timer_text = Some("Timer 9:00".into());
    let slots = compact_slots(&Text::new(), 0., 200., &s);
    assert_eq!(slots[0].0, BarItem::Clock);
    assert!(slots.len() < 6);
    assert!(slots.iter().all(|(_, x, w)| *x >= 0. && x + w <= 200.5));
}

#[test]
fn hitboxes_sit_on_the_drawn_slots() {
    let mut s = state();
    s.bar = vec![BarItem::Weather, BarItem::Clock];
    let w = isle_render::pages::compact_tray_width(&s);
    let slots = compact_slots(&Text::new(), (720. - w) / 2., w, &s);
    let weather = slots.iter().find(|s| s.0 == BarItem::Weather).unwrap();
    let hb = compute_hitboxes(720., w, 34., 6., &s)
        .into_iter()
        .find(|b| matches!(b.action, UiAction::OpenWeather))
        .unwrap();
    assert!((hb.x - weather.1).abs() < 0.5);
    assert!(compute_hitboxes(720., w, 34., 6., &s)
        .iter()
        .all(|b| !matches!(b.action, UiAction::ToggleTray)), "no tray item: no tray button");
}

#[test]
fn the_bell_opens_the_notifications() {
    let mut s = state();
    s.bar = vec![BarItem::Clock, BarItem::Notifications];
    s.notifications.receive(
        vec![isle_core::notifications::Notification { id: 1, arrival_ms: 1, title: "Oi".into(), ..Default::default() }],
        false,
    );
    let w = isle_render::pages::compact_tray_width(&s);
    assert!(compute_hitboxes(720., w, 34., 6., &s).iter().any(|b| matches!(
        b.action,
        UiAction::SwitchTab(isle_core::state::TabPage::Notifications)
    )));
}

#[test]
fn the_clock_in_the_middle_keeps_its_sides_by_the_list() {
    let mut s = state();
    s.ui.clock_center = true;
    s.bar = vec![BarItem::Clock];
    let slots = compact_slots(&Text::new(), 100., 400., &s);
    assert_eq!(items(&slots), [BarItem::Clock]);
    assert!((slots[0].1 + slots[0].2 / 2. - 300.).abs() < 0.5, "the clock alone in the middle");
    s.bar = vec![BarItem::Date, BarItem::Clock, BarItem::Weather];
    let slots = compact_slots(&Text::new(), 100., 400., &s);
    assert_eq!(items(&slots), [BarItem::Date, BarItem::Clock, BarItem::Weather]);
    assert_eq!(slots[0].1, 116.0, "the date 16 px in");
    assert!((slots[1].1 + slots[1].2 / 2. - 300.).abs() < 0.5, "the clock in the middle");
    assert!((slots[2].1 + slots[2].2 - 488.0).abs() < 0.5, "the weather 12 px before the right edge");
    s.ui.clock_center = false;
    let off = compact_slots(&Text::new(), 100., 400., &s);
    assert!((off[1].1 + off[1].2 / 2. - 300.).abs() > 4., "off: the clock goes right with the rest");
}
