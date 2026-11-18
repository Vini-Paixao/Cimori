use super::*;
impl PillRenderer {
    pub(super) fn media_art(&mut self, x: f32, y: f32, size: f32, state: &IslandState) {
        let art = state.media.as_ref().and_then(|m| m.artwork.as_ref());
        if art.is_none() {
            self.media_image = None;
            self.round(x, y, size, size, size * 0.16, surface());
            self.ui_icon(
                Icon::Music,
                x + size / 2.,
                y + size / 2.,
                size * 0.38,
                muted(),
            );
            return;
        }
        let art = art.unwrap();
        if self
            .media_image
            .as_ref()
            .is_none_or(|(a, _)| !std::sync::Arc::ptr_eq(a, art))
        {
            let mut rgba = art.rgba.clone();
            for px in rgba.chunks_exact_mut(4) {
                let a = px[3] as u16;
                for c in &mut px[..3] {
                    *c = (*c as u16 * a / 255) as u8;
                }
            }
            self.media_image = tiny_skia::IntSize::from_wh(art.width, art.height)
                .and_then(|sz| Pixmap::from_vec(rgba, sz))
                .map(|p| (art.clone(), p));
        }
        let Some((_, image)) = &self.media_image else {
            return;
        };
        let scale = size * self.scale / image.width().min(image.height()) as f32;
        let dx = x * self.scale + (size * self.scale - image.width() as f32 * scale) / 2.;
        let dy = y * self.scale + (size * self.scale - image.height() as f32 * scale) / 2.;
        let Some(path) = build_rounded_rect(
            x * self.scale,
            y * self.scale,
            size * self.scale,
            size * self.scale,
            size * 0.16 * self.scale,
        ) else {
            return;
        };
        let Some(mut mask) = Mask::new(self.framebuffer.width, self.framebuffer.height) else {
            return;
        };
        mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
        self.layer.draw_pixmap(
            0,
            0,
            image.as_ref(),
            &PixmapPaint {
                quality: FilterQuality::Bicubic,
                ..Default::default()
            },
            Transform::from_scale(scale, scale).post_translate(dx, dy),
            Some(&mask),
        );
    }
}
