// SPDX-License-Identifier: GPL-3.0
//! The README's demo: the island on a made-up desktop, drawn by the same renderer as the app, with
//! made-up data, the springs ticking as they do on screen and a cursor opening the pages. Nothing is
//! captured from a real screen.
//!
//! `cargo run --release -p isle-render --example demo_reel -- <out.mp4> [chapters.txt]` (needs ffmpeg
//! on PATH); `CIMORI_LANG=en-US` draws it in another language of locales/ (pt-BR otherwise).
//! `scripts/readme-media.ps1` cuts the README's clips from it.

use isle_core::config::IsleConfig;
use isle_core::spring::PillGeometry;
use isle_core::state::{IslandState, PermissionCard, PillMode, TabPage};
use isle_render::pages::UiAction;
use isle_render::renderer::PillRenderer;
use std::io::Write;
use tiny_skia::{Color, FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, Point, RadialGradient, Rect, SpreadMode, Stroke, Transform};

const W: u32 = 1920;
const H: u32 = 1080;
const FPS: f32 = 30.0;
const CANVAS_W: u32 = isle_render::window::CANVAS_MAX_WIDTH as u32;
const CANVAS_H: u32 = isle_render::window::CANVAS_MAX_HEIGHT as u32;
/// Where the island's canvas sits on the screen: centred at the top, as its window.
const OFF_X: f32 = (W - CANVAS_W) as f32 / 2.0;
/// Fri 27 Nov 2026, 21:41 in São Paulo.
const NOW_MS: i64 = 1_795_826_460_000;
const UTC_OFFSET: i64 = -3 * 3600;

fn main() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(run).unwrap().join().unwrap();
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba8(r, g, b, a)
}

/// A dark desktop in Cimori's colours: dunes of thin apricot lines under a low sun, and a taskbar.
fn wallpaper() -> Pixmap {
    let mut p = Pixmap::new(W, H).unwrap();
    let mut paint = Paint::default();
    paint.shader = LinearGradient::new(Point::from_xy(0., 0.), Point::from_xy(0., H as f32),
        vec![GradientStop::new(0., rgba(14, 14, 16, 255)), GradientStop::new(1., rgba(26, 20, 17, 255))],
        SpreadMode::Pad, Transform::identity()).unwrap();
    p.fill_rect(Rect::from_xywh(0., 0., W as f32, H as f32).unwrap(), &paint, Transform::identity(), None);
    let (sx, sy) = (1500., 430.);
    let glow = |p: &mut Pixmap, r: f32, a: u8| {
        let mut paint = Paint::default();
        paint.shader = RadialGradient::new(Point::from_xy(sx, sy), Point::from_xy(sx, sy), r,
            vec![GradientStop::new(0., rgba(242, 150, 108, a)), GradientStop::new(1., rgba(242, 150, 108, 0))],
            SpreadMode::Pad, Transform::identity()).unwrap();
        p.fill_rect(Rect::from_xywh(0., 0., W as f32, H as f32).unwrap(), &paint, Transform::identity(), None);
    };
    glow(&mut p, 1500., 70);
    glow(&mut p, 260., 90);
    let mut sun = Paint::default();
    sun.set_color(rgba(250, 196, 160, 255));
    sun.anti_alias = true;
    p.fill_path(&PathBuilder::from_circle(sx, sy, 44.).unwrap(), &sun, FillRule::Winding, Transform::identity(), None);
    // Back to front: each ridge hides the lines behind it, then gets its own line.
    for k in 0..70 {
        let f = k as f32 / 70.;
        let base = 600. + f * f * 560.;
        let ridge = |x: f32| base
            + (60. + 50. * f) * (x * 0.0021 + k as f32 * 0.21).sin()
            + 34. * (x * 0.0057 - k as f32 * 0.37).sin() * (1. - f * 0.5)
            + 14. * (x * 0.013 + k as f32 * 0.9).sin();
        let mut line = PathBuilder::new();
        line.move_to(-10., ridge(-10.));
        let mut x = -10.;
        while x <= W as f32 + 10. {
            x += 6.;
            line.line_to(x, ridge(x));
        }
        let line = line.finish().unwrap();
        let mut body = PathBuilder::new();
        body.push_path(&line);
        body.line_to(W as f32 + 10., H as f32 + 10.);
        body.line_to(-10., H as f32 + 10.);
        body.close();
        let mut fill = Paint::default();
        fill.set_color(rgba(18 + (8. * f) as u8, 15 + (5. * f) as u8, 14, 255));
        p.fill_path(&body.finish().unwrap(), &fill, FillRule::Winding, Transform::identity(), None);
        let mut ink = Paint::default();
        ink.anti_alias = true;
        ink.set_color(rgba(232, 142, 100, (40. + 120. * (1. - (f - 0.35).abs())) as u8));
        p.stroke_path(&line, &ink, &Stroke { width: 1.1 + f, ..Default::default() }, Transform::identity(), None);
    }
    // A plain taskbar: a few app tiles in the middle, nothing anyone's brand.
    let mut bar = Paint::default();
    bar.set_color(rgba(22, 22, 24, 235));
    p.fill_rect(Rect::from_xywh(0., H as f32 - 48., W as f32, 48.).unwrap(), &bar, Transform::identity(), None);
    let tiles = [(120, 120, 128), (242, 150, 108), (96, 140, 220), (90, 180, 140), (200, 120, 200), (230, 190, 90)];
    let x0 = W as f32 / 2. - tiles.len() as f32 * 22.;
    for (i, (r, g, b)) in tiles.into_iter().enumerate() {
        let mut t = Paint::default();
        t.anti_alias = true;
        t.set_color(rgba(r, g, b, 230));
        p.fill_path(&rounded(x0 + i as f32 * 44. + 8., H as f32 - 38., 28., 28., 7.), &t, FillRule::Winding, Transform::identity(), None);
    }
    p
}

fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32) -> tiny_skia::Path {
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish().unwrap()
}

/// A cover for the made-up song: the same dunes, small.
fn cover() -> std::sync::Arc<isle_core::artwork::Artwork> {
    let full = wallpaper();
    let mut p = Pixmap::new(300, 300).unwrap();
    p.draw_pixmap(-1240, -400, full.as_ref(), &tiny_skia::PixmapPaint::default(), Transform::identity(), None);
    std::sync::Arc::new(isle_core::artwork::Artwork::new(300, 300, p.data().to_vec()).unwrap())
}

fn arrow() -> tiny_skia::Path {
    let pts = [(0., 0.), (0., 19.), (4.6, 14.6), (7.8, 21.6), (10.6, 20.4), (7.6, 13.6), (13.6, 13.6)];
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0].0, pts[0].1);
    for (x, y) in &pts[1..] { pb.line_to(*x, *y); }
    pb.close();
    pb.finish().unwrap()
}

struct Reel {
    t: f32,
    geom: PillGeometry,
    state: IslandState,
    cfg: IsleConfig,
    renderer: PillRenderer,
    bg: Pixmap,
    frame: Pixmap,
    cursor: (f32, f32),
    click_at: Option<f32>,
    /// When the Pomodoro running on the Timer page reaches zero.
    pomodoro_end: Option<i64>,
    out: std::process::ChildStdin,
    chapters: Vec<(String, f32, f32)>,
}

impl Reel {
    fn now_ms(&self) -> i64 { NOW_MS + (self.t * 1000.) as i64 }

    fn draw(&mut self) {
        self.state.now_ms = self.now_ms();
        if let Some(end) = self.pomodoro_end {
            self.state.timers.pomodoro_remaining_ms = (end - self.state.now_ms).max(0);
        }
        for _ in 0..2 { self.geom.tick(0.5 / FPS); }
        self.frame.data_mut().copy_from_slice(self.bg.data());
        let fb = self.renderer.render(&self.geom, &self.state, &self.cfg);
        // The island's premultiplied BGRA over the desktop, as the compositor does.
        let data = self.frame.data_mut();
        for y in 0..fb.height as usize {
            for x in 0..fb.width as usize {
                let s = (y * fb.width as usize + x) * 4;
                let a = fb.bgra_premultiplied[s + 3] as u32;
                if a == 0 { continue; }
                let d = (y * W as usize + x + OFF_X as usize) * 4;
                for (c, from) in [(0, 2), (1, 1), (2, 0)] {
                    data[d + c] = (fb.bgra_premultiplied[s + from] as u32 + data[d + c] as u32 * (255 - a) / 255).min(255) as u8;
                }
            }
        }
        let (cx, cy) = self.cursor;
        if let Some(at) = self.click_at {
            let k = (self.t - at) / 0.35;
            if (0. ..1.).contains(&k) {
                let mut ring = Paint::default();
                ring.anti_alias = true;
                ring.set_color(rgba(255, 255, 255, (150. * (1. - k)) as u8));
                self.frame.stroke_path(&PathBuilder::from_circle(cx, cy, 6. + 16. * k).unwrap(), &ring,
                    &Stroke { width: 2., ..Default::default() }, Transform::identity(), None);
            }
        }
        let at = Transform::from_translate(cx, cy).pre_scale(1.15, 1.15);
        let mut white = Paint::default();
        white.anti_alias = true;
        white.set_color(rgba(255, 255, 255, 255));
        let mut black = white.clone();
        black.set_color(rgba(0, 0, 0, 255));
        self.frame.fill_path(&arrow(), &white, FillRule::Winding, at, None);
        self.frame.stroke_path(&arrow(), &black, &Stroke { width: 1.1, ..Default::default() }, at, None);
        self.out.write_all(self.frame.data()).expect("ffmpeg took the frame");
        self.t += 1. / FPS;
    }

    fn hold(&mut self, secs: f32) {
        for _ in 0..(secs * FPS).round() as usize { self.draw(); }
    }

    /// Eased, as a hand moves a mouse.
    fn move_to(&mut self, x: f32, y: f32, secs: f32) {
        let (x0, y0) = self.cursor;
        let n = (secs * FPS).round().max(1.) as usize;
        for i in 1..=n {
            let k = i as f32 / n as f32;
            let e = if k < 0.5 { 4. * k * k * k } else { 1. - (-2. * k + 2.).powi(3) / 2. };
            self.cursor = (x0 + (x - x0) * e, y0 + (y - y0) * e);
            self.draw();
        }
    }

    fn click(&mut self) {
        self.click_at = Some(self.t);
        self.hold(0.12);
    }

    /// The size the app gives a mode (`App::sync_tray_panel`, `fit_panels`, `show_permission`).
    fn set_mode(&mut self, mode: PillMode) {
        self.state.mode = mode.clone();
        let ui = &self.cfg.ui;
        let curve = self.cfg.notch_curve as f32 / 75.0;
        let (w, h, r) = match mode {
            PillMode::CompactBar => {
                let h = ui.pill_height_px();
                let r = if ui.pill_style == isle_core::config::PillStyle::Notch { isle_render::renderer::notch_radius(h, self.cfg.notch_curve) } else { (h * 0.5 * curve).min(h * 0.5) };
                (isle_render::pages::compact_tray_width(&self.state), h, r)
            }
            PillMode::Expanded(tab) => {
                let (aw, ah) = (W as f32 - 32., H as f32 - self.cfg.top_gap() - 16.);
                let (mut w, h, r) = isle_render::layout::fit_size(tab, aw.min(CANVAS_W as f32 - 8.), ah.min(CANVAS_H as f32 - 16.));
                if ui.uniform_panels { w = w.max(isle_render::pages::compact_tray_width(&self.state)); }
                w = w.max(isle_render::layout::header_room(&self.state).min(CANVAS_W as f32 - 8.));
                (w, h, r * curve)
            }
            PillMode::PermissionPrompt => {
                let cmd = self.state.pending_permission.as_ref().map_or("", |c| c.command.as_str()).to_owned();
                (isle_render::renderer::CARD_WIDTH, isle_render::renderer::card_height(&cmd), 22.)
            }
            PillMode::AlertBanner { .. } if self.state.notification_banner.is_some() => {
                let (w, h) = ui.banner_px();
                (w, h, 20. * ui.banner_scale())
            }
            PillMode::AlertBanner { .. } => (360., 64., 20.),
            PillMode::Minimized => (120., 5., 2.5),
        };
        self.geom.morph_to(w, h, r);
        self.geom.glow_intensity.set_target(if self.state.mode == PillMode::PermissionPrompt { 1. } else { 0. });
    }

    /// Where `want` can be clicked now, on the screen.
    fn target(&self, want: impl Fn(&UiAction) -> bool) -> Option<(f32, f32)> {
        isle_render::pages::compute_hitboxes(CANVAS_W as f32, self.geom.width.target, self.geom.height.target, self.cfg.top_gap(), &self.state)
            .into_iter().find(|b| want(&b.action))
            .map(|b| (OFF_X + b.x + b.w / 2., b.y + b.h / 2.))
    }

    fn chapter(&mut self, name: &str) { self.chapters.push((name.into(), self.t, self.t)); }
    fn end(&mut self) { if let Some(c) = self.chapters.last_mut() { c.2 = self.t; } }

    /// Opens a tab from the header as the cursor would.
    fn open_tab(&mut self, tab: TabPage) -> bool {
        let Some((x, y)) = self.target(|a| *a == UiAction::SwitchTab(tab)) else { return false };
        self.move_to(x + 2., y + 3., 0.55);
        self.click();
        self.set_mode(PillMode::Expanded(tab));
        true
    }
}

/// The made-up data in the reel's language: pt-BR, or English for any other.
fn say(pt: &'static str, en: &'static str) -> &'static str {
    if isle_core::i18n::current().code == "pt-BR" { pt } else { en }
}

fn run() {
    if let Ok(code) = std::env::var("CIMORI_LANG") {
        isle_core::i18n::set(isle_core::i18n::resolve(&code, &[]));
    }
    let out = std::env::args().nth(1).unwrap_or_else(|| "demo.mp4".into());
    let chapters_file = std::env::args().nth(2);
    let mut ffmpeg = std::process::Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{W}x{H}"), "-r", &FPS.to_string(), "-i", "-",
            "-c:v", "libx264", "-preset", "slow", "-crf", "14", "-pix_fmt", "yuv420p", "-movflags", "+faststart", &out])
        .stdin(std::process::Stdio::piped()).spawn().expect("ffmpeg on PATH");
    let mut cfg = IsleConfig::default();
    // The closed pill with the song, as it is set up most often.
    cfg.bar_items = vec![isle_core::config::BarItem::Clock, isle_core::config::BarItem::Media, isle_core::config::BarItem::Weather];
    let mut state = IslandState::default();
    state.modules = cfg.modules;
    state.ui = cfg.ui;
    state.control_tiles = cfg.control_tiles.clone();
    state.bar = cfg.bar_shown();
    state.clock_text = "21:41".into();
    state.date_text = isle_core::clock::date_text(NOW_MS / 1000 + UTC_OFFSET, &cfg);
    state.utc_offset_s = UTC_OFFSET;
    state.today = (NOW_MS / 1000 + UTC_OFFSET).div_euclid(86_400);
    state.agenda_day = state.today;
    state.mode = PillMode::CompactBar;
    state.media = Some(isle_core::state::MediaInfo {
        artwork: Some(cover()), title: "Horizonte".into(), artist: "Cimori Sessions".into(),
        playing: true, position_ms: 84_000, duration_ms: 222_000, sampled_at_ms: NOW_MS,
    });
    state.active_lyric_line = Some(say("A tarde cai devagar sobre as dunas", "The evening settles slowly on the dunes").into());
    // The control center.
    state.audio_available = true;
    state.mic_available = true;
    state.volume_pct = 72;
    state.wifi_connected = Some(true);
    state.wifi_name = Some(say("Casa · 5 GHz", "Home · 5 GHz").into());
    state.bluetooth_on = Some(true);
    state.night_light_on = Some(false);
    state.audio_outputs = [say("Fones de ouvido", "Headphones"), say("Alto-falantes", "Speakers")].iter().enumerate()
        .map(|(i, n)| isle_core::state::AudioOutput { id: i.to_string(), name: (*n).into(), is_default: i == 0 }).collect();
    state.app_volumes = [("system", say("Sons do sistema", "System sounds"), 100), ("music", say("Música", "Music"), 64), ("browser", say("Navegador", "Browser"), 80)].into_iter()
        .map(|(id, name, v)| isle_core::state::AppVolume { id: id.into(), name: name.into(), volume_pct: v, muted: false }).collect();
    // The agenda.
    let ics = format!("BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:a\r\nDTSTART:20261127T130000Z\r\nSUMMARY:{}\r\nEND:VEVENT\r\n\
        BEGIN:VEVENT\r\nUID:b\r\nDTSTART;VALUE=DATE:20261127\r\nSUMMARY:{}\r\nEND:VEVENT\r\n\
        BEGIN:VEVENT\r\nUID:c\r\nDTSTART:20261127T223000Z\r\nSUMMARY:{}\r\nEND:VEVENT\r\n\
        BEGIN:VEVENT\r\nUID:d\r\nDTSTART:20261130T170000Z\r\nSUMMARY:{}\r\nEND:VEVENT\r\nEND:VCALENDAR",
        say("Daily do time", "Team stand-up"), say("Lançamento da Cimori", "Cimori launch"),
        say("Jantar com a família", "Family dinner"), say("Revisão do projeto", "Project review"));
    state.agenda = isle_core::agenda::expand(&isle_core::agenda::parse_ics(&ics, UTC_OFFSET), state.today - 40, state.today + 400);
    state.agenda.extend(isle_core::agenda::parse_holidays(&format!(r#"[{{"date":"2026-12-25","localName":"{}"}}]"#, say("Natal", "Christmas"))));
    state.agenda.sort_by_key(|i| (i.day, i.minute.map_or(-1, |m| m as i64)));
    state.weather = isle_core::weather::Weather::from_open_meteo("São Paulo", include_str!("open-meteo-sample.json")).ok();
    state.timers = isle_core::timers::TimersView {
        focus_minutes: 25, break_minutes: 5,
        pomodoro_phase: isle_core::pomodoro::Phase::Focus, pomodoro_running: true,
        pomodoro_remaining_ms: 14 * 60_000 + 32_000, pomodoro_duration_ms: 25 * 60_000,
        countdown_remaining_ms: 10 * 60_000, countdown_duration_ms: 10 * 60_000,
        ..Default::default()
    };
    // AI usage, as the header's rings and the Uso page show it.
    let window = |label: &str, percent: f64, hours: i64| isle_core::state::UsageWindow { label: label.into(), percent, resets_at_ms: NOW_MS + hours * 3_600_000 };
    state.usage = vec![
        isle_core::state::UsageProvider { id: "claude".into(), name: "Claude".into(), plan: "Pro".into(), updated_ms: NOW_MS - 120_000,
            windows: vec![window("usage.five_hours", 42.0, 3), window("usage.week", 61.0, 70)], ..Default::default() },
        isle_core::state::UsageProvider { id: "codex".into(), name: "Codex".into(), plan: "Plus".into(), updated_ms: NOW_MS - 300_000,
            windows: vec![window("usage.five_hours", 18.0, 4), window("usage.week", 35.0, 110)], ..Default::default() },
    ];
    let mut reel = Reel {
        t: 0., geom: PillGeometry::default(), state, cfg, renderer: PillRenderer::with_scale(CANVAS_W, CANVAS_H, 1.0),
        bg: wallpaper(), frame: Pixmap::new(W, H).unwrap(), cursor: (1180., 760.), click_at: None, pomodoro_end: None,
        out: ffmpeg.stdin.take().unwrap(), chapters: Vec::new(),
    };
    reel.set_mode(PillMode::CompactBar);
    let (w, h, r) = (reel.geom.width.target, reel.geom.height.target, reel.geom.corner_radius.target);
    reel.geom = PillGeometry::default();
    reel.geom.morph_to(w, h, r);
    (reel.geom.width.value, reel.geom.height.value, reel.geom.corner_radius.value) = (w, h, r);

    // The closed pill, music playing; the mouse comes up to it and it opens on the song.
    reel.chapter("music");
    reel.hold(1.2);
    reel.move_to(OFF_X + CANVAS_W as f32 / 2. + 30., 22., 1.1);
    reel.hold(0.25);
    reel.set_mode(PillMode::Expanded(TabPage::Media));
    reel.hold(3.2);
    reel.end();

    reel.chapter("control");
    if reel.open_tab(TabPage::ControlCenter) {
        reel.hold(1.0);
        // The volume, dragged down and back a little.
        if let Some((_, y)) = reel.target(|a| matches!(a, UiAction::SetVolume(_))) {
            let hb = isle_render::pages::compute_hitboxes(CANVAS_W as f32, reel.geom.width.target, reel.geom.height.target, reel.cfg.top_gap(), &reel.state)
                .into_iter().find(|b| matches!(b.action, UiAction::SetVolume(_))).unwrap();
            let at = |v: f32| OFF_X + hb.x + hb.w * v / 100.;
            reel.move_to(at(72.), y + 2., 0.6);
            for (from, to) in [(72., 38.), (38., 55.)] {
                let n = (0.7 * FPS) as usize;
                for i in 1..=n {
                    let v = from + (to - from) * i as f32 / n as f32;
                    reel.state.volume_pct = v.round() as u8;
                    reel.cursor = (at(v), y + 2.);
                    reel.draw();
                }
            }
            reel.hold(0.6);
        }
    }
    reel.end();

    reel.chapter("agenda");
    if reel.open_tab(TabPage::CalendarWeather) { reel.hold(2.4); }
    reel.end();

    reel.chapter("weather");
    if reel.open_tab(TabPage::Weather) { reel.hold(2.2); }
    reel.end();

    reel.chapter("timer");
    reel.pomodoro_end = Some(reel.now_ms() + reel.state.timers.pomodoro_remaining_ms);
    if reel.open_tab(TabPage::Timer) { reel.hold(2.4); }
    reel.end();

    reel.chapter("usage");
    if reel.open_tab(TabPage::AiHub) { reel.hold(2.6); }
    reel.end();

    // The mascot opens the chat: a second click on Claude picks the next model, a request is typed, the
    // answer streams in and the command waits for Executar.
    reel.chapter("chat");
    if let Some((x, y)) = reel.target(|a| *a == UiAction::AiOpen) {
        use isle_core::state::AiProviderKind as P;
        reel.move_to(x, y + 3., 0.6);
        reel.click();
        reel.state.chat.open = true;
        reel.state.chat.provider = P::ClaudeCode;
        reel.state.chat.model_label = "Sonnet".into();
        reel.set_mode(PillMode::Expanded(TabPage::AiHub));
        reel.hold(1.1);
        if let Some((x, y)) = reel.target(|a| *a == UiAction::AiProvider(P::ClaudeCode)) {
            reel.move_to(x, y + 3., 0.6);
            reel.click();
            reel.state.chat.model_label = "Opus".into();
            reel.hold(0.9);
        }
        if let Some((x, y)) = reel.target(|a| *a == UiAction::AiInput) {
            reel.move_to(x - 140., y + 3., 0.6);
            reel.click();
        }
        let ask = say("Instala o Node para mim", "Install Node for me");
        for i in 1..=ask.chars().count() {
            reel.state.chat.input.text = ask.chars().take(i).collect();
            reel.state.chat.caret = Some(true);
            reel.hold(1. / 15.);
        }
        reel.hold(0.4);
        if let Some((x, y)) = reel.target(|a| *a == UiAction::AiSend) {
            reel.move_to(x, y + 3., 0.5);
            reel.click();
        }
        reel.state.chat.items.push(isle_core::chat::Message { user: true, text: ask.into() });
        reel.state.chat.input.text.clear();
        reel.state.chat.caret = None;
        reel.state.chat.busy = true;
        reel.state.chat.items.push(isle_core::chat::Message { user: false, text: String::new() });
        reel.hold(0.6);
        let answer = say("✓ Ver o que está instalado\nO Node não está instalado. Vou instalar o nvm-windows pelo winget, e com ele a versão LTS do Node.",
            "✓ Check what's installed\nNode isn't installed. I'll install nvm-windows with winget, and the LTS version of Node with it.");
        for i in (0..=answer.chars().count()).step_by(3) {
            reel.state.chat.items.last_mut().unwrap().text = answer.chars().take(i).collect();
            reel.draw();
        }
        reel.state.chat.items.last_mut().unwrap().text = answer.into();
        reel.state.chat.approval = Some(isle_core::chat::Approval {
            title: say("Instalar o nvm-windows", "Install nvm-windows").into(),
            command: "winget install -e --id CoreyButler.NVMforWindows".into(),
        });
        reel.hold(1.4);
        if let Some((x, y)) = reel.target(|a| *a == UiAction::AiRun) {
            reel.move_to(x, y + 3., 0.8);
            reel.click();
        }
        reel.state.chat.approval = None;
        reel.hold(0.8);
        let done = say("\n✓ Instalar o nvm-windows\nPronto: o nvm-windows está instalado. Abra um terminal novo e rode nvm install lts.",
            "\n✓ Install nvm-windows\nDone: nvm-windows is installed. Open a new terminal and run nvm install lts.");
        let base = reel.state.chat.items.last().unwrap().text.clone();
        for i in (0..=done.chars().count()).step_by(3) {
            reel.state.chat.items.last_mut().unwrap().text = format!("{base}{}", done.chars().take(i).collect::<String>());
            reel.draw();
        }
        reel.state.chat.items.last_mut().unwrap().text = format!("{base}{done}");
        reel.state.chat.busy = false;
        reel.hold(2.4);
    }
    reel.end();

    // The mouse leaves: it closes back to the pill.
    reel.move_to(1250., 640., 0.9);
    reel.set_mode(PillMode::CompactBar);
    reel.hold(1.2);

    // Claude Code asks; the island grows into the card, and Permitir answers it.
    reel.chapter("agents");
    reel.state.pending_permission = Some(PermissionCard {
        request_id: "demo".into(),
        agent: "Claude Code · cimori".into(),
        tool_name: "Bash".into(),
        command: "cargo test --workspace".into(),
        ..Default::default()
    });
    reel.set_mode(PillMode::PermissionPrompt);
    reel.hold(1.4);
    if let Some((x, y)) = reel.target(|a| matches!(a, UiAction::Permission(isle_core::state::PermissionDecision::Allow, _))) {
        reel.move_to(x, y + 2., 0.9);
        reel.click();
    }
    reel.state.pending_permission = None;
    reel.set_mode(PillMode::CompactBar);
    reel.hold(1.6);
    reel.set_mode(PillMode::AlertBanner { title: format!("{} · cimori", isle_core::i18n::trf("agent.done", &[("name", &"Claude Code")])), subtitle: say("Os 412 testes passaram.", "All 412 tests passed.").into() });
    reel.hold(2.6);
    reel.set_mode(PillMode::CompactBar);
    reel.hold(0.8);
    reel.end();

    // A message drops out of the pill, then the bell keeps it.
    reel.chapter("notifications");
    reel.move_to(1300., 700., 0.6);
    let note = isle_core::notifications::Notification {
        id: 1, app_id: say("Mensagens", "Messages").into(), app_name: say("Mensagens", "Messages").into(), title: "Ana".into(),
        body: say("Chego em 10 minutos, pode descer!", "Be there in 10 minutes, come on down!").into(), arrival_ms: reel.now_ms(), unread: true, ..Default::default()
    };
    reel.state.notifications.receive(vec![note.clone()], false);
    reel.state.notification_banner = Some(note);
    reel.set_mode(PillMode::AlertBanner { title: format!("{} · Ana", say("Mensagens", "Messages")), subtitle: say("Chego em 10 minutos, pode descer!", "Be there in 10 minutes, come on down!").into() });
    reel.hold(3.0);
    reel.state.notification_banner = None;
    reel.set_mode(PillMode::CompactBar);
    reel.hold(1.4);
    reel.end();

    drop(reel.out);
    let status = ffmpeg.wait().expect("ffmpeg ran");
    assert!(status.success(), "ffmpeg failed");
    settings_tour(std::path::Path::new(&out).parent().unwrap_or(std::path::Path::new(".")), &reel.cfg, &reel.state);
    let list: String = reel.chapters.iter().map(|(n, a, b)| format!("{n} {a:.2} {b:.2}\n")).collect();
    match chapters_file { Some(f) => std::fs::write(f, list).expect("write chapters"), None => print!("{list}") }
    println!("{out}: {:.1} s", reel.t);
}

/// The settings window, a few of its pages: `settings-01.png`, `settings-02.png`… next to the video.
fn settings_tour(dir: &std::path::Path, cfg: &IsleConfig, state: &IslandState) {
    use isle_core::prefs::SettingsTab as T;
    let place = |d: &str, left| isle_render::window::MonitorPlace { device: d.into(), left, top: 0, width: 1920, scale: 1.0, primary: left == 0 };
    for (i, tab) in [T::General, T::Look, T::Bar, T::Ai, T::Monitors, T::Hiding, T::Media, T::Updates].into_iter().enumerate() {
        let mut view = isle_render::settings::SettingsView { tab, ..Default::default() };
        view.info.extend([("info.version", isle_core::i18n::trf("settings.version", &[("version", &"1.0.4")])), ("info.hooks", isle_core::i18n::tr("settings.hooks_installed").into()), ("info.bridge", isle_core::i18n::tr("settings.bridge_on").into())]);
        view.monitors = vec![place(r"\\.\DISPLAY1", 0), place(r"\\.\DISPLAY2", 1920)];
        let mut renderer = PillRenderer::with_scale(1180, 760, 1.0);
        let fb = renderer.render_settings(&view, cfg, state);
        let rgba: Vec<u8> = fb.bgra_premultiplied.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], 255]).collect();
        let pixmap = Pixmap::from_vec(rgba, tiny_skia::IntSize::from_wh(fb.width, fb.height).unwrap()).unwrap();
        pixmap.save_png(dir.join(format!("settings-{:02}.png", i + 1))).expect("write settings png");
    }
}
