// SPDX-License-Identifier: GPL-3.0-or-later
//! Port of ReadIconPicture/DrawIconPicture (mac-island 3.1.1), using resvg and tiny-skia.
use isle_core::{config::IsleConfig, icons::{Icon, IconImages, IconPack}};
use std::{collections::HashMap, sync::{Arc, Mutex}};
use tiny_skia::{Pixmap, PixmapPaint, Transform};

static IMAGES: Mutex<Option<Arc<IconImages>>> = Mutex::new(None);
pub fn set_images(images: Arc<IconImages>) { *IMAGES.lock().unwrap() = Some(images); }

#[derive(Default)]
pub(crate) struct IconCache {
    source: Option<Arc<IconImages>>,
    rasters: HashMap<(Icon, u32, [u8;4]), Option<Pixmap>>,
    bytes: usize,
}
impl IconCache {
    pub fn prepare(&mut self, cfg: &IsleConfig) {
        let source = IMAGES.lock().unwrap().as_ref().filter(|s| s.pack == cfg.icon_pack && s.folder == cfg.icon_folder).cloned();
        let same = match (&source, &self.source) { (Some(a),Some(b)) => Arc::ptr_eq(a,b), (None,None) => true, _ => false };
        if !same { self.source = source; self.rasters.clear(); self.bytes = 0; }
    }
    pub fn render(&mut self, icon: Icon, size: u32, color: [u8;4]) -> Option<&Pixmap> {
        let key = (icon,size,color);
        if !self.rasters.contains_key(&key) {
            let source = self.source.as_ref()?;
            let data = source.pictures.get(&icon.info().glyph)?;
            let image = raster_picture(data,size,color,source.pack != IconPack::Folder);
            let bytes = image.as_ref().map_or(0,|p|p.data().len());
            if self.bytes + bytes > 16*1024*1024 || self.rasters.len() >= 512 { self.rasters.clear(); self.bytes = 0; }
            self.bytes += bytes;
            self.rasters.insert(key,image);
        }
        self.rasters.get(&key)?.as_ref()
    }
}

// Port of ReadIconPicture (mac-island 3.1.1): aspect fit, physical-pixel raster and shape tint.
pub fn raster_picture(data: &[u8], size: u32, color: [u8;4], tint: bool) -> Option<Pixmap> {
    if data.len() > 2*1024*1024 || !(1..=512).contains(&size) { return None; }
    let mut output = Pixmap::new(size,size)?;
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        if data.len() < 24 { return None; }
        let w = u32::from_be_bytes(data[16..20].try_into().ok()?);
        let h = u32::from_be_bytes(data[20..24].try_into().ok()?);
        if w == 0 || h == 0 || w > 2048 || h > 2048 { return None; }
        let image = Pixmap::decode_png(data).ok()?;
        let k = size as f32 / w.max(h) as f32;
        let transform = Transform::from_scale(k,k).post_translate((size as f32-w as f32*k)/2.,(size as f32-h as f32*k)/2.);
        output.draw_pixmap(0,0,image.as_ref(),&PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() },transform,None);
    } else {
        let mut options = resvg::usvg::Options::default();
        options.image_href_resolver.resolve_string = Box::new(|_,_| None);
        let tree = resvg::usvg::Tree::from_data(data,&options).ok()?;
        let wh = tree.size();
        let k = size as f32 / wh.width().max(wh.height());
        let transform = Transform::from_scale(k,k).post_translate((size as f32-wh.width()*k)/2.,(size as f32-wh.height()*k)/2.);
        resvg::render(&tree,transform,&mut output.as_mut());
    }
    if tint {
        for px in output.data_mut().chunks_exact_mut(4) {
            let alpha = (u32::from(px[3])*u32::from(color[3])+127)/255;
            for i in 0..3 { px[i] = ((u32::from(color[i])*alpha+127)/255) as u8; }
            px[3] = alpha as u8;
        }
    }
    // Port of IconPictureShows (mac-island 3.1.1): empty/transparent pictures keep Windows' glyph.
    output.data().chunks_exact(4).any(|px|px[3]>0).then_some(output)
}
