#![allow(clippy::field_reassign_with_default, clippy::needless_range_loop, clippy::chunks_exact_to_as_chunks)]
use isle_core::config::IsleConfig;
use isle_core::prefs::SettingsTab;
use isle_core::state::IslandState;
use isle_render::renderer::{PillFrameBuffer, PillRenderer};
use isle_render::settings::{SettingsView, control_rect, layout};
use isle_render::text::Text;

const W: f32 = 1180.;
const H: f32 = 760.;

fn view(tab: SettingsTab) -> SettingsView {
    SettingsView { tab, ..Default::default() }
}

fn state() -> IslandState {
    let mut s = IslandState::default();
    s.clock_text = "22:41".into();
    s.date_text = "qua 11 nov".into();
    s
}

/// Pixels near the default accent (241, 139, 99) inside a logical rect.
fn accent_in(fb: &PillFrameBuffer, scale: f32, (x, y, w, h): (f32, f32, f32, f32)) -> usize {
    let mut n = 0;
    for py in (y * scale) as u32..((y + h) * scale) as u32 {
        for px in (x * scale) as u32..((x + w) * scale) as u32 {
            let i = ((py * fb.width + px) * 4) as usize;
            let (b, g, r) = (fb.bgra_premultiplied[i], fb.bgra_premultiplied[i + 1], fb.bgra_premultiplied[i + 2]);
            if r > 200 && (110..170).contains(&g) && b < 130 {
                n += 1;
            }
        }
    }
    n
}

fn bright_in(fb: &PillFrameBuffer, (x, y, w, h): (f32, f32, f32, f32)) -> usize {
    let mut n = 0;
    for py in y as u32..(y + h) as u32 {
        for px in x as u32..(x + w) as u32 {
            let i = ((py * fb.width + px) * 4) as usize;
            if fb.bgra_premultiplied[i..i + 3].iter().all(|c| *c > 200) {
                n += 1;
            }
        }
    }
    n
}

fn switch_of(id: &str, scale: f32, cfg: &IsleConfig) -> usize {
    let v = view(SettingsTab::Bar);
    let l = layout(&v, cfg, &Text::new(), W, H);
    let row = l.rows.iter().find(|r| r.pref.id == id).unwrap();
    let rect = control_rect(row, &Text::new());
    let mut r = PillRenderer::with_scale(W as u32, H as u32, scale);
    let fb = r.render_settings(&v, cfg, &state());
    accent_in(fb, scale, rect)
}

#[test]
fn switches_draw_accent_only_when_on() {
    let cfg = IsleConfig::default();
    assert!(switch_of("clock_weekday", 1.0, &cfg) > 40, "on");
    assert_eq!(switch_of("clock_seconds", 1.0, &cfg), 0, "off");
}

#[test]
fn draws_at_150_percent_with_the_same_layout() {
    assert!(switch_of("clock_weekday", 1.5, &IsleConfig::default()) > 90);
}

#[test]
fn the_preview_is_the_real_bar() {
    let v = view(SettingsTab::Bar);
    let l = layout(&v, &IsleConfig::default(), &Text::new(), W, H);
    let (px, py, pw, _) = l.preview.unwrap();
    let mut r = PillRenderer::new(W as u32, H as u32);
    let fb = r.render_settings(&v, &IsleConfig::default(), &state()).clone();
    let mut empty = state();
    empty.clock_text.clear();
    let mut r2 = PillRenderer::new(W as u32, H as u32);
    let without = r2.render_settings(&v, &IsleConfig::default(), &empty).clone();
    let area = (px, py, pw, 80.);
    assert!(bright_in(&fb, area) > bright_in(&without, area) + 20, "the clock's glyphs are drawn in the preview");
}

#[test]
fn secrets_never_draw_their_text() {
    let mut a = IsleConfig::default();
    a.calendar_ics_url = Some("https://calendar.google.com/calendar/ical/segredo/basic.ics".into());
    let mut b = IsleConfig::default();
    b.calendar_ics_url = Some("https://outro.example/x.ics".into());
    let v = view(SettingsTab::Agenda);
    let fa = PillRenderer::new(W as u32, H as u32).render_settings(&v, &a, &state()).clone();
    let fb = PillRenderer::new(W as u32, H as u32).render_settings(&v, &b, &state()).clone();
    assert!(fa.bgra_premultiplied == fb.bgra_premultiplied, "two secrets look the same");
}

#[test]
fn every_tab_draws_opaque() {
    for tab in SettingsTab::ALL {
        let mut r = PillRenderer::new(820, 600);
        let fb = r.render_settings(&view(tab), &IsleConfig::default(), &state());
        assert!(fb.bgra_premultiplied.chunks_exact(4).all(|p| p[3] == 255), "{tab:?}");
    }
}

/// (r, g, b) un-premultiplied enough to compare hues, at a logical point.
fn rgb_at(fb: &PillFrameBuffer, scale: f32, x: f32, y: f32) -> (u8, u8, u8) {
    let i = (((y * scale) as u32 * fb.width + (x * scale) as u32) * 4) as usize;
    (fb.bgra_premultiplied[i + 2], fb.bgra_premultiplied[i + 1], fb.bgra_premultiplied[i])
}

#[test]
fn the_selected_tab_follows_the_chosen_accent() {
    for scale in [1.0, 1.5] {
        isle_render::set_accent((0, 200, 0));
        let v = view(SettingsTab::Bar);
        let cfg = IsleConfig::default();
        let l = layout(&v, &cfg, &Text::new(), W, H);
        let hb = l.nav.iter().find(|n| n.action == isle_render::settings::SettingsAction::Tab(SettingsTab::Bar)).unwrap();
        let mut r = PillRenderer::with_scale(W as u32, H as u32, scale);
        let fb = r.render_settings(&v, &cfg, &state());
        let (tr, tg, _) = rgb_at(fb, scale, hb.x + 3., hb.y + hb.h / 2.);
        let (lr, lg, _) = rgb_at(fb, scale, 33., 34.);
        let apricot = accent_in(fb, scale, (0., 0., W, H));
        isle_render::set_accent((241, 139, 99));
        assert!(tg > tr, "tab background is green-tinted at {scale}: {tr},{tg}");
        assert!(lg > lr, "logo stripe follows the accent at {scale}: {lr},{lg}");
        assert_eq!(apricot, 0, "no default-accent pixel left at {scale}");
    }
}
