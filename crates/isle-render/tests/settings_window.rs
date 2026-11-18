use isle_core::config::WindowPlacement;
use isle_render::settings_window::fit_placement;

/// Work areas (left, top, right, bottom); the first is the primary monitor.
const MONITORS: [(i32, i32, i32, i32); 2] = [(0, 0, 2560, 1400), (2560, 0, 5120, 1400)];

#[test]
fn a_placement_on_a_monitor_that_is_gone_is_centered_on_the_main_one() {
    let p = WindowPlacement { x: 9000, y: 200, w: 1180, h: 760, tab: "bar".into() };
    assert_eq!(fit_placement(Some(&p), &MONITORS, 1.0), ((2560 - 1180) / 2, (1400 - 760) / 2, 1180, 760));
}

#[test]
fn a_placement_on_a_live_monitor_is_kept() {
    let p = WindowPlacement { x: 3000, y: 100, w: 900, h: 700, tab: "bar".into() };
    assert_eq!(fit_placement(Some(&p), &MONITORS, 1.0), (3000, 100, 900, 700));
}

#[test]
fn the_first_time_it_opens_centered_at_the_screen_scale() {
    assert_eq!(fit_placement(None, &MONITORS, 1.5), ((2560 - 1770) / 2, (1400 - 1140) / 2, 1770, 1140));
}

#[test]
fn it_never_opens_bigger_than_the_work_area() {
    let small = [(0, 0, 1280, 680)];
    let (_, _, w, h) = fit_placement(None, &small, 1.0);
    assert!(w <= 1280 && h <= 680);
}
