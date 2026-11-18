//! Headless frame timing: `cargo run --release -p isle-render --example frame_bench`.
//! Renders the expand, tab switch, collapse and compact frames at 100% and 150% and prints per-frame
//! raster times and how many pixels each frame changed (what `present` copies and hands to the screen).
//! The first frame of each run is cold (icons and glyphs drawn for the first time).
use isle_core::config::IsleConfig;
use isle_core::spring::PillGeometry;
use isle_core::state::{IslandState, MediaInfo, PillMode, TabPage};
use isle_render::renderer::PillRenderer;
use std::time::Instant;

fn run(name: &str, scale: f32, state: &IslandState, cfg: &IsleConfig, from: (f32, f32, f32), to: (f32, f32, f32)) {
    let mut r = PillRenderer::with_scale(720, 520, scale);
    let mut g = PillGeometry::default();
    g.width.value = from.0; g.height.value = from.1; g.corner_radius.value = from.2;
    g.morph_to(to.0, to.1, to.2);
    let mut times = vec![];
    let mut sink = 0u64;
    for _ in 0..90 {
        g.tick(1. / 60.);
        let t = Instant::now();
        let fb = r.render(&g, state, cfg);
        let (x0, y0, x1, y1) = fb.dirty;
        sink += ((x1 - x0) * (y1 - y0)) as u64;
        times.push(t.elapsed().as_secs_f64() * 1000.);
        if g.is_settled() && times.len() > 30 { break }
    }
    let n = times.len() as f64;
    let sink = sink / times.len() as u64;
    let mean = times.iter().sum::<f64>() / n;
    let max = times.iter().cloned().fold(0., f64::max);
    let over = times.iter().filter(|&&t| t > 16.6).count();
    println!("{name:<28} {:>4.0}%  frames {:>3}  mean {mean:>6.2} ms  max {max:>6.2} ms  >16.6ms {over}  dirty {sink} px/frame", scale * 100., times.len());
}

fn main() {
    let cfg = IsleConfig::default();
    let mut glass = cfg.clone(); glass.liquid_glass = true;
    let compact = (220., 34., 17.);
    for scale in [1.0, 1.5] {
        let mut s = IslandState::default();
        s.clock_text = "21:30".into();
        s.media = Some(MediaInfo { artwork: None, title: "A rather long song title that must scroll".into(), artist: "Artist".into(),
            playing: true, position_ms: 60_000, duration_ms: 180_000, sampled_at_ms: 0 });
        for tab in [TabPage::Media, TabPage::AiHub, TabPage::ControlCenter, TabPage::Notifications, TabPage::CalendarWeather] {
            s.mode = PillMode::Expanded(tab);
            let to = isle_render::layout::expanded_size(tab);
            run(&format!("expand {tab:?}"), scale, &s, &cfg, compact, to);
        }
        s.mode = PillMode::Expanded(TabPage::Weather);
        run("tab Media->Weather", scale, &s, &cfg, isle_render::layout::expanded_size(TabPage::Media), isle_render::layout::expanded_size(TabPage::Weather));
        s.mode = PillMode::Expanded(TabPage::Media);
        run("expand Media (glass)", scale, &s, &glass, compact, isle_render::layout::expanded_size(TabPage::Media));
        s.mode = PillMode::CompactBar;
        run("collapse to compact", scale, &s, &cfg, (560., 380., 28.), compact);
        run("compact settled (marquee)", scale, &s, &cfg, compact, compact);
    }
}
