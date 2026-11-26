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

/// Side by side, `BAR_GAP` (8 px) apart, the group in the middle of the pill: no hole between them.
fn assert_packed(slots: &[(BarItem, f32, f32)]) {
    for pair in slots.windows(2) {
        let gap = pair[1].1 - (pair[0].1 + pair[0].2);
        assert!((gap - 8.0).abs() < 0.5, "{pair:?}: {gap} px apart");
    }
}

#[test]
fn items_sit_side_by_side_in_the_middle_of_the_pill() {
    let s = state();
    let slots = compact_slots(&Text::new(), 100., 400., &s);
    assert_eq!(items(&slots), [BarItem::Clock, BarItem::Next, BarItem::Weather, BarItem::Tray]);
    assert_packed(&slots);
    let (first, last) = (slots[0], *slots.last().unwrap());
    let (left, right) = (first.1 - 100., 500. - (last.1 + last.2));
    assert!((left - right).abs() < 0.5, "as much room on both ends: {left} {right}");
}

#[test]
fn the_pill_is_as_wide_as_its_items_at_least_the_chosen_width() {
    let mut s = state();
    s.ui.pill_width = 160;
    s.bar = vec![BarItem::Clock, BarItem::Weather, BarItem::Date, BarItem::Media];
    let w = isle_render::pages::compact_tray_width(&s);
    let slots = compact_slots(&Text::new(), 0., w, &s);
    assert_eq!(items(&slots), [BarItem::Clock, BarItem::Weather, BarItem::Date, BarItem::Media], "nothing dropped");
    assert!((slots[0].1 - 14.).abs() < 0.5 && (w - 14. - (slots[3].1 + slots[3].2)).abs() < 0.5, "14 px from each edge");
    s.ui.pill_width = 560;
    assert_eq!(isle_render::pages::compact_tray_width(&s), 560., "never under the chosen width");
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
    assert_packed(&slots);
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

fn clock_middle(slots: &[(BarItem, f32, f32)]) -> f32 {
    let c = slots.iter().find(|s| s.0 == BarItem::Clock).unwrap();
    c.1 + c.2 / 2.
}

#[test]
fn the_clock_in_the_middle_with_the_others_on_its_two_sides() {
    let mut s = state();
    s.ui.clock_center = true;
    s.bar = vec![BarItem::Clock];
    let slots = compact_slots(&Text::new(), 100., 400., &s);
    assert!((clock_middle(&slots) - 300.).abs() < 0.5, "the clock alone in the middle");
    // First in the list, the clock still goes in the middle: the others share its sides, in order.
    s.bar = vec![BarItem::Clock, BarItem::Weather, BarItem::Date, BarItem::Media];
    let w = isle_render::pages::compact_tray_width(&s);
    let slots = compact_slots(&Text::new(), 0., w, &s);
    assert_eq!(slots.len(), 4, "nothing dropped: {slots:?}");
    assert!((clock_middle(&slots) - w / 2.).abs() < 0.5, "the clock in the middle of the pill");
    let left: Vec<_> = slots.iter().filter(|i| i.1 < clock_middle(&slots) && i.0 != BarItem::Clock).map(|i| i.0).collect();
    assert!(!left.is_empty() && left.len() < 3, "items on both sides: {slots:?}");
    let order: Vec<_> = slots.iter().map(|i| i.0).filter(|i| *i != BarItem::Clock).collect();
    assert_eq!(order, [BarItem::Weather, BarItem::Date, BarItem::Media], "the others keep their order");
    let others: Vec<_> = slots.iter().copied().filter(|i| i.0 != BarItem::Clock).collect();
    assert_packed(&others[..left.len()]);
    assert_packed(&others[left.len()..]);
    let clock = slots.iter().find(|i| i.0 == BarItem::Clock).unwrap();
    let before = slots.iter().filter(|i| i.1 < clock.1).last().unwrap();
    assert!((clock.1 - (before.1 + before.2) - 14.).abs() < 0.5, "more room on the clock's sides");
}

#[test]
fn the_clock_keeps_its_place_in_the_list_while_the_sides_are_nearly_even() {
    let mut s = state();
    s.ui.clock_center = true;
    s.bar = vec![BarItem::Date, BarItem::Clock, BarItem::Weather];
    let w = isle_render::pages::compact_tray_width(&s);
    let slots = compact_slots(&Text::new(), 0., w, &s);
    assert_eq!(items(&slots), [BarItem::Date, BarItem::Clock, BarItem::Weather]);
    assert!((clock_middle(&slots) - w / 2.).abs() < 0.5);
    s.ui.clock_center = false;
    let w = isle_render::pages::compact_tray_width(&s);
    let off = compact_slots(&Text::new(), 0., w, &s);
    assert_eq!(items(&off), [BarItem::Date, BarItem::Clock, BarItem::Weather], "off: just the list's order");
    assert_packed(&off);
}

#[test]
fn a_ticking_clock_does_not_change_the_pill() {
    let mut s = state();
    s.clock_text = "11:11".into();
    let a = isle_render::pages::compact_tray_width(&s);
    s.clock_text = "20:08".into();
    assert_eq!(a, isle_render::pages::compact_tray_width(&s));
}
