// SPDX-License-Identifier: GPL-3.0-or-later
// Port of DrawIcon/DrawIconPicture and IconFont (mac-island 3.1.1).
use super::*;
pub(super) use isle_core::icons::Icon;
impl PillRenderer {
    // Port of DrawIcon (mac-island 3.1.1): missing pictures keep the native Windows glyph.
    pub(super) fn ui_icon(&mut self, icon: Icon, cx: f32, cy: f32, size: f32, color: Color) {
        let size_px = (size*self.scale).round().clamp(1.,512.) as u32;
        let c = color.to_color_u8();
        let rgba = [c.red(),c.green(),c.blue(),c.alpha()];
        if let Some(image) = self.icon_cache.render(icon,size_px,rgba) {
            self.layer.draw_pixmap((cx*self.scale-size_px as f32/2.).round() as i32,(cy*self.scale-size_px as f32/2.).round() as i32,
                image.as_ref(),&PixmapPaint::default(),Transform::identity(),None);
            return;
        }
        #[cfg(windows)]
        if let Some(image) = self.notification_text.as_mut().and_then(|t|t.render_glyph(icon.info().glyph,size_px as f32,rgba)) {
            self.layer.draw_pixmap((cx*self.scale-image.width() as f32/2.).round() as i32,(cy*self.scale-image.height() as f32/2.).round() as i32,
                image.as_ref(),&PixmapPaint::default(),Transform::identity(),None);
        }
    }
}
