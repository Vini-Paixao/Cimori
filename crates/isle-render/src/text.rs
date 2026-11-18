//! Text on the pill: Inter (the font zickrian/isle ships, SIL OFL, see assets/Inter-LICENSE.txt), embedded so
//! the island looks the same everywhere and tests need no system fonts. Sizes are CSS-like pixels (em size).

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, VariableFont, point};
use tiny_skia::{Color, Pixmap, PremultipliedColorU8};

static INTER: &[u8] = include_bytes!("../assets/InterVariable.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Medium,
    Semibold,
}

pub struct Text {
    regular: FontRef<'static>,
    medium: FontRef<'static>,
    semibold: FontRef<'static>,
}

impl Default for Text {
    fn default() -> Self {
        Self::new()
    }
}

impl Text {
    pub fn new() -> Self {
        let at = |wght: f32| {
            let mut font = FontRef::try_from_slice(INTER).expect("embedded Inter is a valid font");
            font.set_variation(b"wght", wght);
            font
        };
        Self {
            regular: at(400.0),
            medium: at(500.0),
            semibold: at(600.0),
        }
    }

    fn font(&self, weight: Weight) -> &FontRef<'static> {
        match weight {
            Weight::Regular => &self.regular,
            Weight::Medium => &self.medium,
            Weight::Semibold => &self.semibold,
        }
    }

    /// ab_glyph scales by ascent-to-descent height; CSS sizes are the em.
    fn scale(font: &FontRef<'static>, size: f32) -> PxScale {
        let em = font.units_per_em().unwrap_or(1000.0);
        PxScale::from(size * font.height_unscaled() / em)
    }

    /// How wide `text` is at `size` px.
    pub fn width(&self, text: &str, size: f32, weight: Weight) -> f32 {
        let font = self.font(weight);
        let scaled = font.as_scaled(Self::scale(font, size));
        let mut width = 0.0;
        let mut last = None;
        for ch in text.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(prev) = last {
                width += scaled.kern(prev, id);
            }
            width += scaled.h_advance(id);
            last = Some(id);
        }
        width
    }

    /// `text` cut to fit `max_w`, ending in "…" when cut.
    pub fn ellipsize(&self, text: &str, max_w: f32, size: f32, weight: Weight) -> String {
        if self.width(text, size, weight) <= max_w {
            return text.to_owned();
        }
        let ends: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
        for &end in ends.iter().rev() {
            let cut = format!("{}…", text[..end].trim_end());
            if self.width(&cut, size, weight) <= max_w {
                return cut;
            }
        }
        "…".to_owned()
    }

    /// `text` in at most `max_lines` lines of `max_w`, broken between words (inside a word too long for
    /// a line); the last line ends in "…" when text is left over.
    pub fn wrap(&self, text: &str, max_w: f32, size: f32, weight: Weight, max_lines: usize) -> Vec<String> {
        let fits = |line: &str| self.width(line, size, weight) <= max_w;
        let mut lines = Vec::new();
        let mut current = String::new();
        for word in text.split_whitespace() {
            let joined = if current.is_empty() {
                word.to_owned()
            } else {
                format!("{current} {word}")
            };
            if fits(&joined) {
                current = joined;
                continue;
            }
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            for ch in word.chars() {
                current.push(ch);
                if !fits(&current) && current.chars().count() > 1 {
                    current.pop();
                    lines.push(std::mem::take(&mut current));
                    current.push(ch);
                }
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
        let max_lines = max_lines.max(1);
        if lines.len() > max_lines {
            let rest = lines[max_lines - 1..].join(" ");
            lines.truncate(max_lines - 1);
            // Left over text always shows as cut, even when the rest would happen to fit.
            let mut last = self.ellipsize(&format!("{rest}…"), max_w, size, weight);
            if !last.ends_with('…') {
                last.push('…');
            }
            lines.push(last);
        }
        lines
    }

    /// `text` in as many lines of `max_w` as it needs, every character kept as it is (spaces too): for what
    /// must be shown exactly, like a command asked about. Lines break after a space when there is one.
    pub fn wrap_exact(&self, text: &str, max_w: f32, size: f32, weight: Weight) -> Vec<String> {
        let mut lines = Vec::new();
        let mut current = String::new();
        for ch in text.chars() {
            current.push(ch);
            if self.width(&current, size, weight) <= max_w || current.chars().count() == 1 {
                continue;
            }
            current.pop();
            // Break after the line's last space, carrying the word begun after it.
            let carried = match current.rfind(' ') {
                Some(at) if at + 1 < current.len() => current.split_off(at + 1),
                _ => String::new(),
            };
            lines.push(std::mem::replace(&mut current, carried));
            current.push(ch);
        }
        if !current.is_empty() {
            lines.push(current);
        }
        lines
    }

    /// Draws `text` with its top at `top`; returns its width.
    pub fn draw(
        &self,
        pixmap: &mut Pixmap,
        text: &str,
        x: f32,
        top: f32,
        size: f32,
        weight: Weight,
        color: Color,
    ) -> f32 {
        self.draw_in_range(pixmap, text, x, top, size, weight, color, 0..i32::MAX)
    }

    pub(crate) fn draw_in_range(
        &self, pixmap: &mut Pixmap, text: &str, x: f32, top: f32, size: f32, weight: Weight,
        color: Color, range: std::ops::Range<i32>,
    ) -> f32 {
        let font = self.font(weight);
        let scaled = font.as_scaled(Self::scale(font, size));
        let baseline = top + scaled.ascent();
        let (w, h) = (pixmap.width() as i32, pixmap.height() as i32);
        let pixels = pixmap.pixels_mut();
        let mut caret = x;
        let mut last = None;
        for ch in text.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(prev) = last {
                caret += scaled.kern(prev, id);
            }
            let glyph = id.with_scale_and_position(scaled.scale(), point(caret, baseline));
            caret += scaled.h_advance(id);
            last = Some(id);
            let Some(outlined) = font.outline_glyph(glyph) else {
                continue;
            };
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, coverage| {
                let px = bounds.min.x as i32 + gx as i32;
                let py = bounds.min.y as i32 + gy as i32;
                if !range.contains(&px) || px < 0 || py < 0 || px >= w || py >= h {
                    return;
                }
                let dst = &mut pixels[(py * w + px) as usize];
                *dst = blend(*dst, color, coverage.clamp(0.0, 1.0));
            });
        }
        caret - x
    }
}

/// `color` at `coverage` over a premultiplied pixel.
fn blend(dst: PremultipliedColorU8, color: Color, coverage: f32) -> PremultipliedColorU8 {
    let a = color.alpha() * coverage;
    let keep = 1.0 - a;
    let mix = |src: f32, dst: u8| ((src * a * 255.0) + dst as f32 * keep).round().clamp(0.0, 255.0) as u8;
    let out_a = mix(1.0, dst.alpha());
    let (r, g, b) = (
        mix(color.red(), dst.red()).min(out_a),
        mix(color.green(), dst.green()).min(out_a),
        mix(color.blue(), dst.blue()).min(out_a),
    );
    PremultipliedColorU8::from_rgba(r, g, b, out_a).unwrap_or(dst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bolder_and_longer_text_is_wider() {
        let t = Text::new();
        let short = t.width("22:41", 15.0, Weight::Regular);
        assert!(short > 20.0 && short < 60.0, "{short}");
        assert!(t.width("22:41", 15.0, Weight::Semibold) > short);
        assert!(t.width("22:41 terça", 15.0, Weight::Regular) > short);
        assert_eq!(t.width("", 15.0, Weight::Regular), 0.0);
    }

    #[test]
    fn long_text_is_cut_with_an_ellipsis_to_fit() {
        let t = Text::new();
        let text = "Claude Code concluiu · um-projeto-com-nome-bem-comprido";
        let cut = t.ellipsize(text, 120.0, 13.0, Weight::Semibold);
        assert!(cut.ends_with('…') && cut.len() < text.len(), "{cut}");
        assert!(t.width(&cut, 13.0, Weight::Semibold) <= 120.0);
        assert_eq!(t.ellipsize("curto", 120.0, 13.0, Weight::Regular), "curto");
    }

    #[test]
    fn commands_wrap_between_words_and_end_in_an_ellipsis() {
        let t = Text::new();
        let cmd = "cargo build --release --target x86_64-pc-windows-msvc --offline && ./scripts/build-win.sh";
        let lines = t.wrap(cmd, 200.0, 13.0, Weight::Regular, 2);
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| t.width(l, 13.0, Weight::Regular) <= 200.0), "{lines:?}");
        assert!(lines[0].starts_with("cargo build"));
        assert!(lines[1].ends_with('…'));
        // A word longer than the line is broken inside it.
        let long = t.wrap(&"x".repeat(80), 100.0, 13.0, Weight::Regular, 3);
        assert!(long.len() > 1 && long.iter().all(|l| t.width(l, 13.0, Weight::Regular) <= 100.0));
        assert_eq!(t.wrap("ls", 200.0, 13.0, Weight::Regular, 2), vec!["ls"]);
    }

    #[test]
    fn drawing_puts_ink_where_the_text_is() {
        let t = Text::new();
        let mut pixmap = Pixmap::new(120, 40).unwrap();
        let w = t.draw(&mut pixmap, "Permitir", 10.0, 10.0, 13.0, Weight::Semibold, Color::WHITE);
        assert!(w > 30.0);
        let inked = |x0: usize, x1: usize| {
            (0..40).flat_map(|y| (x0..x1).map(move |x| (x, y)))
                .filter(|&(x, y)| pixmap.pixels()[y * 120 + x].alpha() > 128)
                .count()
        };
        assert!(inked(10, 10 + w as usize) > 30);
        assert_eq!(inked(12 + w as usize, 120), 0);
    }

    #[test]
    fn exact_wrapping_keeps_every_character_of_a_command() {
        let t = Text::new();
        let cmd = "grep \"a  b\" f && cargo build --release --target x86_64-pc-windows-msvc && rm -rf ~/x";
        let lines = t.wrap_exact(cmd, 160.0, 13.0, Weight::Regular);
        assert!(lines.len() > 2);
        assert_eq!(lines.concat(), cmd, "nothing dropped, spaces kept");
        assert!(lines.iter().all(|l| t.width(l, 13.0, Weight::Regular) <= 160.0), "{lines:?}");
    }
}
