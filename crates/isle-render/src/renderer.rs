#[path = "phase8_view.rs"]
pub mod phase8_view;
// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Adapted for isle-win from Mac Island & Dock by caliberda v3.1.0 (ControlPanel::DrawTile/DrawMain):
// tiny-skia controls and NotificationPanel using isle palette and Inter.
// TrayListPanel/pinned icons adapted from Mac Island & Dock v3.1.1.

use crate::pages::{MediaKey, UiAction, agenda_grid, agenda_selected, compute_hitboxes};
use crate::text::{Text, Weight};
use isle_core::agenda::{civil_from_days, weekday};
use isle_core::config::IsleConfig;
use isle_core::spring::PillGeometry;
use isle_core::state::{AppearanceMode, ControlTile, IslandState, PermissionDecision, PillMode, TabPage,
};
use tiny_skia::{
    Color, FillRule, FilterQuality, Mask, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform,
};

/// Around the hidden line, how far the mouse is taken: sideways, and below it (it is taken from the
/// screen's very top edge down). Kept in sync with the window's hit area.
pub const LINE_REACH_X: f32 = 20.0;
pub const LINE_REACH_Y: f32 = 6.0;
/// How far past the pill a frame draws (shadow spread 10 + 3 down, glow 2, the line's reach 20).
const DRAWN_MARGIN: f32 = 24.0;

#[derive(Clone)]
pub struct PillFrameBuffer {
    pub width: u32,
    pub height: u32,
    pub bgra_premultiplied: Vec<u8>,
    /// What this frame changed (x0, y0, x1, y1, px): the rest is as the frame before left it.
    pub dirty: (usize, usize, usize, usize),
}

use crate::theme::*;
#[path = "avatar.rs"]
mod avatar;
#[path = "icons.rs"]
mod icons;
#[path = "glass_view.rs"]
mod glass_view;
#[path = "media_art.rs"]
mod media_art;
#[path = "notification_view.rs"]
mod notification_view;
#[path="viewport.rs"]
mod viewport;
#[path = "polish.rs"]
mod polish;
#[path = "provider_logos.rs"]
mod provider_logos;
use avatar::Mood;
use icons::Icon;

/// Text sizes, as the original's CSS.
const CLOCK: f32 = 15.0;
const TITLE: f32 = 13.5;
const BODY: f32 = 13.0;
const SMALL: f32 = 12.0;
const PAD: f32 = 18.0;

/// Draws the pill in logical (100%) pixels, rasterized at `scale` (the monitor's DPI / 96) so text and
/// curves are drawn sharp at 125%, 150%… instead of stretched.
pub struct PillRenderer {
    glass_active: bool,
    /// The glass light's alpha over the pill's bounds, for the geometry in the key.
    glass_optics: Option<(Vec<u32>,Vec<u8>)>,
    icon_cache: crate::icon_images::IconCache,
    scale: f32,
    pixmap: Pixmap,
    /// Body and content, clipped to the pill before going onto `pixmap` (the glow stays outside).
    layer: Pixmap,
    /// The pill's shape (kept between frames, cleared each one): what of `layer` shows.
    clip: Mask,
    /// The device pixels the last frame drew (x0, y0, x1, y1): only they and this frame's are composited.
    drawn: (usize, usize, usize, usize),
    /// The pill's body, under `layer` (None: nothing under it).
    body: Option<Body>,
    text: Text,
    tray_images: std::collections::HashMap<String, (Vec<u8>, Option<Pixmap>)>,
    framebuffer: PillFrameBuffer,
    media_image: Option<(std::sync::Arc<isle_core::artwork::Artwork>, Pixmap)>,
    notification_images: std::collections::HashMap<String, (std::sync::Arc<isle_core::artwork::Artwork>, u32, Pixmap)>,
    #[cfg(windows)]
    notification_text: Option<crate::notification_text::NotificationText>,
    pub(crate) pointer: Option<(f32, f32)>,
    pub(crate) pressed: bool,
}

pub(crate) fn build_rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    let r = r.min(w * 0.5).min(h * 0.5).max(0.0);
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
    pb.finish()
}

/// Port of NotchGeometry (mac-island 3.1.1): a body `w` x `h` hanging from the canvas' top edge at
/// `x`, round corners `r` under it, and beside each side a curve the other way (`sh` along the edge,
/// as deep as the side allows) where it meets the edge. `closed` runs back along the edge (the fill);
/// open, it is the outline away from the edge (the faint line drawn round it).
pub(crate) fn build_notch(x: f32, w: f32, h: f32, r: f32, sh: f32, closed: bool) -> Option<tiny_skia::Path> {
    let r = r.clamp(0.0, (w / 2.0).min(h).max(0.0));
    let s = sh.max(0.0);
    let d = s.min((h - r).max(0.0));
    let (left, right) = (x, x + w);
    let mut pb = PathBuilder::new();
    pb.move_to(left - s, 0.0);
    if s > 0.01 && d > 0.01 { pb.quad_to(left, 0.0, left, d); } else { pb.line_to(left, 0.0); }
    pb.line_to(left, h - r);
    pb.quad_to(left, h, left + r, h);
    pb.line_to(right - r, h);
    pb.quad_to(right, h, right, h - r);
    pb.line_to(right, d);
    if s > 0.01 && d > 0.01 { pb.quad_to(right, 0.0, right + s, 0.0); } else { pb.line_to(right, 0.0); }
    if closed {
        pb.close();
    }
    pb.finish()
}

/// Port of NotchCurveAmount/NotchRadius/NotchShoulder (mac-island 3.1.1): the notch's corner and
/// shoulder for a body `h` tall, from the "Arredondamento" setting (75 % by default).
pub fn notch_radius(h: f32, curve: u32) -> f32 {
    (h / 4.0 * curve as f32 / 100.0 * 1.5).round()
}
pub fn notch_shoulder(radius: f32, curve: u32) -> f32 {
    // NotchShoulder is NotchRadius times the curve's amount; isle's open notch keeps 22 px at most.
    (radius * curve as f32 / 100.0 * 1.5).min(22.0)
}

fn quota_bar_color(pct: f32) -> Color {
    if pct >= 95.0 {
        red()
    } else if pct >= 80.0 {
        orange()
    } else {
        accent()
    }
}

/// The top that centres a line of `size` text in a box from `y`, `h` tall.
fn centered_top(y: f32, h: f32, size: f32) -> f32 {
    y + (h - size * 1.21) * 0.5
}

/// "renova em 2h 13m" until a limit starts over; "" when unknown.
pub fn resets_in(resets_at_ms: i64, now_ms: i64) -> String {
    if resets_at_ms <= 0 {
        return String::new();
    }
    let left = resets_at_ms - now_ms;
    if left <= 0 {
        return "renovando".into();
    }
    let minutes = (left + 59_999) / 60_000;
    let (d, h, m) = (minutes / 1440, minutes % 1440 / 60, minutes % 60);
    match (d, h) {
        (0, 0) => format!("renova em {m}m"),
        (0, _) => format!("renova em {h}h {m}m"),
        _ => format!("renova em {d}d {h}h"),
    }
}

/// "3:07" for a song's position or length.
fn mm_ss(ms: i64) -> String {
    let seconds = ms.max(0) / 1000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// The permission card: its width, where the command starts, a line's height, and the most lines it shows.
pub const CARD_WIDTH: f32 = 440.0;
pub const CARD_COMMAND_TOP: f32 = 30.0;
pub const CARD_LINE: f32 = 18.0;
pub const CARD_MAX_LINES: usize = 7;

fn command_lines(command: &str) -> Vec<String> {
    Text::new().wrap_exact(command, CARD_WIDTH - PAD * 2.0, BODY, Weight::Regular)
}

/// Whether the card can show `command` whole. When it can't, the island must not ask about it: what is
/// approved has to be what was seen, so the AI asks in its terminal instead.
pub fn command_fits(command: &str) -> bool {
    command_lines(command).len() <= CARD_MAX_LINES
}

/// How tall the card is for `command`: every line of it, then the buttons.
pub fn card_height(command: &str) -> f32 {
    let lines = command_lines(command).len().clamp(1, CARD_MAX_LINES) as f32;
    (CARD_COMMAND_TOP + lines * CARD_LINE + 56.0).max(120.0)
}

#[path = "timer_view.rs"]
mod timer_view;
#[path = "weather_view.rs"]
mod weather_view;
#[path = "settings_view.rs"]
mod settings_view;


/// The pill's body: a linear gradient (straight colors, one alpha) from `from` at the line's start
/// (x, y) to `to` at its end (x + dx, y + dy), padded, as tiny-skia's LinearGradient fills it.
#[derive(Clone, Copy)]
struct Body {
    from: [u8; 4],
    to: [u8; 4],
    line: (f32, f32, f32, f32),
}

impl Body {
    /// The premultiplied RGBA at 256 steps along the line (its colors are a few levels apart).
    fn ramp(&self) -> Vec<[u32; 4]> {
        let a = self.from[3] as f32 / 255.;
        (0..256).map(|i| {
            let t = i as f32 / 255.;
            let c = |c: usize| ((self.from[c] as f32 + (self.to[c] as f32 - self.from[c] as f32) * t) * a + 0.5) as u32;
            [c(0), c(1), c(2), self.from[3] as u32]
        }).collect()
    }

    /// The ramp's step at pixel (0, py), and how much it moves per pixel to the right.
    fn row(&self, py: usize) -> (f32, f32) {
        let (x, y, dx, dy) = self.line;
        let len = dx * dx + dy * dy;
        if len <= 0. { return (0., 0.); }
        (((0.5 - x) * dx + (py as f32 + 0.5 - y) * dy) / len * 255., dx / len * 255.)
    }
}

fn fill_round(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.set_color(color);
    if let Some(path) = build_rounded_rect(x, y, w, h, r) {
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None,
        );
    }
}

impl PillRenderer {
    pub fn new(width: u32, height: u32) -> Self {
        Self::with_scale(width, height, 1.0)
    }

    /// A canvas `width` x `height` logical pixels on a screen scaled by `scale`.
    pub fn with_scale(width: u32, height: u32, scale: f32) -> Self {
        let scale = scale.clamp(0.5, 4.0);
        let (width, height) = ((width as f32 * scale).round() as u32, (height as f32 * scale).round() as u32,
        );
        Self {
            scale,
            icon_cache: Default::default(),
            glass_active: false,
            glass_optics: None,
            pixmap: Pixmap::new(width, height).expect("valid pixmap dimensions"),
            layer: Pixmap::new(width, height).expect("valid pixmap dimensions"),
            clip: Mask::new(width, height).expect("valid mask dimensions"),
            drawn: (0, 0, width as usize, height as usize),
            body: None,
            text: Text::new(),
            tray_images: Default::default(),
            media_image: None,
            notification_images: Default::default(),
            #[cfg(windows)]
            notification_text: crate::notification_text::NotificationText::new().ok(),
            pointer: None,
            pressed: false,
            framebuffer: PillFrameBuffer {
                width,
                height,
                bgra_premultiplied: vec![0; (width * height * 4) as usize],
                dirty: (0, 0, width as usize, height as usize),
            },
        }
    }

    pub fn render(
        &mut self,
        geom: &PillGeometry,
        state: &IslandState,
        cfg: &IsleConfig,
    ) -> &PillFrameBuffer {
        self.icon_cache.prepare(cfg);
        self.glass_active = cfg.liquid_glass && state.mode != PillMode::Minimized;
        self.tray_images.retain(|key,_| state.tray.iter().any(|i| &i.key == key));
        for item in &state.tray {
            if self.tray_images.get(&item.key).is_none_or(|(png,_)| png != &item.png) {
                let image = if item.png.len() >= 24 && item.png.len() <= 1024*1024 {
                    let width = u32::from_be_bytes(item.png[16..20].try_into().unwrap());
                    let height = u32::from_be_bytes(item.png[20..24].try_into().unwrap());
                    if (1..=256).contains(&width) && (1..=256).contains(&height) { Pixmap::decode_png(&item.png).ok() } else { None }
                } else { None };
                self.tray_images.insert(item.key.clone(),(item.png.clone(),image));
            }
        }
        self.pixmap.fill(Color::TRANSPARENT);
        self.layer.fill(Color::TRANSPARENT);
        self.clip.clear();
        self.body = None;

        let s = self.scale;
        let canvas_w = self.framebuffer.width as f32 / s;
        let canvas_h = self.framebuffer.height as f32 / s;
        let w = geom.width.value.clamp(4.0, canvas_w - 8.0);
        let h = geom.height.value.clamp(2.0, canvas_h - 12.0);
        let r = geom.corner_radius.value;
        let x = (canvas_w - w) * 0.5;
        let y = cfg.top_gap();
        let notch = cfg.ui.pill_style == isle_core::config::PillStyle::Notch;
        // The line keeps no curves: they grow out of it as it opens (NotchShoulderWidth x open).
        let shoulder = if notch && state.mode != PillMode::Minimized { notch_shoulder(r, cfg.notch_curve) } else { 0.0 };
        // Where this frame can draw: the pill, its glow, shadow and the line's reach (all within
        // DRAWN_MARGIN of it), from the screen's top edge. With the last frame's, the only pixels composited.
        let (fw, fh) = (self.framebuffer.width as usize, self.framebuffer.height as usize);
        let px = |v: f32, max: usize| ((v * s).max(0.) as usize).min(max);
        let now = (px(x - DRAWN_MARGIN, fw), 0, (px(x + w + DRAWN_MARGIN, fw) + 1).min(fw), (px(y + h + DRAWN_MARGIN, fh) + 1).min(fh));
        let last = std::mem::replace(&mut self.drawn, now);
        let area = (now.0.min(last.0), 0, now.2.max(last.2), now.3.max(last.3));

        let built = crate::glass::glass_path(canvas_w,w,h,r,y,notch,if state.mode==PillMode::Minimized {0}else{cfg.notch_curve},s);
        let Some(pill_path) = built else {
            return self.finish_in(area);
        };

        // 0. Hidden to a line: an unseen strip from the screen's top edge takes the mouse (a layered
        //    window lets clicks and the mouse through wherever its pixels are fully transparent).
        if state.mode == PillMode::Minimized {
            let mut reach = Paint::default();
            reach.set_color_rgba8(0, 0, 0, 1);
            let reach_w = w + LINE_REACH_X * 2.0;
            if let Some(rect) = Rect::from_xywh((x - LINE_REACH_X) * s, 0.0, reach_w * s, (y + h + LINE_REACH_Y) * s,
            ) {
                self.pixmap.fill_rect(rect, &reach, Transform::identity(), None);
            }
        }

        if state.mode == PillMode::Minimized && state.line_hidden {
            return self.finish_in(area);
        }

        // The notch's line leaves out the screen's edge (NotchGeometry, not filled).
        let outline = if notch { build_notch(x * s, w * s, h * s, r * s, shoulder * s, false) } else { None };
        // 1. Glow, outside the pill, when an AI asks or finished.
        let glow = if state.mode == PillMode::PermissionPrompt {
            1.0
        } else {
            geom.glow_intensity.value
        };
        if glow > 0.01 {
            let mut glow_paint = Paint::default();
            glow_paint.anti_alias = true;
            glow_paint.set_color(Color::from_rgba8(
                241,
                139,
                99,
                (glow * 140.0).clamp(0.0, 255.0) as u8,
            ));
            let stroke = Stroke { width: 4.0 * s, ..Default::default() };
            // The notch glows along its outline only, never along the screen's edge it hangs from.
            self.pixmap.stroke_path(outline.as_ref().unwrap_or(&pill_path), &glow_paint, &stroke, Transform::identity(), None,
            );
        }

        if state.mode != PillMode::Minimized {
            // Three nested shadows (spread 10, 6, 3; alpha 3, 5, 8), each painted once: the outer two as
            // rings, the inner with all three's alpha. Three full fills of the panel cost ~6 ms at 150%.
            // Under the black body (alpha 252) the inner one shows nowhere but at the pill's edge.
            let round = |spread: f32| build_rounded_rect((x - spread) * s, (y + 3.) * s, (w + spread * 2.) * s, (h + spread) * s, (r + spread) * s);
            let opaque = !self.glass_active && state.appearance == AppearanceMode::Black;
            let inside = || build_rounded_rect((x + 2.) * s, (y + 5.) * s, (w - 4.) * s, (h - 7.) * s, (r - 2.).max(0.) * s);
            let mut paint = Paint { anti_alias: true, ..Paint::default() };
            for (outer, inner, alpha) in [(10., Some(6.), 3), (6., Some(3.), 8), (3., None, 16)] {
                let mut pb = PathBuilder::new();
                let Some(path) = round(outer) else { continue };
                pb.push_path(&path);
                let hole = match inner { Some(spread) => round(spread), None if opaque => inside(), None => None };
                if let Some(hole) = hole { pb.push_path(&hole); }
                paint.set_color(Color::from_rgba8(0, 0, 0, alpha));
                if let Some(path) = pb.finish() {
                    self.pixmap.fill_path(&path, &paint, FillRule::EvenOdd, Transform::identity(), None);
                }
            }
        }

        // 2. Body (#1C1C1E from the Windhawk backup by default) and a faint edge.
        let (br, bg, bb) = cfg.background_rgb;
        let alpha = match state.appearance {
            AppearanceMode::Black => 252,
            AppearanceMode::Glass { opacity } => (opacity.clamp(0.2, 0.95) * 255.0) as u8,
        };
        // Filled under the layer as it is composited (finish_in): the same as tiny-skia's gradient fill
        // under the content, without its pass over the panel's ~0.6 M pixels at 150% (~5 ms a frame).
        self.body = Some(if self.glass_active {
            Body { from: [8, 10, 13, 56], to: [8, 10, 13, 56], line: (0., 0., 0., 0.) }
        } else if state.mode == PillMode::Minimized {
            Body { from: [br, bg, bb, alpha], to: [br, bg, bb, alpha], line: (0., 0., 0., 0.) }
        } else {
            Body { from: [br.saturating_add(7), bg.saturating_add(9), bb.saturating_add(13), alpha], to: [br, bg, bb, alpha],
                line: (x * s, y * s, w * s, h * s) }
        });
        if self.glass_active { self.glass_light(&pill_path,x,y,w,h,r,notch,shoulder); }
        let mut edge = Paint::default();
        edge.anti_alias = true;
        edge.set_color(Color::from_rgba8(255, 255, 255, 24));
        let edge_stroke = Stroke { width: 1.0 * s, ..Default::default() };
        if !self.glass_active { self.layer.stroke_path(outline.as_ref().unwrap_or(&pill_path), &edge, &edge_stroke, Transform::identity(), None); }

        // 3. What the mode shows, laid out for the size the spring is heading to; the pill reveals it
        //    as it grows, so nothing jumps around mid-animation.
        let lw = geom.width.target.clamp(4.0, canvas_w - 8.0);
        let lh = geom.height.target.clamp(2.0, canvas_h - 12.0);
        let lx = (canvas_w - lw) * 0.5;
        match &state.mode {
            PillMode::Minimized => {
                self.draw_download_meter(lx, y, lw, state);
                if state.ui.timer_on_line && state.modules.timer {
                    if let Some(text) = &state.timer_text {
                        self.centered_label(text, lx + lw * 0.5, centered_top(y, lh, 10.), 10., Weight::Medium, accent());
                    }
                }
            }
            PillMode::CompactBar => self.draw_compact(lx, y, lw, lh, state),
            PillMode::AlertBanner { title, subtitle } => {
                // Port of BannerScale (mac-island 3.1.1): the contents grow with the banner's size,
                // drawn at the screen's scale times it, so they stay sharp.
                let k = state.ui.banner_scale();
                let screen = self.scale;
                self.scale = screen * k;
                let (lx, y, lw, lh) = (lx / k, y / k, lw / k, lh / k);
                if let Some(n) = &state.notification_banner {
                    let center = y + (lh - 64.) * 0.5;
                    self.notification_icon(n,lx+12.,center+14.,32.);
                    self.notification_label(title,lx+56.,center+12.,lw-68.,TITLE,Weight::Semibold,white());
                    self.notification_label(subtitle,lx+56.,center+33.,lw-68.,SMALL,Weight::Regular,muted());
                } else {
                    self.draw_banner(lx, y, lw, title, subtitle)
                }
                self.scale = screen;
            }
            PillMode::PermissionPrompt => self.draw_card(lx, y, lw, lh, canvas_w, state),
            PillMode::Expanded(tab) => {
                self.draw_expanded(lx, y, lw, lh, canvas_w, *tab, state, cfg);
                // Port of Island::DragOverAt's bubble (mac-island 3.1.1), over the open panel.
                if state.drop_hint {
                    let text = "Solte para pôr na prateleira";
                    let tw = self.text.width(text, 12., Weight::Semibold) + 32.;
                    self.round(lx + (lw - tw) * 0.5, y + lh - 46., tw, 30., 15., accent());
                    self.centered_label(text, lx + lw * 0.5, y + lh - 38., 12., Weight::Semibold, white());
                }
            }
        }

        self.hover_feedback(canvas_w, lw, lh, y, state, cfg);
        // 4. Content stays inside the pill, also while the spring is still growing it.
        self.clip.fill_path(&pill_path, FillRule::Winding, true, Transform::identity());
        self.finish_in(area)
    }

    /// In `area` only (device px): `layer`, clipped to the pill, over `pixmap`, as the premultiplied
    /// BGRA UpdateLayeredWindow takes. One pass instead of tiny-skia's full-canvas mask and draw: those
    /// cost ~7 ms a frame at 150%, most of an animation frame's budget. Outside `area` the frame stays clear.
    fn finish_in(&mut self, area: (usize, usize, usize, usize)) -> &PillFrameBuffer {
        self.framebuffer.dirty = area;
        let (x0, y0, x1, y1) = area;
        let width = self.framebuffer.width as usize;
        let (base, layer, clip, body) = (self.pixmap.data(), self.layer.data(), self.clip.data(), self.body);
        let ramp = body.map(|b| b.ramp());
        let div255 = |v: u32| (v + 128 + ((v + 128) >> 8)) >> 8;
        for y in y0..y1 {
            let (from, to) = (y * width + x0, y * width + x1);
            let (mut t, step) = body.map_or((0., 0.), |b| b.row(y));
            t += step * x0 as f32;
            let pixels = self.framebuffer.bgra_premultiplied[from * 4..to * 4].chunks_exact_mut(4)
                .zip(base[from * 4..to * 4].chunks_exact(4)).zip(layer[from * 4..to * 4].chunks_exact(4)).zip(&clip[from..to]);
            for (((dst, b), l), &m) in pixels {
                let under = ramp.as_ref().map(|ramp| ramp[t.clamp(0., 255.) as usize]);
                t += step;
                let m = m as u32;
                if m == 0 || (l[3] == 0 && under.is_none()) {
                    dst.copy_from_slice(&[b[2], b[1], b[0], b[3]]);
                    continue;
                }
                if let (255, 0, Some(u)) = (m, l[3], under) {
                    // Inside the pill, no content: the body alone over the shadow.
                    let keep = 255 - u[3];
                    let mix = |c: usize| (u[c] + div255(b[c] as u32 * keep)).min(255) as u8;
                    dst.copy_from_slice(&[mix(2), mix(1), mix(0), mix(3)]);
                    continue;
                }
                // The body, filled with the pill's coverage, under the layer; both then clipped by it.
                let cover = |c: u32| if m == 255 { c } else { div255(c * m) };
                let mut l = [l[0] as u32, l[1] as u32, l[2] as u32, l[3] as u32];
                if let Some(under) = under {
                    let keep = 255 - l[3];
                    l = [0, 1, 2, 3].map(|c| (l[c] + div255(cover(under[c]) * keep)).min(255));
                }
                let top = l.map(cover);
                let keep = 255 - top[3];
                let mix = |c: usize| (top[c] + div255(b[c] as u32 * keep)).min(255) as u8;
                dst.copy_from_slice(&[mix(2), mix(1), mix(0), mix(3)]);
            }
        }
        &self.framebuffer
    }

    /// Converts tiny-skia's premultiplied RGBA to the premultiplied BGRA UpdateLayeredWindow takes.
    fn finish(&mut self) -> &PillFrameBuffer {
        self.framebuffer.dirty = (0, 0, self.framebuffer.width as usize, self.framebuffer.height as usize);
        let src = self.pixmap.data();
        for (dst, px) in self.framebuffer.bgra_premultiplied.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
            dst[0] = px[2];
            dst[1] = px[1];
            dst[2] = px[0];
            dst[3] = px[3];
        }
        &self.framebuffer
    }

    /// `text` at logical `x`, `top`, `size`, rasterized at the screen's scale; returns its logical width.
    #[allow(clippy::too_many_arguments)]
    fn label(&mut self, text: &str, x: f32, top: f32, size: f32, weight: Weight, color: Color,
    ) -> f32 {
        let s = self.scale;
        self.text.draw(&mut self.layer, text, x * s, top * s, size * s, weight, color,
        ) / s
    }

    /// A filled polygon in logical coordinates, at the screen's scale.
    fn polygon(&mut self, points: &[(f32, f32)], color: Color) {
        let s = self.scale;
        let mut pb = PathBuilder::new();
        for (i, &(px, py)) in points.iter().enumerate() {
            if i == 0 {
                pb.move_to(px * s, py * s);
            } else {
                pb.line_to(px * s, py * s);
            }
        }
        pb.close();
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.anti_alias = true;
            paint.set_color(color);
            self.layer.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None,
            );
        }
    }

    /// Previous (bar and triangle), play or pause, next, centred on `cx`, `cy` (Inter has no such symbols).

    /// Small text ending at `right`; returns its width.
    fn label_right(&mut self, text: &str, right: f32, top: f32, weight: Weight, color: Color,
    ) -> f32 {
        let w = self.text.width(text, SMALL, weight);
        self.label(text, right - w, top, SMALL, weight, color);
        w
    }

    #[allow(clippy::too_many_arguments)]
    /// A small rounded button with its label centred (Avisos' Responder/Enviar).

    fn round(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
        let s = self.scale;
        if self.glass_active && w>=16. && h>=16. && color.alpha()>0.8 && color.red().max(color.green()).max(color.blue())<0.28 {
            self.glass_card(x,y,w,h,r);
            return;
        }
        fill_round(&mut self.layer, x * s, y * s, w * s, h * s, r * s, color);
    }

    // Port of ControlPanel's glyph catalog (mac-island 3.1.1): every control uses the chosen pack.
    fn control_icon(&mut self, tile: ControlTile, cx: f32, cy: f32) {
        let icon = match tile {
            ControlTile::Wifi => Icon::Wifi,
            ControlTile::Bluetooth => Icon::Bluetooth,
            ControlTile::NightLight => Icon::NightLight,
            ControlTile::Project => Icon::Project,
            ControlTile::Cast => Icon::Cast,
            ControlTile::Captions => Icon::Captions,
            ControlTile::Accessibility => Icon::Accessibility,
            ControlTile::TaskManager => Icon::TaskManager,
            ControlTile::Explorer => Icon::Explorer,
            ControlTile::DoNotDisturb => Icon::Moon,
            ControlTile::WindowsSettings => Icon::Gear,
            ControlTile::Power => Icon::Power,

            ControlTile::Hotspot => Icon::Hotspot,
            ControlTile::Airplane => Icon::Airplane,
            ControlTile::DarkMode => Icon::Contrast,
            ControlTile::Screenshot => Icon::Crop,
            ControlTile::Lock => Icon::Lock,
            ControlTile::Calculator => Icon::Calculator,
            ControlTile::Emoji => Icon::Smile,
            ControlTile::Clipboard => Icon::Clipboard,
            ControlTile::ShowDesktop => Icon::Desktop,
            ControlTile::OnScreenKeyboard | ControlTile::TouchKeyboard => Icon::Keyboard,
            ControlTile::WinX => Icon::WinKey,
            ControlTile::Microphone => Icon::Mic,
            ControlTile::Brightness | ControlTile::Weather => Icon::Sun,
            ControlTile::Battery => Icon::Battery,
            ControlTile::PowerMode => Icon::Gauge,
            ControlTile::Drives => Icon::Usb,
            ControlTile::RecycleBin => Icon::Trash,
            ControlTile::Alarms => Icon::Bell,
            ControlTile::NextEvent => Icon::Calendar,
            _ => Icon::Timer,
        };
        self.ui_icon(icon,cx,cy,20.,white());
    }

    fn slider(&mut self, hb: &crate::pages::HitBox, pct: u8, muted: bool) {
        let cy = hb.y + hb.h*0.5;
        self.round(hb.x,cy-3.0,hb.w,6.0,3.0,surface());
        let fill = hb.w * pct as f32 / 100.0;
        if fill > 0.0 { self.round(hb.x,cy-3.0,fill,6.0,3.0,if muted { Color::from_rgba8(150,150,150,255) } else { accent() },
            ); }
        self.round(hb.x+fill-5.0,cy-5.0,10.0,10.0,5.0,white());
    }

    /// The bar's items in the settings' order (`layout::compact_slots`); quota bars along the bottom.
    fn draw_compact(&mut self, x: f32, y: f32, w: f32, h: f32, state: &IslandState) {
        use isle_core::config::BarItem;
        let slots = crate::layout::compact_slots(&self.text, x, w, state);
        let playing = crate::layout::compact_media_text(state);
        for (item, sx, sw) in slots {
            match item {
                BarItem::Clock => {
                    let clock = state.clock_text.clone();
                    self.label(&clock, sx, centered_top(y, h, CLOCK), CLOCK, Weight::Semibold, white());
                }
                BarItem::Next => {
                    let text = crate::layout::next_text(state).to_string();
                    self.bar_text(&text, sx, sw, y, h, state.chat.busy, state);
                }
                BarItem::Date => {
                    let date = state.date_text.clone();
                    self.bar_text(&date, sx, sw, y, h, false, state);
                }
                BarItem::Media => {
                    if let Some(title) = playing {
                        let controls = crate::pages::compact_media_keys(state).len() as f32 * 23.;
                        self.bar_media_text(title, sx, sw - controls, y, h, state);
                    }
                }
                BarItem::Weather => {
                    self.draw_compact_weather(sx + sw - 6.0, y, h, state);
                }
                BarItem::Notifications => {
                    self.ui_icon(Icon::Bell, sx + 9.0, y + h / 2.0, 15.0, white());
                    let count = state.notifications.unread_count().min(99).to_string();
                    self.label(&count, sx + 20.0, centered_top(y, h, 12.0), 12.0, Weight::Semibold, accent());
                }
                BarItem::Timer => {
                    let t = state.timer_text.clone().unwrap_or_default();
                    self.bar_text(&t, sx, sw, y, h, false, state);
                }
                BarItem::Tray => {}
                BarItem::Power => self.ui_icon(Icon::Power, sx + sw / 2., y + h / 2., 15., white()),
            }
        }
        self.draw_download_meter(x, y, w, state);
        if state.bar_quota_strip && (state.claude_quota.used_pct > 0.0 || state.codex_quota.used_pct > 0.0) {
            self.draw_quota_strip(x + 14.0, y + h - 5.0, w - 28.0, state);
        }
        for hb in compute_hitboxes(x*2.0+w,w,h,y,state) {
            match hb.action {
                UiAction::TrayClick(i,_) => {
                    self.draw_tray_icon(&state.tray[i],hb.x+3.0,hb.y+3.0,18.0)
                }
                UiAction::ToggleTray => {
                    self.ui_icon(Icon::Tray, hb.x + hb.w / 2., hb.y + hb.h / 2., 15., muted()); }
                UiAction::Media(key) => {
                    let icon = match key {
                        MediaKey::Previous => Icon::Prev,
                        MediaKey::Next => Icon::Next,
                        MediaKey::PlayPause if state.media.as_ref().is_some_and(|media| media.playing) => Icon::Pause,
                        MediaKey::PlayPause => Icon::Play,
                    };
                    self.ui_icon(icon, hb.x + hb.w * 0.5, hb.y + hb.h * 0.5, 13., white());
                }
                _ => {}
            }
        }
    }

    fn draw_download_meter(&mut self, x: f32, y: f32, w: f32, state: &IslandState) {
        if !state.phase8.download_meter { return; }
        // Port of Island::UpdateFill (mac-island 3.1.1), using the presentation tween.
        let animation = state.phase8.download_animation && crate::window::animations_enabled();
        if state.phase8.downloads.is_some() || (animation && state.phase8.download_fill.ending && state.phase8.download_fill.visible(state.now_ms)) {
            let progress = state.phase8.downloads;
            let track_w = (w - 26.).max(0.);
            let value = (progress.is_none() || progress.is_some_and(|p| p.permille.is_some())).then(|| {
                if animation { state.phase8.download_fill.value(state.now_ms) }
                else { progress.and_then(|p| p.permille).unwrap_or(0) as f32 / 1000. }
            });
            self.download_bar(x + 13., y + 1., track_w, 3., value, state.phase8.download_state, state.now_ms);
        }
    }

    fn bar_media_text(&mut self, text: &str, sx: f32, sw: f32, y: f32, h: f32, state: &IslandState) {
        let has_art = sw > 48.;
        let art_room = if has_art { 26. } else { 0. };
        let room = sw - art_room;
        let text_w = self.text.width(text, 12., Weight::Medium);
        if !state.ui.media_marquee || !crate::window::animations_enabled() || text_w <= room || room < 10. {
            self.bar_text(text, sx, sw, y, h, true, state);
            return;
        }
        if has_art {
            self.media_art(sx, y + (h - 22.) * 0.5, 22., state);
        }
        let left = sx + art_room;
        let speed = 34.;
        let pause = 1.0;
        let travel = text_w - room;
        let cycle = 2. * pause + 2. * travel / speed;
        let t = (state.now_ms as f64 / 1000.).rem_euclid(f64::from(cycle)) as f32;
        let offset = if t < cycle * 0.5 {
            ((t - pause) * speed).clamp(0., travel)
        } else {
            ((cycle - t) * speed).clamp(0., travel)
        };
        let scale = self.scale;
        self.text.draw_in_range(
            &mut self.layer, text, (left - offset) * scale, centered_top(y, h, 12.) * scale,
            12. * scale, Weight::Medium, muted(),
            (left * scale).ceil() as i32..((left + room) * scale).floor() as i32,
        );
    }

    /// A muted text item, right-aligned in its slot, with the cover (or the AI's face) before it.
    fn bar_text(&mut self, text: &str, sx: f32, sw: f32, y: f32, h: f32, art: bool, state: &IslandState) {
        let art_room = if art && sw > 48. { 26. } else { 0. };
        let shown = self.text.ellipsize(text, sw - art_room, 12., Weight::Medium);
        let shown_w = self.text.width(&shown, 12., Weight::Medium);
        let text_x = sx + sw - shown_w;
        if art_room > 0. {
            if state.chat.busy {
                self.avatar(text_x - 26., y + (h - 24.) / 2., 22., Mood::Thinking);
            } else {
                self.media_art(text_x - 26., y + (h - 22.) / 2., 22., state);
            }
        }
        self.label(&shown, text_x, centered_top(y, h, 12.), 12., Weight::Medium, muted());
    }

    fn draw_tray_icon(&mut self, item: &isle_core::tray::TrayItem, x: f32, y: f32, size: f32) {
        if let Some(icon) = self.tray_images.get(&item.key).and_then(|(_,p)| p.as_ref()) {
            // Smaller than the slot (a 16 px tray icon): a whole-number scale, pixel for pixel,
            // centred. Bigger: shrunk smoothly. Never a blurry fractional stretch.
            let fit = size*self.scale/icon.width().max(icon.height()) as f32;
            let (k,quality) = if fit >= 1.0 { (fit.floor(),FilterQuality::Nearest) } else { (fit,FilterQuality::Bicubic) };
            let (dw,dh) = (icon.width() as f32*k,icon.height() as f32*k);
            let (px,py) = ((x*self.scale+(size*self.scale-dw)*0.5).round(),(y*self.scale+(size*self.scale-dh)*0.5).round(),
            );
            let paint = PixmapPaint { quality, ..PixmapPaint::default() };
            self.layer.draw_pixmap(0,0,icon.as_ref(),&paint,Transform::from_scale(k,k).post_translate(px,py),None,
            );
        } else {
            self.round(x,y,size,size,4.0,surface());
            let letter = item.display_name.chars().next().unwrap_or('?').to_string();
            self.label(&letter,x+4.0,centered_top(y,size,11.0),11.0,Weight::Semibold,white(),
            );
        }
    }

    fn draw_banner(&mut self, x: f32, y: f32, w: f32, title: &str, subtitle: &str) {
        self.avatar(x + 12., y + 12., 34., Mood::Happy);
        let max = w - 64.;
        let title = self.text.ellipsize(title, max, TITLE, Weight::Semibold);
        self.label(&title, x + 56., y + 12., TITLE, Weight::Semibold, white());
        let subtitle = self.text.ellipsize(subtitle, max, SMALL, Weight::Regular);
        self.label(&subtitle, x + 56., y + 33., SMALL, Weight::Regular, muted());
    }

    /// Who asks, what it would run (two lines at most), and Permitir / Sempre / Negar.
    fn draw_card(&mut self, x: f32, y: f32, w: f32, h: f32, canvas_w: f32, state: &IslandState) {
        let Some(card) = &state.pending_permission else {
            return;
        };
        let max = w - PAD * 2.0;
        // Who asks and with which tool may be cut; the command never is (see `command_fits`).
        let who = format!("{} · {}", card.agent, card.tool_name);
        let more = (state.permission_waiting > 0).then(|| format!("+{} na fila", state.permission_waiting));
        let more_w = more.as_ref().map_or(0.0, |m| self.text.width(m, SMALL, Weight::Regular) + 8.0);
        let who = self.text.ellipsize(&who, max - more_w, SMALL, Weight::Semibold);
        if let Some(m) = &more {
            self.label(m, x + w - PAD - more_w + 8.0, y + 10., SMALL, Weight::Regular, muted());
        }
        self.avatar(x + 12., y + 6., 20., Mood::Attention);
        self.clipped_label(
            &who,
            x + 38.,
            y + 10.,
            max - 24.,
            SMALL,
            Weight::Semibold,
            muted(),
        );
        let lines = self.text.wrap_exact(&card.command, max, BODY, Weight::Regular);
        if lines.len() > CARD_MAX_LINES {
            return;
        }
        for (i, line) in lines.iter().enumerate() {
            self.label(line, x + PAD, y + CARD_COMMAND_TOP + i as f32 * CARD_LINE, BODY, Weight::Regular, white(),
            );
        }
        for hb in compute_hitboxes(canvas_w, w, h, y, state) {
            let UiAction::Permission(decision, _) = hb.action else {
                continue;
            };
            let (fill, label, ink) = match decision {
                PermissionDecision::Allow => (accent(), "Permitir", ink()),
                PermissionDecision::Always => (surface(), "Sempre", white()),
                PermissionDecision::Deny => (surface(), "Negar", red()),
            };
            self.round(hb.x, hb.y, hb.w, hb.h, hb.h * 0.5, fill);
            let label_w = self.text.width(label, SMALL, Weight::Semibold);
            self.label(label, hb.x + (hb.w - label_w) * 0.5, centered_top(hb.y, hb.h, SMALL), SMALL, Weight::Semibold, ink,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]

    /// Mac Island & Dock's "big day" calendar card: the day big beside the month, that day's events
    /// and what comes next, holidays in red.

    fn draw_quota_strip(&mut self, x: f32, y: f32, total_w: f32, state: &IslandState) {
        if total_w <= 20.0 {
            return;
        }
        let bar_w = (total_w - 8.0) * 0.5;
        for (idx, quota) in [&state.claude_quota, &state.codex_quota].iter().enumerate() {
            let bx = x + (idx as f32) * (bar_w + 8.0);
            let mut bg = Paint::default();
            bg.set_color(Color::from_rgba8(255, 255, 255, 30));
            let s = self.scale;
            if let Some(rect) = Rect::from_xywh(bx * s, y * s, bar_w * s, 3.0 * s) {
                self.layer.fill_rect(rect, &bg, Transform::identity(), None);
            }
            let fill_w = (bar_w * (quota.used_pct.clamp(0.0, 100.0) / 100.0)).max(2.0);
            let mut fg = Paint::default();
            fg.set_color(quota_bar_color(quota.used_pct));
            if let Some(rect) = Rect::from_xywh(bx * s, y * s, fill_w * s, 3.0 * s) {
                self.layer.fill_rect(rect, &fg, Transform::identity(), None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_produces_non_empty_bgra_pill_with_configured_background() {
        let cfg = IsleConfig::default();
        let geom = PillGeometry::default();
        let state = IslandState::default();
        let mut renderer = PillRenderer::new(720, 520);
        let fb = renderer.render(&geom, &state, &cfg);

        let idx = ((20 * 580 + 290) * 4) as usize;
        assert!(fb.bgra_premultiplied[idx + 3] > 200);
        assert!(fb.bgra_premultiplied[idx] >= 0x18);
        assert_eq!(fb.bgra_premultiplied[(5 * 580 + 5) * 4 + 3], 0);
    }

    #[test]
    fn draws_visible_text_glyphs_for_clock_and_permission_labels() {
        let cfg = IsleConfig::default();
        let geom = PillGeometry::default();
        let mut state = IslandState::default();
        state.clock_text = "21:30".to_string();
        let mut renderer = PillRenderer::new(720, 520);
        let fb = renderer.render(&geom, &state, &cfg);

        let mut white_pixels = 0;
        for y in 14..28 {
            for x in 190..380 {
                let idx = ((y * 720 + x) * 4) as usize;
                if fb.bgra_premultiplied[idx] > 180
                    && fb.bgra_premultiplied[idx + 1] > 180
                    && fb.bgra_premultiplied[idx + 2] > 180
                {
                    white_pixels += 1;
                }
            }
        }
        assert!(
            white_pixels > 10,
            "expected rendered white text pixels, got {white_pixels}"
        );
    }

    fn card_state() -> IslandState {
        let mut state = IslandState::default();
        state.mode = PillMode::PermissionPrompt;
        state.pending_permission = Some(isle_core::state::PermissionCard {
            request_id: "claude-0".into(),
            agent: "Claude Code · isle-win".into(),
            tool_name: "Bash".into(),
            command: "cargo test --workspace".into(),
            ..Default::default()
        });
        state
    }

    fn settled(w: f32, h: f32, r: f32) -> PillGeometry {
        let mut geom = PillGeometry::default();
        geom.morph_to(w, h, r);
        geom.width.value = w;
        geom.height.value = h;
        geom.corner_radius.value = r;
        geom
    }

    /// Text pixels in a box: every text colour (white, muted, red) is red-heavy, every fill (the pill,
    /// dark surfaces) is not; include antialiased strokes of small vector symbols.
    fn ink(fb: &PillFrameBuffer, x0: u32, y0: u32, x1: u32, y1: u32) -> usize {
        (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| ((y * fb.width + x) * 4) as usize))
            .filter(|&i| fb.bgra_premultiplied[i + 2] > 120)
            .count()
    }

    #[test]
    fn the_permission_card_shows_its_command_and_labelled_buttons() {
        let mut renderer = PillRenderer::new(720, 520);
        let fb = renderer.render(&settled(440.0, 130.0, 22.0), &card_state(), &IsleConfig::default(),
        );
        let x = (720 - 440) / 2;
        // The command line, then each of the three buttons (Allow, Always, Deny) carries a label.
        assert!(ink(fb, x + 16, 34, x + 300, 54) > 40, "command text");
        for hb in compute_hitboxes(720.0, 440.0, 130.0, 6.0, &card_state()) {
            let (bx, by) = (hb.x as u32, hb.y as u32);
            assert!(ink(fb, bx, by, bx + hb.w as u32, by + hb.h as u32) > 15, "{:?}", hb.action);
        }
    }

    #[test]
    fn content_never_spills_outside_a_pill_still_growing() {
        let mut renderer = PillRenderer::new(720, 520);
        // The card's content while the pill is still compact: nothing below the pill and its glow.
        let fb = renderer.render(&settled(220.0, 34.0, 17.0), &card_state(), &IsleConfig::default(),
        );
        let below = (62..520u32)
            .flat_map(|y| (0..720u32).map(move |x| ((y * 720 + x) * 4 + 3) as usize))
            .filter(|&i| fb.bgra_premultiplied[i] != 0)
            .count();
        assert_eq!(below, 0);
    }

    #[test]
    fn the_expanded_pill_shows_its_tabs() {
        let mut renderer = PillRenderer::new(720, 520);
        let mut state = IslandState::default();
        state.mode = PillMode::Expanded(isle_core::state::TabPage::AiHub);
        let fb = renderer.render(&settled(560.0, 450.0, 28.0), &state, &IsleConfig::default());
        for hb in compute_hitboxes(720.0, 560.0, 450.0, 6.0, &state)
            .into_iter()
            .filter(|hb| matches!(hb.action, crate::pages::UiAction::SwitchTab(_)))
        {
            let (bx, by) = (hb.x as u32, hb.y as u32);
            assert!(ink(fb, bx, by, bx + hb.w as u32, by + hb.h as u32) > 10, "{:?}", hb.action);
        }
    }

    #[test]
    fn a_scaled_screen_gets_the_same_pill_in_more_pixels() {
        let state = card_state();
        let geom = settled(440.0, 130.0, 22.0);
        let mut normal = PillRenderer::new(720, 520);
        let mut hidpi = PillRenderer::with_scale(720, 520, 1.5);
        let small = normal.render(&geom, &state, &IsleConfig::default());
        let small_ink = ink(small, 0, 0, 720, 520);
        let big = hidpi.render(&geom, &state, &IsleConfig::default());
        assert_eq!((big.width, big.height), (1080, 780));
        // The pill's middle is opaque at its scaled place; the corner stays clear.
        let at = |x: u32, y: u32| big.bgra_premultiplied[((y * big.width + x) * 4 + 3) as usize];
        assert!(at(540, 100) > 200);
        assert_eq!(at(10, 10), 0);
        assert!(at(540, 6 * 3 / 2 + 130 * 3 / 2 + 32) == 0, "nothing under the scaled pill");
        // Text is redrawn sharper, not stretched: about 1.5² as many inked pixels.
        let big_ink = ink(big, 0, 0, 1080, 780) as f32;
        assert!(big_ink > small_ink as f32 * 1.8, "{big_ink} vs {small_ink}");
    }

    #[test]
    fn a_frame_after_a_bigger_one_redraws_only_around_the_pill_and_leaves_nothing_behind() {
        let mut state = IslandState::default();
        state.clock_text = "21:30".into();
        let compact = settled(220.0, 34.0, 17.0);
        let fresh = PillRenderer::with_scale(720, 520, 1.5).render(&compact, &state, &IsleConfig::default()).clone();
        let mut renderer = PillRenderer::with_scale(720, 520, 1.5);
        let mut open = state.clone();
        open.mode = PillMode::Expanded(isle_core::state::TabPage::Media);
        renderer.render(&settled(560.0, 380.0, 28.0), &open, &IsleConfig::default());
        // Shrinking: the panel's old pixels are cleared, the frame is the one a new renderer draws.
        let shrunk = renderer.render(&compact, &state, &IsleConfig::default()).clone();
        assert!(shrunk.bgra_premultiplied == fresh.bgra_premultiplied, "stale pixels left by the bigger frame");
        // Settled small: only the pill and its shadow change, not the whole 1080x780 canvas.
        let (x0, y0, x1, y1) = renderer.render(&compact, &state, &IsleConfig::default()).dirty;
        assert!((x1 - x0) * (y1 - y0) < 1080 * 780 / 8, "{:?}", (x0, y0, x1, y1));
    }

    #[test]
    fn the_hidden_line_still_catches_the_mouse_at_the_screen_edge() {
        let mut renderer = PillRenderer::new(720, 520);
        let mut state = IslandState::default();
        state.mode = PillMode::Minimized;
        let fb = renderer.render(&settled(120.0, 5.0, 2.5), &state, &IsleConfig::default());
        let alpha = |x: u32, y: u32| fb.bgra_premultiplied[((y * 720 + x) * 4 + 3) as usize];
        // The very top row above the line, and a little to its sides, take the mouse (alpha > 0) unseen.
        assert!((1..8).contains(&alpha(360, 0)));
        assert!(alpha(360 - 60 - 10, 2) > 0);
        assert!(alpha(360, 8) > 200, "the line itself shows");
        assert_eq!(alpha(100, 2), 0, "far from it, clicks go through");
        assert_eq!(alpha(360, 40), 0);
    }

    #[test]
    fn resets_read_as_people_say_them() {
        let min = 60_000;
        assert_eq!(resets_in(0, 1_000), "");
        assert_eq!(resets_in(1_000, 2_000), "renovando");
        assert_eq!(resets_in(30_000, 0), "renova em 1m");
        assert_eq!(resets_in(133 * min, 0), "renova em 2h 13m");
        assert_eq!(resets_in((3 * 24 * 60 + 4 * 60 + 5) * min, 0), "renova em 3d 4h");
    }

    #[test]
    fn the_ai_tab_shows_each_plans_limits() {
        use isle_core::state::{UsageProvider, UsageWindow};
        let mut state = IslandState::default();
        state.mode = PillMode::Expanded(isle_core::state::TabPage::AiHub);
        state.usage = vec![
            UsageProvider {
                id: "claude".into(),
                name: "Claude".into(),
                plan: "Pro".into(),
                windows: vec![
                    UsageWindow { label: "5 horas".into(), percent: 62.0, resets_at_ms: 3_600_000,
                    },
                    UsageWindow { label: "Semana".into(), percent: 31.0, resets_at_ms: 0,
                    },
                ],
                ..UsageProvider::default()
            },
            UsageProvider { id: "codex".into(), name: "Codex".into(), note: "Do último uso do Codex".into(), ..UsageProvider::default() },
        ];
        let mut renderer = PillRenderer::new(720, 520);
        let fb = renderer.render(&settled(560.0, 450.0, 28.0), &state, &IsleConfig::default());
        // Port of isle's usage page: the head at the body's top (6 + 94 + 14), a limit's line 34 below, its bar 22 under that.
        let left = 100u32;
        assert!(ink(fb, left + 26, 114, left + 120, 132) > 30, "provider name beside logo");
        assert!(ink(fb, left + 70, 114, left + 110, 134) > 10, "plan badge");
        assert!(ink(fb, left, 148, left + 60, 164) > 10, "5-hour label");
        let warm = (170..175u32)
            .flat_map(|y| (left..left + 150).map(move |x| ((y * 720 + x) * 4) as usize))
            .filter(|&i| {
                fb.bgra_premultiplied[i + 2] > 200
                    && fb.bgra_premultiplied[i + 1] > 100
                    && fb.bgra_premultiplied[i] < 150
            })
            .count();
        assert!(warm > 100, "visible 62% quota bar");
        assert!(!compute_hitboxes(720.0, 560.0, 450.0, 6.0, &state).iter().any(|b| matches!(b.action, crate::pages::UiAction::AiInput)), "no chat on the usage page");
    }

    #[test]
    fn the_media_tab_shows_the_song_and_its_three_buttons() {
        let mut state = IslandState::default();
        state.mode = PillMode::Expanded(isle_core::state::TabPage::Media);
        state.media = Some(isle_core::state::MediaInfo {
            artwork: None,
            title: "Song".into(),
            artist: "Artist".into(),
            playing: true,
            position_ms: 60_000,
            duration_ms: 180_000,
            sampled_at_ms: 0,
        });
        state.active_lyric_line = Some("a line being sung".into());
        let mut renderer = PillRenderer::new(720, 520);
        let fb = renderer.render(&settled(560.0, 450.0, 28.0), &state, &IsleConfig::default());
        let left = 226;
        assert!(ink(fb, left, 142, left + 200, 168) > 20, "media title");
        let buttons: Vec<_> = compute_hitboxes(720.0, 560.0, 450.0, 6.0, &state)
            .into_iter()
            .filter(|hb| matches!(hb.action, crate::pages::UiAction::Media(_)))
            .collect();
        assert_eq!(buttons.len(), 3);
        for hb in buttons {
            let (bx, by) = (hb.x as u32, hb.y as u32);
            assert!(ink(fb, bx, by, bx + hb.w as u32, by + hb.h as u32) > 10, "{:?}", hb.action);
        }
    }

    #[test]
    fn control_tiles_and_mixer_draw_at_their_hitboxes_at_both_scales() {
        let mut state = IslandState::default();
        state.mode = PillMode::Expanded(TabPage::ControlCenter);
        state.bluetooth_on = Some(true); state.audio_available = true; state.mic_available = true;
        state.volume_pct = 65;
        state.app_volumes.push(isle_core::state::AppVolume { id: "app".into(), name: "Firefox".into(), volume_pct: 40, muted: false,
        });
        for scale in [1.0, 1.5] {
            let mut renderer = PillRenderer::with_scale(720, 520, scale);
            let fb = renderer.render(&settled(600.0, 470.0, 28.0), &state, &IsleConfig::default());
            for hb in compute_hitboxes(720.0, 600.0, 470.0, 6.0, &state) {
                if let UiAction::Control(_) = hb.action {
                    assert!(ink(fb, (hb.x*scale) as u32, ((hb.y+ 10.0)*scale) as u32,
                        ((hb.x+hb.w)*scale) as u32, ((hb.y+hb.h)*scale) as u32) > 10, "tile label {:?}", hb.action);
                }
            }
            let hb = compute_hitboxes(720., 600., 470., 6., &state)
                .into_iter()
                .find(|b| b.action == UiAction::Control(ControlTile::Bluetooth))
                .unwrap();
            let i = ((((hb.y + 3.) * scale) as u32 * fb.width + ((hb.x + 20.) *scale) as u32)*4) as usize;
            assert!(fb.bgra_premultiplied[i + 2] > fb.bgra_premultiplied[i],
                "active connection has warm tint"
            );
        }
    }

    #[test]
    fn notification_titles_and_bodies_draw_inside_clickable_rows_at_both_scales() {
        let mut state = IslandState::default();
        state.mode = PillMode::Expanded(TabPage::Notifications);
        state.notifications.receive((1..=4).map(|id| isle_core::notifications::Notification {
            id, app_id: "Mail".into(), app_name: "Mail".into(), title: "Mensagem recebida".into(),
            body: "O build terminou com sucesso.".into(), ..Default::default()
        }).collect(),false,
        );
        for scale in [1.0,1.5] {
            let mut renderer = PillRenderer::with_scale(720, 520, scale);
            let fb = renderer.render(&settled(600.0, 470.0, 28.0),&state,&IsleConfig::default());
            for hb in compute_hitboxes(720.0, 600.0, 470.0,6.0,&state) {
                if matches!(hb.action,UiAction::ReadNotification(..)) {
                    for top in [hb.y + 24.0,hb.y + 43.0] {
                        assert!(ink(fb,((hb.x+12.0)*scale) as u32,(top*scale) as u32,
                            ((hb.x+hb.w-12.0)*scale) as u32,((top+15.0)*scale) as u32)>20);
                    }
                }
            }
        }
    }

    #[test]
    fn a_long_command_shows_whole_on_a_taller_card() {
        let cmd = "cargo build --release --target x86_64-pc-windows-msvc --offline && ./scripts/build-win.sh && echo done && rm -rf ~/scratch";
        assert!(command_fits(cmd));
        let tall = card_height(cmd);
        assert!(tall > card_height("ls"), "{tall}");
        let mut state = card_state();
        state.pending_permission.as_mut().unwrap().command = cmd.into();
        let mut renderer = PillRenderer::new(720, 520);
        let fb = renderer.render(&settled(440.0, tall, 22.0), &state, &IsleConfig::default());
        // The command's last line (with "rm -rf") is drawn above the buttons, not cut away.
        let lines = Text::new().wrap_exact(cmd, 440.0 - PAD * 2.0, BODY, Weight::Regular);
        let last_top = 6.0 + CARD_COMMAND_TOP + (lines.len() - 1) as f32 * CARD_LINE;
        let x = (720 - 440) / 2 + 18;
        assert!(ink(fb, x, last_top as u32, x + 200, last_top as u32 + 16) > 20, "last line inked");
        // Far longer than any card can show: never offered for approval.
        assert!(!command_fits(&"x".repeat(2_000)));
    }
}
