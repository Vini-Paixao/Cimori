// SPDX-License-Identifier: GPL-3.0
// The weather panel, laid out after Mac Island & Dock 3.1.1's (city and temperature, eight facts,
// seven days, the hours as a curve), and the sky icons the island draws itself: Inter has no
// emoji, so each of the mod's kinds of sky (WeatherConcept) is a few shapes.

use super::*;
use isle_core::weather::{Sky, degrees};

fn sun() -> Color { Color::from_rgba8(0xFF, 0xC5, 0x3D, 255) }
fn moon() -> Color { Color::from_rgba8(0xF2, 0xE6, 0xC2, 255) }
fn cloud() -> Color { Color::from_rgba8(0xDC, 0xE2, 0xEA, 255) }
fn storm_cloud() -> Color { Color::from_rgba8(0xA9, 0xB3, 0xC1, 255) }
fn water() -> Color { Color::from_rgba8(0x5A, 0xA9, 0xFF, 255) }


/// "2026-11-11" as local days since 1970.
pub(super) fn day_number(date: &str) -> Option<i64> {
    let mut parts = date.split('-').map(|p| p.parse::<i64>().ok());
    Some(isle_core::agenda::days_from_civil(parts.next()??, parts.next()??, parts.next()??,
    ))
}

impl PillRenderer {
    pub(super) fn circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        self.round(cx - r, cy - r, r * 2.0, r * 2.0, r, color);
    }

    /// Lines in logical coordinates, in a color and width.
    pub(super) fn stroke(&mut self, lines: &[&[(f32, f32)]], color: Color, width: f32) {
        let mut path = PathBuilder::new();
        for line in lines {
            for (i, &(x, y)) in line.iter().enumerate() {
                if i == 0 { path.move_to(x, y); } else { path.line_to(x, y); }
            }
        }
        if let Some(path) = path.finish() {
            let mut paint = Paint::default();
            paint.set_color(color);
            paint.anti_alias = true;
            let stroke = Stroke { width, line_cap: tiny_skia::LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
            self.layer.stroke_path(&path, &paint, &stroke, Transform::from_scale(self.scale, self.scale), None,
            );
        }
    }

    /// A cloud on (cx, cy), `u` = size/20.
    fn cloud_shape(&mut self, cx: f32, cy: f32, u: f32, color: Color) {
        self.circle(cx - 4.0 * u, cy + 1.5 * u, 4.5 * u, color);
        self.circle(cx + 1.5 * u, cy - 1.5 * u, 6.0 * u, color);
        self.circle(cx + 6.0 * u, cy + 2.0 * u, 4.0 * u, color);
        self.round(cx - 8.5 * u, cy + 1.5 * u, 18.5 * u, 4.5 * u, 2.2 * u, color,
        );
    }

    fn sun_shape(&mut self, cx: f32, cy: f32, r: f32) {
        let rays: Vec<[(f32, f32); 2]> = (0..8).map(|i| {
            let a = i as f32 * std::f32::consts::FRAC_PI_4;
            [(cx + a.cos() * r * 1.45, cy + a.sin() * r * 1.45), (cx + a.cos() * r * 1.95, cy + a.sin() * r * 1.95),
                ]
        }).collect();
        let lines: Vec<&[(f32, f32)]> = rays.iter().map(|r| r.as_slice()).collect();
        self.stroke(&lines, sun(), (r * 0.32).max(1.0));
        self.circle(cx, cy, r, sun());
    }

    /// A crescent: the left of a disc, cut by a curve bulging into it.
    fn moon_shape(&mut self, cx: f32, cy: f32, r: f32) {
        let s = self.scale;
        let mut pb = PathBuilder::new();
        let (sin, cos) = (70f32.to_radians().sin(), 70f32.to_radians().cos());
        let steps = 20;
        for i in 0..=steps {
            let a = (70.0 + 220.0 * i as f32 / steps as f32).to_radians();
            let (x, y) = (cx + a.cos() * r, cy + a.sin() * r);
            if i == 0 { pb.move_to(x * s, y * s); } else { pb.line_to(x * s, y * s); }
        }
        pb.quad_to((cx - 0.1 * r) * s, cy * s, (cx + cos * r) * s, (cy + sin * r) * s,
        );
        pb.close();
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.anti_alias = true;
            paint.set_color(moon());
            self.layer.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None,
            );
        }
    }

    /// The sky's icon, `size` wide, centred on (cx, cy).
    pub(super) fn sky_icon(&mut self, sky: Sky, cx: f32, cy: f32, size: f32) {
        let u = size / 20.0;
        let drops = |me: &mut Self, color: Color, short: bool| {
            let len = if short { 1.6 } else { 3.6 };
            let lines: Vec<[(f32, f32); 2]> = [-4.0f32, 0.5, 5.0].iter()
                .map(|&dx| {
                    [(cx + dx * u, cy + 7.5 * u), (cx + (dx - 1.2) * u, cy + (7.5 + len) * u),
                    ]
                }).collect();
            let refs: Vec<&[(f32, f32)]> = lines.iter().map(|l| l.as_slice()).collect();
            me.stroke(&refs, color, 1.6 * u);
        };
        match sky {
            Sky::ClearDay => self.sun_shape(cx, cy, 5.0 * u),
            Sky::ClearNight => self.moon_shape(cx, cy, 7.0 * u),
            Sky::PartlyDay => { self.sun_shape(cx - 3.5 * u, cy - 3.5 * u, 3.8 * u); self.cloud_shape(cx + 1.5 * u, cy + 2.0 * u, 0.8 * u, cloud()); }
            Sky::PartlyNight => { self.moon_shape(cx - 3.0 * u, cy - 3.5 * u, 5.0 * u); self.cloud_shape(cx + 1.5 * u, cy + 2.0 * u, 0.8 * u, cloud()); }
            Sky::Cloud => self.cloud_shape(cx, cy, u, cloud()),
            Sky::Fog => {
                self.cloud_shape(cx, cy - 3.0 * u, 0.8 * u, cloud());
                for (dy, w) in [(5.0, 14.0), (8.5, 10.0)] {
                    self.round(cx - w * 0.5 * u, cy + dy * u, w * u, 1.8 * u, 0.9 * u, cloud(),
                    );
                }
            }
            Sky::Rain => { self.cloud_shape(cx, cy - 2.5 * u, u, storm_cloud()); drops(self, water(), false); }
            Sky::Drizzle => { self.cloud_shape(cx, cy - 2.5 * u, u, cloud()); drops(self, water(), true); }
            Sky::Snow | Sky::Sleet => {
                self.cloud_shape(cx, cy - 2.5 * u, u, cloud());
                for (i, dx) in [-4.0f32, 0.5, 5.0].into_iter().enumerate() {
                    if sky == Sky::Sleet && i == 1 { drops(self, water(), true); continue; }
                    self.circle(cx + dx * u, cy + 9.0 * u, 1.3 * u, white());
                }
            }
            Sky::Thunder => {
                self.cloud_shape(cx, cy - 2.5 * u, u, storm_cloud());
                self.polygon(&[(cx + 0.5 * u, cy + 4.0 * u), (cx - 3.0 * u, cy + 10.0 * u), (cx - 0.2 * u, cy + 10.0 * u),
                    (cx - 1.5 * u, cy + 14.5 * u), (cx + 3.5 * u, cy + 7.8 * u), (cx + 0.6 * u, cy + 7.8 * u), (cx + 2.2 * u, cy + 4.0 * u),
                    ],
                    sun(),
                );
            }
        }
    }

    /// The selected day and the card surfaces use the same rectangles as the day buttons.
    pub(super) fn draw_forecast_day(
        &mut self,
        hb: &crate::pages::HitBox,
        day: &isle_core::weather::Day,
        name: &str,
        selected: bool,
        state: &IslandState,
    ) {
        let cards = state.ui.weather_cards;
        let cx = hb.x + hb.w / 2.;
        if cards || selected {
            let color = match (cards, selected) {
                (true, true) => tint(),
                (true, false) => Color::from_rgba8(255, 255, 255, 8),
                (false, _) => surface(),
            };
            self.round(hb.x, hb.y, hb.w, hb.h, 11., color);
        }
        let name = self.text.ellipsize(name, hb.w - 4., 10., Weight::Medium);
        self.centered_label(&name, cx, hb.y + 5., 10., Weight::Medium, muted());
        self.sky_icon(
            Sky::of(day.code, true),
            cx,
            hb.y + if cards { 28. } else { 31. },
            21.,
        );
        let high = degrees(day.max, state.fahrenheit);
        let low = degrees(day.min, state.fahrenheit);
        if cards {
            self.centered_label(&high, cx, hb.y + 44., 11., Weight::Medium, white());
            self.centered_label(&low, cx, hb.y + 57., 10., Weight::Medium, muted());
        } else {
            self.centered_label(
                &format!("{high}  {low}"),
                cx,
                hb.y + 49.,
                11.,
                Weight::Medium,
                white(),
            );
        }
    }

    /// The compact bar's "[sky] 24°", right-aligned at `right`; its width.
    pub(super) fn draw_compact_weather(&mut self, right: f32, y: f32, h: f32, state: &IslandState,
    ) -> f32 {
        let Some(weather) = &state.weather else { return 0.0; };
        let text = degrees(weather.temperature, state.fahrenheit);
        let text_w = self.text.width(&text, CLOCK, Weight::Medium);
        self.label(&text, right - text_w, centered_top(y, h, CLOCK), CLOCK, Weight::Medium, white(),
        );
        self.sky_icon(Sky::of(weather.code, weather.day), right - text_w - 13.0, y + h * 0.5 - 1.0, 16.0,
        );
        text_w + 26.0
    }
}
