//! Bounded, normalized media artwork. Shared across snapshots without copying full images.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artwork {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
pub const MAX_ENCODED: u64 = 8 * 1024 * 1024;
pub fn valid_encoded_size(size: u64) -> bool {
    size > 0 && size <= MAX_ENCODED
}
pub fn normalized_size(w: u32, h: u32) -> Option<(u32, u32)> {
    if w == 0 || h == 0 || w > 4096 || h > 4096 || (w as u64) * (h as u64) > 16_000_000 {
        return None;
    }
    let k = (512.0 / w.max(h) as f64).min(1.0);
    Some((
        (w as f64 * k).round().max(1.) as u32,
        (h as f64 * k).round().max(1.) as u32,
    ))
}
impl Artwork {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Option<Self> {
        if width == 0
            || height == 0
            || width > 512
            || height > 512
            || rgba.len() != width as usize * height as usize * 4
        {
            return None;
        }
        Some(Self {
            width,
            height,
            rgba,
        })
    }
}
