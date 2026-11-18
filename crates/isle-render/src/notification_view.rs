use super::*;
use isle_core::notifications::Notification;

impl PillRenderer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn notification_label(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        width: f32,
        size: f32,
        weight: Weight,
        color: Color,
    ) {
        #[cfg(windows)]
        if let Some(native) = &mut self.notification_text {
            let rgba = color.to_color_u8();
            if let Some(image) = native.render(
                text,
                width * self.scale,
                size * self.scale,
                weight != Weight::Regular,
                [rgba.red(), rgba.green(), rgba.blue(), rgba.alpha()],
            ) {
                self.layer.draw_pixmap(
                    (x * self.scale).round() as i32,
                    (y * self.scale).round() as i32,
                    image.as_ref(),
                    &PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
                return;
            }
        }
        self.clipped_label(text, x, y, width, size, weight, color);
    }

    pub(super) fn notification_icon(&mut self, n: &Notification, x: f32, y: f32, size: f32) {
        let Some(art) = n.app_icon.as_ref() else {
            self.round(x, y, size, size, size * 0.3, surface());
            self.ui_icon(
                Icon::Message,
                x + size / 2.,
                y + size / 2.,
                size * 0.54,
                muted(),
            );
            return;
        };
        let key = n.app_id.to_lowercase();
        // Same rule as the tray icons: never a blurry fractional stretch. Bigger than the slot is
        // shrunk once with an area average; smaller is drawn at a whole-number scale, pixel for pixel.
        let target = ((size * self.scale).round() as u32).max(1);
        if self
            .notification_images
            .get(&key)
            .is_none_or(|(old, t, _)| !std::sync::Arc::ptr_eq(old, art) || *t != target)
        {
            let (dw, dh, _) = icon_plan(art.width, art.height, target);
            let mut rgba = art.rgba.clone();
            for pixel in rgba.chunks_exact_mut(4) {
                let alpha = pixel[3] as u16;
                for channel in &mut pixel[..3] {
                    *channel = (*channel as u16 * alpha / 255) as u8;
                }
            }
            let rgba = shrink_premultiplied(&rgba, art.width, art.height, dw, dh);
            if let Some(image) = tiny_skia::IntSize::from_wh(dw, dh).and_then(|s| Pixmap::from_vec(rgba, s)) {
                if self.notification_images.len() >= 64 {
                    self.notification_images.clear();
                }
                self.notification_images.insert(key.clone(), (art.clone(), target, image));
            }
        }
        let Some((_, _, image)) = self.notification_images.get(&key) else {
            return;
        };
        let (_, _, k) = icon_plan(art.width, art.height, target);
        let k = k as f32;
        let dx = (x * self.scale + (size * self.scale - image.width() as f32 * k) / 2.).round();
        let dy = (y * self.scale + (size * self.scale - image.height() as f32 * k) / 2.).round();
        self.layer.draw_pixmap(
            0,
            0,
            image.as_ref(),
            &PixmapPaint { quality: FilterQuality::Nearest, ..Default::default() },
            Transform::from_scale(k, k).post_translate(dx, dy),
            None,
        );
    }
}

/// Size of the picture to draw in a `target` px slot, and its whole-number magnification:
/// (width, height, k). Larger sources shrink to fit (k = 1); smaller ones grow by floor(target / source).
pub(crate) fn icon_plan(w: u32, h: u32, target: u32) -> (u32, u32, u32) {
    let big = w.max(h).max(1);
    if big > target {
        let f = |v: u32| ((v as u64 * target as u64 + big as u64 / 2) / big as u64).max(1) as u32;
        (f(w), f(h), 1)
    } else {
        (w, h, (target / big).max(1))
    }
}

/// Area-average shrink of premultiplied RGBA (exact for whole ratios, weighted at the edges otherwise).
pub(crate) fn shrink_premultiplied(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
    if (sw, sh) == (dw, dh) {
        return src.to_vec();
    }
    let mut out = vec![0u8; dw as usize * dh as usize * 4];
    let (rx, ry) = (sw as f32 / dw as f32, sh as f32 / dh as f32);
    for y in 0..dh {
        let (y0, y1) = (y as f32 * ry, (y + 1) as f32 * ry);
        for x in 0..dw {
            let (x0, x1) = (x as f32 * rx, (x + 1) as f32 * rx);
            let (mut acc, mut total) = ([0f32; 4], 0f32);
            for sy in y0.floor() as u32..(y1.ceil() as u32).min(sh) {
                let wy = (y1.min(sy as f32 + 1.) - y0.max(sy as f32)).max(0.);
                for sx in x0.floor() as u32..(x1.ceil() as u32).min(sw) {
                    let wgt = wy * (x1.min(sx as f32 + 1.) - x0.max(sx as f32)).max(0.);
                    let i = ((sy * sw + sx) * 4) as usize;
                    for c in 0..4 {
                        acc[c] += src[i + c] as f32 * wgt;
                    }
                    total += wgt;
                }
            }
            let o = ((y * dw + x) * 4) as usize;
            for c in 0..4 {
                out[o + c] = (acc[c] / total.max(f32::EPSILON)).round() as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn icons_shrink_to_the_slot_or_grow_by_whole_numbers() {
        assert_eq!(icon_plan(128, 128, 32), (32, 32, 1));
        assert_eq!(icon_plan(64, 64, 48), (48, 48, 1));
        assert_eq!(icon_plan(32, 32, 32), (32, 32, 1));
        assert_eq!(icon_plan(32, 32, 48), (32, 32, 1));
        assert_eq!(icon_plan(16, 16, 48), (16, 16, 3));
        assert_eq!(icon_plan(256, 128, 32), (32, 16, 1));
    }
    #[test]
    fn shrinking_averages_instead_of_dropping_pixels() {
        // A 4x4 checkerboard of opaque white/black halves to a flat 50% grey, not black or white.
        let mut src = Vec::new();
        for y in 0..4 {
            for x in 0..4 {
                let v = if (x + y) % 2 == 0 { 255 } else { 0 };
                src.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let out = shrink_premultiplied(&src, 4, 4, 2, 2);
        assert!(out.chunks_exact(4).all(|p| (126..=129).contains(&p[0]) && p[3] == 255));
        assert_eq!(shrink_premultiplied(&src, 4, 4, 4, 4), src);
    }
}
