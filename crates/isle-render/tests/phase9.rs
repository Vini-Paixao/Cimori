//! Phase 9 (Claude track): the notch drawn and laid out.
use isle_core::config::{IsleConfig, PillStyle};
use isle_core::spring::PillGeometry;
use isle_core::state::{IslandState, PillMode, TabPage};
use isle_render::renderer::{PillRenderer, notch_radius, notch_shoulder};

fn settled(w: f32, h: f32, r: f32) -> PillGeometry {
    let mut g = PillGeometry::default();
    g.morph_to(w, h, r);
    (g.width.value, g.height.value, g.corner_radius.value) = (w, h, r);
    g
}

fn notch_cfg() -> IsleConfig {
    let mut cfg = IsleConfig::default();
    cfg.ui.pill_style = PillStyle::Notch;
    cfg
}

#[test]
fn the_notch_touches_the_top_edge_with_its_curves_and_the_pill_floats() {
    let r = notch_radius(34.0, 75);
    let sh = notch_shoulder(r, 75);
    assert!(r > 0.0 && sh > 0.0);
    for scale in [1.0f32, 1.5] {
        let mut renderer = PillRenderer::with_scale(720, 520, scale);
        let alpha = |fb: &isle_render::renderer::PillFrameBuffer, x: f32, y: f32| {
            fb.bgra_premultiplied[(((y * scale) as u32 * fb.width + (x * scale) as u32) * 4 + 3) as usize]
        };
        let state = IslandState::default();
        let fb = renderer.render(&settled(280.0, 34.0, r), &state, &notch_cfg());
        assert!(alpha(fb, 360.0, 0.5) > 200, "the notch starts at the screen's edge");
        // The curve the other way, just outside the body's left side, against the edge.
        assert!(alpha(fb, 220.0 - sh * 0.3, 0.5) > 100, "its shoulder flares along the edge");
        assert_eq!(alpha(fb, 220.0 - sh - 4.0, 0.5), 0);
        let fb = renderer.render(&settled(280.0, 34.0, 17.0), &state, &IsleConfig::default());
        assert_eq!(alpha(fb, 360.0, 0.5), 0, "the pill keeps its gap");
    }
}

#[test]
fn the_notch_line_has_no_curves() {
    let mut state = IslandState::default();
    state.mode = PillMode::Minimized;
    let mut renderer = PillRenderer::new(720, 520);
    let fb = renderer.render(&settled(120.0, 5.0, 2.5), &state, &notch_cfg());
    let alpha = |x: u32, y: u32| fb.bgra_premultiplied[((y * 720 + x) * 4 + 3) as usize];
    assert!(alpha(360, 2) > 200);
    // Beside it only the unseen strip that takes the mouse (alpha 1), no shoulder.
    assert!(alpha(360 - 60 - 6, 1) <= 1);
}

#[test]
fn the_notch_keeps_its_one_item_in_the_middle() {
    let mut s = IslandState::default();
    s.ui = notch_cfg().ui;
    s.bar = notch_cfg().bar_shown();
    let text = isle_render::text::Text::new();
    let w = isle_render::pages::compact_tray_width(&s);
    let slots = isle_render::layout::compact_slots(&text, 100.0, w, &s);
    assert_eq!(slots.len(), 1);
    let (_, x, sw) = slots[0];
    assert!(((x - 100.0) - (100.0 + w - x - sw)).abs() < 0.5, "centred: {x} {sw} in {w}");
}

#[test]
fn the_closed_pill_width_follows_the_setting() {
    let mut s = IslandState::default();
    let base = isle_render::pages::compact_tray_width(&s);
    s.ui.pill_width += 40;
    assert_eq!(isle_render::pages::compact_tray_width(&s), base + 40.0);
}

#[test]
fn an_open_notch_still_shows_its_tabs() {
    let mut s = IslandState::default();
    s.mode = PillMode::Expanded(TabPage::ControlCenter);
    let (w, h, r) = isle_render::layout::expanded_size(TabPage::ControlCenter);
    let mut renderer = PillRenderer::new(720, 520);
    let fb = renderer.render(&settled(w, h, r), &s, &notch_cfg());
    let a = |x: u32, y: u32| fb.bgra_premultiplied[((y * 720 + x) * 4 + 3) as usize];
    assert!(a(360, 0) > 200 && a(360, h as u32 - 4) > 200);
}

#[test]
fn the_power_menu_lays_its_actions_and_confirmation_inside_the_panel() {
    use isle_core::power::PowerAction;
    use isle_render::pages::{UiAction, compute_hitboxes};
    let mut s = IslandState::default();
    s.mode = PillMode::Expanded(TabPage::Power);
    s.power_confirm = Some(PowerAction::ShutDown);
    let (w, h, r) = isle_render::layout::expanded_size(TabPage::Power);
    let boxes = compute_hitboxes(720.0, w, h, 6.0, &s);
    let mine: Vec<_> = boxes.iter().filter(|b| matches!(b.action, UiAction::Power(_) | UiAction::PowerConfirm | UiAction::PowerCancel)).collect();
    assert_eq!(mine.len(), 6 + 2);
    for (i, a) in mine.iter().enumerate() {
        assert!(a.x >= (720.0 - w) / 2.0 && a.x + a.w <= (720.0 + w) / 2.0 && a.y + a.h <= 6.0 + h, "{a:?}");
        for b in &mine[i + 1..] {
            assert!(a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y, "overlap {a:?} {b:?}");
        }
    }
    s.hibernate_allowed = false;
    s.power_confirm = None;
    assert_eq!(compute_hitboxes(720.0, w, h, 6.0, &s).iter().filter(|b| matches!(b.action, UiAction::Power(_))).count(), 5);
    for scale in [1.0f32, 1.5] {
        let mut renderer = PillRenderer::with_scale(720, 520, scale);
        let fb = renderer.render(&settled(w, h, r), &s, &IsleConfig::default());
        let hb = compute_hitboxes(720.0, w, h, 6.0, &s).into_iter().find(|b| b.action == UiAction::Power(PowerAction::Lock)).unwrap();
        let inked = (hb.y as u32..(hb.y + hb.h) as u32).flat_map(|y| (hb.x as u32..(hb.x + hb.w) as u32).map(move |x| (x, y)))
            .filter(|&(x, y)| fb.bgra_premultiplied[((((y as f32 * scale) as u32) * fb.width + (x as f32 * scale) as u32) * 4 + 2) as usize] > 150)
            .count();
        assert!(inked > 20, "icon and name drawn at {scale}");
    }
    // Not a tab of the navigation.
    assert!(!isle_render::layout::visible_tabs(&isle_core::config::Modules::default()).contains(&TabPage::Power));
}

#[test]
fn the_power_item_on_the_bar_opens_the_menu() {
    use isle_render::pages::{UiAction, compute_hitboxes};
    let mut s = IslandState::default();
    s.bar = vec![isle_core::config::BarItem::Clock, isle_core::config::BarItem::Power];
    let w = isle_render::pages::compact_tray_width(&s);
    let hit = compute_hitboxes(720.0, w, 34.0, 6.0, &s).into_iter().find(|b| b.action == UiAction::OpenPower).unwrap();
    assert!(hit.x + hit.w <= (720.0 + w) / 2.0);
}

#[test]
fn edit_mode_marks_each_control_and_lists_the_rest_without_overlap() {
    use isle_core::state::ControlTile;
    use isle_render::pages::{UiAction, compute_hitboxes};
    let mut s = IslandState::default();
    s.mode = PillMode::Expanded(TabPage::ControlCenter);
    s.audio_available = true;
    s.control_editing = true;
    let (w, h, r) = isle_render::layout::expanded_size(TabPage::ControlCenter);
    let natural = isle_render::layout::content_height(TabPage::ControlCenter, &s);
    assert!(natural > h, "the catalog scrolls: {natural}");
    let all = isle_render::layout::expanded_hitboxes(720.0, w, natural, 6.0, &s);
    let count = |f: fn(&UiAction) -> bool| all.iter().filter(|b| f(&b.action)).count();
    assert_eq!(count(|a| matches!(a, UiAction::ControlRemove(_))), 10);
    assert_eq!(count(|a| matches!(a, UiAction::ControlGrab(_))), 10);
    assert_eq!(count(|a| matches!(a, UiAction::ControlAdd(_))), ControlTile::CATALOG.len() - 10);
    assert_eq!(count(|a| matches!(a, UiAction::SetVolume(_) | UiAction::Control(_))), 0, "no sound while editing");
    let adds: Vec<_> = all.iter().filter(|b| matches!(b.action, UiAction::ControlAdd(_) | UiAction::ControlGrab(_))).collect();
    for (i, a) in adds.iter().enumerate() {
        assert!(a.x >= (720.0 - w) / 2.0 && a.x + a.w <= (720.0 + w) / 2.0 && a.y + a.h <= 6.0 + natural, "{a:?}");
        for b in &adds[i + 1..] {
            assert!(a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y, "overlap {a:?} {b:?}");
        }
    }
    // Each "−" sits on its control and is found before it.
    let minus = all.iter().position(|b| b.action == UiAction::ControlRemove(ControlTile::Wifi)).unwrap();
    let grab = all.iter().position(|b| b.action == UiAction::ControlGrab(0)).unwrap();
    assert!(minus < grab && all[grab].contains(all[minus].x + 1., all[minus].y + 1.));
    // Drawn at both scales: the "−" badges and the header's button.
    for scale in [1.0f32, 1.5] {
        let mut renderer = PillRenderer::with_scale(720, 520, scale);
        let fb = renderer.render(&settled(w, h, r), &s, &IsleConfig::default());
        let hb = compute_hitboxes(720.0, w, h, 6.0, &s).into_iter().find(|b| b.action == UiAction::ControlRemove(ControlTile::Project)).unwrap();
        let px = |x: f32, y: f32| fb.bgra_premultiplied[((((y * scale) as u32) * fb.width + (x * scale) as u32) * 4) as usize];
        assert!(px(hb.x + hb.w / 2., hb.y + hb.h / 2.) > 150, "the minus is white at {scale}");
        assert!(compute_hitboxes(720.0, w, h, 6.0, &s).iter().any(|b| b.action == UiAction::EditControls));
    }
}
