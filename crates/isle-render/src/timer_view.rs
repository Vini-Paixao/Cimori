// SPDX-License-Identifier: GPL-3.0
// The Timer tab: a ring with the time inside, after the GNOME isle's Timer page (Copyright (C) 2026
// zickrian, GPL-3.0-or-later: ring, work/break steppers, Start/Pause/Reset), with the countdown and
// stopwatch Mac Island & Dock 3.1.1 has beside it.

use super::*;

impl PillRenderer {
    /// A ring `width` thick, full at `fraction` (clockwise from the top).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn ring(&mut self, cx: f32, cy: f32, r: f32, fraction: f32, color: Color, width: f32) {
        let s = self.scale;
        let mut paint = Paint::default();
        paint.anti_alias = true;
        let stroke = Stroke { width: width * s, line_cap: tiny_skia::LineCap::Round, ..Default::default() };
        let arc = |from: f32, to: f32| {
            let mut pb = PathBuilder::new();
            let steps = (64.0 * (to - from)).ceil().max(2.0) as usize;
            for i in 0..=steps {
                let a = (from + (to - from) * i as f32 / steps as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                let (x, y) = ((cx + a.cos() * r) * s, (cy + a.sin() * r) * s);
                if i == 0 { pb.move_to(x, y); } else { pb.line_to(x, y); }
            }
            pb.finish()
        };
        paint.set_color(Color::from_rgba8(255, 255, 255, 30));
        if let Some(path) = arc(0.0, 1.0) { self.layer.stroke_path(&path, &paint, &stroke, Transform::identity(), None); }
        if fraction > 0.002 {
            paint.set_color(color);
            if let Some(path) = arc(0.0, fraction.min(1.0)) { self.layer.stroke_path(&path, &paint, &stroke, Transform::identity(), None); }
        }
    }
}
