//! Cimori's companion is separate from its brand mark. Event-driven expressions, no idle timer.
use super::*;
#[derive(Clone, Copy)]
pub(super) enum Mood {
    Ready,
    Thinking,
    Speaking,
    Happy,
    Attention,
    Error,
}
impl PillRenderer {
    pub(super) fn avatar(&mut self, x: f32, y: f32, size: f32, mood: Mood) {
        let s = self.scale * size / 80.0;
        let transform = Transform::from_scale(s, s).post_translate(x * self.scale, y * self.scale);
        let mut p = PathBuilder::new();
        p.move_to(10., 65.);
        p.line_to(10., 37.);
        p.cubic_to(10., 17., 22., 7., 40., 7.);
        p.cubic_to(58., 7., 70., 17., 70., 37.);
        p.line_to(70., 65.);
        p.cubic_to(70., 77., 64., 80., 59., 73.);
        p.cubic_to(54., 65., 50., 69., 46., 75.);
        p.cubic_to(42., 81., 38., 81., 34., 75.);
        p.cubic_to(30., 69., 26., 65., 21., 73.);
        p.cubic_to(16., 80., 10., 77., 10., 65.);
        p.close();
        let mut paint = Paint::default();
        paint.anti_alias = true;
        paint.shader = tiny_skia::LinearGradient::new(
            tiny_skia::Point::from_xy(18., 8.),
            tiny_skia::Point::from_xy(62., 79.),
            vec![
                tiny_skia::GradientStop::new(0., Color::from_rgba8(255, 250, 243, 255)),
                tiny_skia::GradientStop::new(0.6, white()),
                tiny_skia::GradientStop::new(1., Color::from_rgba8(240, 174, 143, 255)),
            ],
            tiny_skia::SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap();
        if let Some(path) = p.finish() {
            self.layer
                .fill_path(&path, &paint, FillRule::Winding, transform, None);
        }
        let (mut dx, mut dy) = if matches!(mood, Mood::Thinking) {
            (3., 0.)
        } else if matches!(mood, Mood::Error) {
            (0., 3.)
        } else if matches!(mood, Mood::Attention) {
            (2., -3.)
        } else {
            (0., 0.)
        };
        if crate::window::animations_enabled() {
            if let Some((mx, my)) = self.pointer {
                dx += ((mx - x - size / 2.) / 40.).clamp(-2.5, 2.5);
                dy += ((my - y - size / 2.) / 50.).clamp(-2., 2.);
            }
        }
        let k = size / 80.;
        let eyes = Color::from_rgba8(37, 40, 46, 255);
        for px in [27., 46.] {
            self.round(
                x + (px + dx) * k,
                y + (32. + dy) * k,
                7. * k,
                if matches!(mood, Mood::Thinking) {
                    10. * k
                } else {
                    14. * k
                },
                3.5 * k,
                eyes,
            );
        }
        if matches!(mood, Mood::Happy | Mood::Speaking) {
            self.round(
                x + 36. * k,
                y + 52. * k,
                8. * k,
                if matches!(mood, Mood::Speaking) {
                    7. * k
                } else {
                    3. * k
                },
                3. * k,
                eyes,
            );
        }
        if matches!(mood, Mood::Attention) {
            self.round(x + 44. * k, y + 24. * k, 12. * k, 2.5 * k, k, eyes);
        }
    }
}
