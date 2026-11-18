//! Notification text uses the same Windows shaping/color-font path as Mac Island & Dock.
//! The rest of Cimori keeps its embedded Inter renderer. Cached rasters are premultiplied RGBA.
#[cfg(windows)]
mod win {
    use std::collections::HashMap;
    use tiny_skia::Pixmap;
    use windows::{
        Win32::{
            Graphics::{
                Direct2D::{Common::*, *},
                DirectWrite::*,
                Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
                Imaging::*,
            },
            System::Com::*,
        },
        core::w,
    };

    struct Apartment(bool);
    impl Drop for Apartment {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() }
            }
        }
    }
    #[derive(Hash, Eq, PartialEq)]
    struct Key {
        content: String,
        width: u32,
        size: u32,
        bold: bool,
        color: [u8; 4],
        icon: bool,
    }
    pub struct NotificationText {
        write: IDWriteFactory,
        draw: ID2D1Factory,
        wic: IWICImagingFactory,
        cache: HashMap<Key, Pixmap>,
        bytes: usize,
        _com: Apartment,
    }
    impl NotificationText {
        pub fn new() -> anyhow::Result<Self> {
            unsafe {
                let status = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                // An existing MTA is valid too; only balance successful initialization calls.
                if status.is_err() && status.0 != 0x80010106u32 as i32 {
                    status.ok()?;
                }
                let com = Apartment(status.is_ok());
                Ok(Self {
                    write: DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?,
                    draw: D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?,
                    wic: CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?,
                    cache: HashMap::new(),
                    bytes: 0,
                    _com: com,
                })
            }
        }
        /// Physical-pixel width and em size. Native trimming handles complete Unicode clusters.
        pub fn render(
            &mut self,
            content: &str,
            width: f32,
            size: f32,
            bold: bool,
            color: [u8; 4],
        ) -> Option<&Pixmap> {
            if !width.is_finite() || !size.is_finite() || width < 1. || size <= 0. || size > 200. {
                return None;
            }
            let key = Key {
                content: content.replace(['\r', '\n'], " "),
                width: width.floor().min(4096.) as u32,
                size: size.to_bits(),
                bold,
                color,
                icon: false,
            };
            if !self.cache.contains_key(&key) {
                let image = self.raster(&key).ok()?;
                if self.bytes + image.data().len() > 8 * 1024 * 1024 || self.cache.len() >= 48 {
                    self.cache.clear();
                    self.bytes = 0;
                }
                self.bytes += image.data().len();
                self.cache.insert(key.clone_key(), image);
            }
            self.cache.get(&key)
        }
        // Port of IconFont (mac-island 3.1.1): Fluent on 11, MDL2 when it is absent on 10.
        fn icon_font(&self) -> windows::core::PCWSTR {
            unsafe {
                let mut fonts = None;
                let mut index = 0;
                let mut exists = windows::Win32::Foundation::BOOL(0);
                if self.write.GetSystemFontCollection(&mut fonts, false).is_ok() {
                    if let Some(fonts) = fonts {
                        let _ = fonts.FindFamilyName(w!("Segoe Fluent Icons"), &mut index, &mut exists);
                    }
                }
                if exists.as_bool() { w!("Segoe Fluent Icons") } else { w!("Segoe MDL2 Assets") }
            }
        }
        // Port of DrawIcon (mac-island 3.1.1), on the existing DirectWrite/WIC text path.
        pub fn render_glyph(&mut self, glyph: u32, size: f32, color: [u8;4]) -> Option<&Pixmap> {
            if !size.is_finite() || size <= 0. || size > 512. { return None; }
            let key = Key { content: char::from_u32(glyph)?.to_string(), width: (size*2.+4.).ceil() as u32,
                size: size.to_bits(), bold: false, color, icon: true };
            if !self.cache.contains_key(&key) {
                let image = self.raster(&key).ok()?;
                let (mut left,mut top,mut right,mut bottom) = (image.width(),image.height(),0,0);
                for y in 0..image.height() { for x in 0..image.width() {
                    if image.pixel(x,y)?.alpha() > 0 { left=left.min(x);top=top.min(y);right=right.max(x+1);bottom=bottom.max(y+1); }
                } }
                if right <= left || bottom <= top { return None; }
                let rect = tiny_skia::IntRect::from_xywh(left as i32,top as i32,right-left,bottom-top)?;
                let image = image.clone_rect(rect)?;
                if self.bytes+image.data().len() > 8*1024*1024 || self.cache.len() >= 256 { self.cache.clear(); self.bytes=0; }
                self.bytes += image.data().len(); self.cache.insert(key.clone_key(),image);
            }
            self.cache.get(&key)
        }
        fn raster(&self, key: &Key) -> anyhow::Result<Pixmap> {
            unsafe {
                let size = f32::from_bits(key.size);
                let height = (size * 1.7 + 2.).ceil() as u32;
                let bitmap = self.wic.CreateBitmap(
                    key.width,
                    height,
                    &GUID_WICPixelFormat32bppPBGRA,
                    WICBitmapCacheOnLoad,
                )?;
                let target = self.draw.CreateWicBitmapRenderTarget(
                    &bitmap,
                    &D2D1_RENDER_TARGET_PROPERTIES {
                        r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                        pixelFormat: D2D1_PIXEL_FORMAT {
                            format: DXGI_FORMAT_B8G8R8A8_UNORM,
                            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                        },
                        dpiX: 96.,
                        dpiY: 96.,
                        usage: D2D1_RENDER_TARGET_USAGE_NONE,
                        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
                    },
                )?;
                let format = self.write.CreateTextFormat(
                    if key.icon { self.icon_font() } else { w!("Segoe UI") },
                    None,
                    if key.bold {
                        DWRITE_FONT_WEIGHT_SEMI_BOLD
                    } else {
                        DWRITE_FONT_WEIGHT_NORMAL
                    },
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size,
                    w!("pt-BR"),
                )?;
                format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                let utf16: Vec<u16> = key.content.encode_utf16().collect();
                let layout = self.write.CreateTextLayout(
                    &utf16,
                    &format,
                    key.width as f32,
                    height as f32,
                )?;
                let ellipsis = self.write.CreateEllipsisTrimmingSign(&format)?;
                layout.SetTrimming(
                    &DWRITE_TRIMMING {
                        granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                        delimiter: 0,
                        delimiterCount: 0,
                    },
                    &ellipsis,
                )?;
                let brush = target.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: key.color[0] as f32 / 255.,
                        g: key.color[1] as f32 / 255.,
                        b: key.color[2] as f32 / 255.,
                        a: key.color[3] as f32 / 255.,
                    },
                    None,
                )?;
                target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
                target.BeginDraw();
                target.Clear(Some(&D2D1_COLOR_F {
                    r: 0.,
                    g: 0.,
                    b: 0.,
                    a: 0.,
                }));
                target.DrawTextLayout(
                    D2D_POINT_2F { x: 0., y: 0. },
                    &layout,
                    &brush,
                    D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT | D2D1_DRAW_TEXT_OPTIONS_CLIP,
                );
                target.EndDraw(None, None)?;
                let mut pixels = vec![0; key.width as usize * height as usize * 4];
                bitmap.CopyPixels(std::ptr::null(), key.width * 4, &mut pixels)?;
                for p in pixels.chunks_exact_mut(4) {
                    p.swap(0, 2);
                }
                Pixmap::from_vec(
                    pixels,
                    tiny_skia::IntSize::from_wh(key.width, height).unwrap(),
                )
                .ok_or_else(|| anyhow::anyhow!("invalid notification text raster"))
            }
        }
    }
    impl Key {
        fn clone_key(&self) -> Self {
            Self {
                content: self.content.clone(),
                width: self.width,
                size: self.size,
                bold: self.bold,
                color: self.color,
                icon: self.icon,
            }
        }
    }
}
#[cfg(windows)]
pub use win::NotificationText;
