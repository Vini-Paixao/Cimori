// SPDX-License-Identifier: GPL-3.0-or-later
//! Native clipboard adapter for isle's GPL-3.0-or-later clipboard history model.

use std::io::Cursor;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use isle_core::clipboard::MAX_TEXT_BYTES;
use reqwest::Url;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData,
    GetClipboardOwner, GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, RemoveClipboardFormatListener, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GHND};
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

// Winuser.h clipboard formats. Their windows-rs constants require the OLE feature.
const CF_DIB: u32 = 8;
const CF_UNICODETEXT: u32 = 13;
const CF_HDROP: u32 = 15;
const CF_DIBV5: u32 = 17;

const MAX_HDROP_BYTES: usize = 512 * 1024;
const MAX_FILES: usize = 4096;
const DROPFILES_SIZE: usize = 20;
const MAX_IMAGE_BYTES: usize = 40 * 1024 * 1024;
const MAX_IMAGE_PIXELS: usize = 8 * 1024 * 1024;
const MAX_IMAGE_SIDE: u32 = 8192;
const BITMAPV5_SIZE: usize = 124;
const BI_RGB: u32 = 0;
const BI_BITFIELDS: u32 = 3;
const SRGB_COLOR_SPACE: u32 = 0x7352_4742;

/// The content of a native copy, using the formats expected by isle's clipboard history.
pub enum ClipboardPayload {
    Text(String),
    Files(Vec<String>),
    Image {
        png: Vec<u8>,
        width: u32,
        height: u32,
    },
}

pub struct ClipboardCapture {
    pub sequence: u32,
    pub payload: ClipboardPayload,
    /// Copied by the Snipping Tool, which shows its own notice.
    pub screenshot: bool,
}

/// Delivers WM_CLIPBOARDUPDATE to an existing window. Drop before destroying the window.
pub struct ClipboardListener(HWND);

impl ClipboardListener {
    pub fn register(hwnd: HWND) -> Result<Self> {
        unsafe { AddClipboardFormatListener(hwnd) }.context("register clipboard listener")?;
        Ok(Self(hwnd))
    }
}

impl Drop for ClipboardListener {
    fn drop(&mut self) {
        let _ = unsafe { RemoveClipboardFormatListener(self.0) };
    }
}

/// Opens the clipboard for one notification. Opt-outs are inspected before accessing its content.
/// Reads can fail temporarily when another window holds the clipboard open.
pub fn capture(hwnd: HWND) -> Result<Option<ClipboardCapture>> {
    capture_ignoring(hwnd, &[])
}

// Port of clipboard.js clipboard-ignored-apps: opt out before reading any content.
pub fn capture_ignoring(hwnd: HWND, ignored: &[String]) -> Result<Option<ClipboardCapture>> {
    let exclude = registered_format(w!("ExcludeClipboardContentFromMonitorProcessing"))?;
    let history = registered_format(w!("CanIncludeInClipboardHistory"))?;
    let png = registered_format(w!("PNG")).ok();

    let clipboard = OpenClipboardGuard::open(hwnd)?;
    let owner = unsafe { GetClipboardOwner() }.ok();
    if is_ignored(&window_executable(owner.or_else(|| unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() }.into())), ignored) {
        return Ok(None);
    }
    if unsafe { GetClipboardOwner() }.ok() == Some(hwnd)
        || has_format(exclude)
        || (has_format(history)
            && !read_clipboard_bytes(history, 64, |_, bytes| Some(history_opt_in(bytes)))
                .unwrap_or(false))
    {
        return Ok(None);
    }

    // Files, then text, then a picture, as isle's clipboard.js and Mac Island's OnClipboardChanged:
    // a spreadsheet's cells come with a picture of them. Only a picture's bare address (a browser's
    // Copy Image) yields to the picture.
    let has_image = png.is_some_and(has_format) || has_format(CF_DIBV5) || has_format(CF_DIB);
    let text = || {
        read_clipboard_bytes(CF_UNICODETEXT, (MAX_TEXT_BYTES + 1) * 2 + 16, |_, bytes| {
            decode_text(bytes)
        })
        .filter(|text| !text.trim().is_empty() && !(has_image && is_image_address(text)))
        .map(|text| CapturedFormat::Ready(ClipboardPayload::Text(text)))
    };
    let raw = read_clipboard_bytes(CF_HDROP, MAX_HDROP_BYTES, read_hdrop)
        .map(ClipboardPayload::Files)
        .map(CapturedFormat::Ready)
        .or_else(text)
        .or_else(|| {
            let format = png?;
            read_clipboard_bytes(format, MAX_IMAGE_BYTES, |_, bytes| {
                let end = png_end(bytes)?;
                Some(CapturedFormat::Png(bytes[..end].to_vec()))
            })
        })
        .or_else(|| {
            [CF_DIBV5, CF_DIB].into_iter().find_map(|format| {
                read_clipboard_bytes(format, MAX_IMAGE_BYTES, |_, bytes| {
                    let layout = DibLayout::parse(bytes)?;
                    Some(CapturedFormat::Dib(bytes[..layout.end].to_vec()))
                })
            })
        });
    let sequence = unsafe { GetClipboardSequenceNumber() };
    let screenshot = is_screenshot_tool(unsafe { GetClipboardOwner() }.ok());
    drop(clipboard);

    Ok(raw
        .and_then(CapturedFormat::into_payload)
        .map(|payload| ClipboardCapture {
            sequence,
            payload,
            screenshot,
        }))
}

/// isle's `/^\s*(https?|data|file):\S*\s*$/`: only a link, as a browser's Copy Image gives with it.
fn is_image_address(text: &str) -> bool {
    let text = text.trim();
    ["http:", "https:", "data:", "file:"]
        .iter()
        .any(|scheme| text.get(..scheme.len()).is_some_and(|s| s.eq_ignore_ascii_case(scheme)))
        && !text.contains(char::is_whitespace)
}

/// The Snipping Tool (Win+Shift+S) owns the copy: it says so in its own banner (Mac Island skips it).
fn is_screenshot_tool(owner: Option<HWND>) -> bool {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    let Some(owner) = owner else { return false };
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(owner, Some(&mut pid)) };
    if pid == 0 {
        return false;
    }
    // The handle is closed below; the buffer outlives the call.
    let path = unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(handle);
        if ok.is_err() {
            return false;
        }
        String::from_utf16_lossy(&buf[..len as usize]).to_lowercase()
    };
    ["screensketch", "screenclipping", "snippingtool"]
        .iter()
        .any(|name| path.contains(name))
}

/// Publishes a saved copy. A successful write makes this window the clipboard owner.
pub fn restore(hwnd: HWND, payload: &ClipboardPayload) -> Result<()> {
    let (primary, extra) = match payload {
        ClipboardPayload::Text(text) => {
            ensure!(
                text.len() <= MAX_TEXT_BYTES && !text.is_empty() && !text.contains('\0'),
                "invalid clipboard text"
            );
            let bytes = text
                .encode_utf16()
                .chain([0])
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>();
            ((CF_UNICODETEXT, ClipboardMemory::new(&bytes)?), None)
        }
        ClipboardPayload::Files(uris) => {
            let bytes = encode_hdrop(uris).context("invalid clipboard file list")?;
            // Restoring a history entry is a copy, including entries captured from a cut.
            let effect = registered_format(w!("Preferred DropEffect"))?;
            let copy = ClipboardMemory::new(&1_u32.to_le_bytes())?; // DROPEFFECT_COPY
            (
                (CF_HDROP, ClipboardMemory::new(&bytes)?),
                Some((effect, copy)),
            )
        }
        ClipboardPayload::Image { png, width, height } => {
            let image = decode_png(png).context("invalid clipboard PNG")?;
            ensure!(
                (image.width, image.height) == (*width, *height),
                "invalid clipboard image dimensions"
            );
            let primary = (CF_DIBV5, ClipboardMemory::new(&encode_dibv5(&image))?);
            let extra = if let Ok(format) = registered_format(w!("PNG")) {
                let end = png_end(png).context("invalid clipboard PNG")?;
                Some((format, ClipboardMemory::new(&png[..end])?))
            } else {
                None
            };
            (primary, extra)
        }
    };

    let _clipboard = OpenClipboardGuard::open(hwnd)?;
    unsafe { EmptyClipboard() }.context("empty clipboard before restore")?;
    primary.1.publish(primary.0)?;
    if let Some((format, data)) = extra {
        data.publish(format)?;
    }
    Ok(())
}

/// Converts a Windows drive or UNC path into an escaped URI for isle's clipboard or shelf.
pub fn file_uri(path: &Path) -> Option<String> {
    if path.as_os_str().encode_wide().any(|unit| unit == 0) {
        return None;
    }
    Some(Url::from_file_path(path).ok()?.into())
}

/// Resolves an isle file URI to a Windows drive or UNC path.
pub fn file_path(uri: &str) -> Option<PathBuf> {
    if !uri.starts_with("file://") {
        return None;
    }
    let url = Url::parse(uri).ok()?;
    if url.query().is_some() || url.fragment().is_some() {
        return None;
    }
    let path = url.to_file_path().ok()?;
    (!path.as_os_str().encode_wide().any(|unit| unit == 0)).then_some(path)
}

struct OpenClipboardGuard;

impl OpenClipboardGuard {
    fn open(hwnd: HWND) -> Result<Self> {
        unsafe { OpenClipboard(hwnd) }.context("open clipboard")?;
        Ok(Self)
    }
}

impl Drop for OpenClipboardGuard {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

/// Owns a moveable allocation until Windows takes ownership in SetClipboardData.
struct ClipboardMemory(HGLOBAL);

impl ClipboardMemory {
    fn new(bytes: &[u8]) -> Result<Self> {
        let memory = Self(unsafe { GlobalAlloc(GHND, bytes.len()) }?);
        let lock = GlobalLockGuard::lock(memory.0).context("lock clipboard memory for restore")?;
        // The allocation is at least bytes.len() long and exclusively owned here.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), lock.ptr, bytes.len()) };
        Ok(memory)
    }

    fn publish(mut self, format: u32) -> Result<()> {
        let HGLOBAL(handle) = self.0;
        unsafe { SetClipboardData(format, HANDLE(handle)) }.context("publish clipboard data")?;
        self.0 = HGLOBAL::default();
        Ok(())
    }
}

impl Drop for ClipboardMemory {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { GlobalFree(self.0) };
        }
    }
}

struct GlobalLockGuard {
    handle: HGLOBAL,
    ptr: *mut u8,
}

impl GlobalLockGuard {
    fn lock(handle: HGLOBAL) -> Option<Self> {
        let ptr = unsafe { GlobalLock(handle) }.cast::<u8>();
        if ptr.is_null() {
            return None;
        }
        Some(Self { handle, ptr })
    }
}

impl Drop for GlobalLockGuard {
    fn drop(&mut self) {
        let _ = unsafe { GlobalUnlock(self.handle) };
    }
}

fn registered_format(name: PCWSTR) -> Result<u32> {
    let format = unsafe { RegisterClipboardFormatW(name) };
    ensure!(format != 0, "register clipboard format");
    Ok(format)
}

fn has_format(format: u32) -> bool {
    unsafe { IsClipboardFormatAvailable(format) }.is_ok()
}

fn read_clipboard_bytes<T>(
    format: u32,
    limit: usize,
    read: impl FnOnce(HGLOBAL, &[u8]) -> Option<T>,
) -> Option<T> {
    if !has_format(format) {
        return None;
    }
    let handle = HGLOBAL(unsafe { GetClipboardData(format) }.ok()?.0);
    let size = unsafe { GlobalSize(handle) };
    if size == 0 || size > limit {
        return None;
    }
    let lock = GlobalLockGuard::lock(handle)?;
    // The caller holds the clipboard open and the allocation is locked for the entire read.
    let bytes = unsafe { std::slice::from_raw_parts(lock.ptr, size) };
    read(handle, bytes)
}

fn history_opt_in(bytes: &[u8]) -> bool {
    le_u32(bytes, 0) == Some(1)
}

fn decode_text(bytes: &[u8]) -> Option<String> {
    let mut wide = Vec::new();
    for pair in bytes.chunks_exact(2) {
        let unit = u16::from_le_bytes([pair[0], pair[1]]);
        if unit == 0 {
            let text = String::from_utf16(&wide).ok()?;
            return (text.len() <= MAX_TEXT_BYTES && !text.is_empty()).then_some(text);
        }
        wide.push(unit);
    }
    None
}

fn read_hdrop(handle: HGLOBAL, bytes: &[u8]) -> Option<Vec<String>> {
    let offset = le_u32(bytes, 0)? as usize;
    if offset < DROPFILES_SIZE {
        return None;
    }
    let files = bytes.get(offset..)?;
    if le_u32(bytes, 16)? != 0 {
        return if offset % 2 == 0 {
            wide_file_list(files)
        } else {
            None
        };
    }

    // Older apps may supply ANSI names. Verify the in-bounds double terminator before
    // letting the shell decode that list with the system code page.
    let end = files.windows(2).position(|pair| pair == [0, 0])?;
    if end == 0 || files[0] == 0 {
        return None;
    }
    let count = files[..end].iter().filter(|byte| **byte == 0).count() + 1;
    if count > MAX_FILES {
        return None;
    }
    let drop = HDROP(handle.0);
    if unsafe { DragQueryFileW(drop, u32::MAX, None) } as usize != count {
        return None;
    }
    (0..count as u32)
        .map(|index| {
            let length = unsafe { DragQueryFileW(drop, index, None) } as usize;
            if length == 0 || length > 32_767 {
                return None;
            }
            let mut wide = vec![0; length + 1];
            if unsafe { DragQueryFileW(drop, index, Some(&mut wide)) } as usize != length {
                return None;
            }
            let path = String::from_utf16(&wide[..length]).ok()?;
            file_uri(Path::new(&path))
        })
        .collect()
}

fn wide_file_list(bytes: &[u8]) -> Option<Vec<String>> {
    let mut wide = Vec::new();
    let mut uris = Vec::new();
    for pair in bytes.chunks_exact(2) {
        let unit = u16::from_le_bytes([pair[0], pair[1]]);
        if unit != 0 {
            if wide.len() == 32_767 {
                return None;
            }
            wide.push(unit);
            continue;
        }
        if wide.is_empty() {
            return (!uris.is_empty()).then_some(uris);
        }
        if uris.len() == MAX_FILES {
            return None;
        }
        let path = String::from_utf16(&wide).ok()?;
        uris.push(file_uri(Path::new(&path))?);
        wide.clear();
    }
    None
}

fn encode_hdrop(uris: &[String]) -> Option<Vec<u8>> {
    if uris.is_empty() || uris.len() > MAX_FILES {
        return None;
    }
    let mut bytes = vec![0; DROPFILES_SIZE];
    bytes[0..4].copy_from_slice(&(DROPFILES_SIZE as u32).to_le_bytes());
    bytes[16..20].copy_from_slice(&1_u32.to_le_bytes());
    for uri in uris {
        let path = file_path(uri)?;
        let wide = path.as_os_str().encode_wide().collect::<Vec<_>>();
        if wide.len() > 32_767 || bytes.len() + (wide.len() + 2) * 2 > MAX_HDROP_BYTES {
            return None;
        }
        bytes.extend(wide.into_iter().chain([0]).flat_map(u16::to_le_bytes));
    }
    bytes.extend_from_slice(&[0, 0]);
    Some(bytes)
}

enum CapturedFormat {
    Ready(ClipboardPayload),
    Png(Vec<u8>),
    Dib(Vec<u8>),
}

impl CapturedFormat {
    fn into_payload(self) -> Option<ClipboardPayload> {
        match self {
            Self::Ready(payload) => Some(payload),
            Self::Png(png) => {
                let (info, _) = read_png(&png)?;
                Some(ClipboardPayload::Image {
                    png,
                    width: info.width,
                    height: info.height,
                })
            }
            Self::Dib(dib) => {
                let image = decode_dib(&dib)?;
                let png = encode_png(&image)?;
                Some(ClipboardPayload::Image {
                    png,
                    width: image.width,
                    height: image.height,
                })
            }
        }
    }
}

struct RgbaImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

fn image_pixels(width: u32, height: u32) -> Option<usize> {
    if width == 0 || height == 0 || width > MAX_IMAGE_SIDE || height > MAX_IMAGE_SIDE {
        return None;
    }
    let pixels = width as usize * height as usize;
    (pixels <= MAX_IMAGE_PIXELS).then_some(pixels)
}

/// The allocation may include padding. Keep only the PNG up to and including its IEND.
fn png_end(bytes: &[u8]) -> Option<usize> {
    if bytes.len() > MAX_IMAGE_BYTES || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    let mut offset = 8;
    while let Some(header) = bytes.get(offset..offset + 8) {
        let length = u32::from_be_bytes(header[..4].try_into().ok()?) as usize;
        offset = offset.checked_add(length)?.checked_add(12)?;
        if offset > bytes.len() {
            return None;
        }
        if &header[4..] == b"IEND" {
            return (length == 0).then_some(offset);
        }
    }
    None
}

fn read_png(bytes: &[u8]) -> Option<(png::OutputInfo, Vec<u8>)> {
    let end = png_end(bytes)?;
    let mut decoder = png::Decoder::new_with_limits(
        Cursor::new(&bytes[..end]),
        png::Limits {
            bytes: MAX_IMAGE_BYTES,
        },
    );
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let info = reader.info();
    image_pixels(info.width, info.height)?;
    if info.animation_control.is_some() || reader.output_buffer_size() > MAX_IMAGE_PIXELS * 4 {
        return None;
    }
    let mut pixels = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut pixels).ok()?;
    reader.finish().ok()?;
    pixels.truncate(frame.buffer_size());
    Some((frame, pixels))
}

fn decode_png(bytes: &[u8]) -> Option<RgbaImage> {
    let (frame, pixels) = read_png(bytes)?;
    let mut rgba = Vec::with_capacity(image_pixels(frame.width, frame.height)? * 4);
    match frame.color_type {
        png::ColorType::Rgba => rgba = pixels,
        png::ColorType::Rgb => {
            for pixel in pixels.chunks_exact(3) {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
        png::ColorType::Grayscale => {
            for gray in pixels {
                rgba.extend_from_slice(&[gray, gray, gray, 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for pixel in pixels.chunks_exact(2) {
                rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
        }
        png::ColorType::Indexed => return None,
    }
    Some(RgbaImage {
        width: frame.width,
        height: frame.height,
        rgba,
    })
}

fn encode_png(image: &RgbaImage) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, image.width, image.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_compression(png::Compression::Fast);
        encoder
            .write_header()
            .ok()?
            .write_image_data(&image.rgba)
            .ok()?;
    }
    (bytes.len() <= MAX_IMAGE_BYTES).then_some(bytes)
}

struct DibLayout {
    width: u32,
    height: u32,
    top_down: bool,
    stride: usize,
    bytes_per_pixel: usize,
    offset: usize,
    end: usize,
    masks: Option<[u32; 4]>,
}

impl DibLayout {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let header = le_u32(bytes, 0)? as usize;
        if !matches!(header, 40 | 52 | 56 | 108 | 124) || header > bytes.len() {
            return None;
        }
        let width = u32::try_from(le_u32(bytes, 4)? as i32).ok()?;
        let signed_height = le_u32(bytes, 8)? as i32;
        let height = signed_height.unsigned_abs();
        image_pixels(width, height)?;
        if le_u16(bytes, 12)? != 1 {
            return None;
        }
        let depth = le_u16(bytes, 14)?;
        let compression = le_u32(bytes, 16)?;
        let (mask_bytes, masks) = match (compression, depth) {
            (BI_RGB, 16) => (0, Some([0x7C00, 0x03E0, 0x001F, 0])),
            (BI_RGB, 24 | 32) => (0, None),
            (BI_BITFIELDS, 16 | 32) => {
                let masks = [
                    le_u32(bytes, 40)?,
                    le_u32(bytes, 44)?,
                    le_u32(bytes, 48)?,
                    if header >= 56 { le_u32(bytes, 52)? } else { 0 },
                ];
                if !valid_masks(masks, depth) {
                    return None;
                }
                (if header == 40 { 12 } else { 0 }, Some(masks))
            }
            _ => return None,
        };
        let bytes_per_pixel = depth as usize / 8;
        let stride = (width as usize * bytes_per_pixel + 3) & !3;
        let table_bytes = (le_u32(bytes, 32)? as usize).checked_mul(4)?;
        let offset = (header + mask_bytes).checked_add(table_bytes)?;
        let end = offset.checked_add(stride * height as usize)?;
        if end > bytes.len() {
            return None;
        }
        Some(Self {
            width,
            height,
            top_down: signed_height < 0,
            stride,
            bytes_per_pixel,
            offset,
            end,
            masks,
        })
    }
}

fn valid_masks(masks: [u32; 4], depth: u16) -> bool {
    if masks[..3].contains(&0) {
        return false;
    }
    let mut used = 0;
    for mask in masks.into_iter().filter(|mask| *mask != 0) {
        let normalized = mask >> mask.trailing_zeros();
        if normalized & normalized.wrapping_add(1) != 0
            || mask & used != 0
            || mask.leading_zeros() < 32 - u32::from(depth)
        {
            return false;
        }
        used |= mask;
    }
    true
}

fn decode_dib(bytes: &[u8]) -> Option<RgbaImage> {
    let layout = DibLayout::parse(bytes)?;
    let mut rgba = Vec::with_capacity(layout.width as usize * layout.height as usize * 4);
    let row_bytes = layout.width as usize * layout.bytes_per_pixel;
    for row in 0..layout.height as usize {
        let physical_row = if layout.top_down {
            row
        } else {
            layout.height as usize - 1 - row
        };
        let start = layout.offset + physical_row * layout.stride;
        for pixel in bytes[start..start + row_bytes].chunks_exact(layout.bytes_per_pixel) {
            if let Some(masks) = layout.masks {
                let mut packed = [0; 4];
                packed[..pixel.len()].copy_from_slice(pixel);
                let packed = u32::from_le_bytes(packed);
                rgba.extend(masks.map(|mask| {
                    if mask == 0 {
                        return 255;
                    }
                    let shift = mask.trailing_zeros();
                    let value = u64::from((packed & mask) >> shift);
                    let max = u64::from(mask >> shift);
                    ((value * 255 + max / 2) / max) as u8
                }));
            } else {
                // The high byte in a 32-bit BI_RGB pixel is unspecified.
                rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
            }
        }
    }
    Some(RgbaImage {
        width: layout.width,
        height: layout.height,
        rgba,
    })
}

fn encode_dibv5(image: &RgbaImage) -> Vec<u8> {
    let mut dib = vec![0; BITMAPV5_SIZE];
    dib[0..4].copy_from_slice(&(BITMAPV5_SIZE as u32).to_le_bytes());
    dib[4..8].copy_from_slice(&image.width.to_le_bytes());
    dib[8..12].copy_from_slice(&(-(image.height as i32)).to_le_bytes());
    dib[12..14].copy_from_slice(&1_u16.to_le_bytes());
    dib[14..16].copy_from_slice(&32_u16.to_le_bytes());
    dib[16..20].copy_from_slice(&BI_BITFIELDS.to_le_bytes());
    dib[20..24].copy_from_slice(&(image.rgba.len() as u32).to_le_bytes());
    dib[40..44].copy_from_slice(&0x00FF_0000_u32.to_le_bytes());
    dib[44..48].copy_from_slice(&0x0000_FF00_u32.to_le_bytes());
    dib[48..52].copy_from_slice(&0x0000_00FF_u32.to_le_bytes());
    dib[52..56].copy_from_slice(&0xFF00_0000_u32.to_le_bytes());
    dib[56..60].copy_from_slice(&SRGB_COLOR_SPACE.to_le_bytes());
    dib.reserve_exact(image.rgba.len());
    for pixel in image.rgba.chunks_exact(4) {
        dib.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }
    dib
}

fn le_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn le_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_store_round_trips_images_and_prunes_missing_files() {
        let dir = std::env::temp_dir().join(format!("cimori-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("exists.txt");
        std::fs::write(&path, "kept").unwrap();
        let mut shelf = isle_core::shelf::Shelf::default();
        shelf.add(isle_core::shelf::ShelfItem { uri: file_uri(&path).unwrap(), name: "exists.txt".into(), added_at_ms: 0 });
        let png = encode_png(&RgbaImage { width: 2, height: 1, rgba: vec![255; 8] }).unwrap();
        let hash = isle_core::clipboard::content_hash(&png);
        let mut history = isle_core::clipboard::ClipboardHistory::default();
        history.add(isle_core::clipboard::Clip::Image { hash: hash.clone(), width: 2, height: 1 }, "paint.exe", 0);
        let images = [(hash.clone(), std::sync::Arc::new(png.clone()))].into_iter().collect();
        let mut store = DiskStore::at(dir.clone());
        store.save(&history, &shelf, &images, true, true).unwrap();
        let loaded = store.load(true, true);
        assert_eq!(*loaded.images[&hash], png);
        assert_eq!(loaded.shelf.items().len(), 1);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(dir.join("clips").join(format!("{hash}.png"))).unwrap();
        let loaded = store.load(true, true);
        assert!(loaded.shelf.items().is_empty());
        assert!(loaded.history.items().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn disk_store_preserves_a_corrupt_index_and_does_not_delete_its_images() {
        let dir = std::env::temp_dir().join(format!("cimori-corrupt-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("clips")).unwrap();
        std::fs::write(dir.join("clipboard.json"), b"{broken").unwrap();
        std::fs::write(dir.join("clips/old.png"), b"untouched").unwrap();
        let mut store = DiskStore::at(dir.clone());
        store.load(true, false);
        assert!(dir.join("clipboard.json.corrupt").exists());
        assert!(dir.join("clips/old.png").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dropping_the_store_flushes_the_last_queued_snapshot_before_returning() {
        let dir = std::env::temp_dir().join(format!("cimori-flush-{}", std::process::id()));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let worker = StoreWorker::start(dir.clone(), true, true, move |_| {
            ready_tx.send(()).unwrap();
            resume_rx.recv().unwrap();
        });
        ready_rx.recv().unwrap();
        let mut state = Stored { keep_clipboard: true, ..Default::default() };
        state.history.add(isle_core::clipboard::Clip::from_text("última cópia").unwrap(), "", 0);
        worker.save(state);
        let resume = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            resume_tx.send(()).unwrap();
        });
        drop(worker);
        let saved_before_return = std::fs::read_to_string(dir.join("clipboard.json")).ok();
        resume.join().unwrap();
        // Let the unfixed worker exit too, so the failure leaves no temporary file.
        let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !dir.join("clipboard.json").exists() && std::time::Instant::now() < until { std::thread::yield_now(); }
        let _ = std::fs::remove_dir_all(dir);
        assert!(saved_before_return.is_some_and(|saved| saved.contains("última cópia")));
    }

    #[test]
    fn thumbnails_crop_to_the_isle_card_and_do_not_upscale_small_images() {
        let png = encode_png(&RgbaImage { width: 1000, height: 500, rgba: vec![255; 1000 * 500 * 4] }).unwrap();
        let thumb = thumbnail(&png).unwrap();
        assert_eq!((thumb.width, thumb.height), (272, 216));
        let png = encode_png(&RgbaImage { width: 2, height: 1, rgba: vec![255; 8] }).unwrap();
        let thumb = thumbnail(&png).unwrap();
        assert_eq!((thumb.width, thumb.height), (2, 1));
    }

    #[test]
    fn ignored_executables_match_leaf_names_without_matching_other_programs() {
        let ignored = vec!["KeePassXC.exe".into()];
        assert!(is_ignored(r"C:\Apps\keepassxc.EXE", &ignored));
        assert!(!is_ignored(r"C:\Apps\keepassxc-helper.exe", &ignored));
        assert_eq!(paste_keys("WindowsTerminal.exe").len(), 6);
        assert_eq!(paste_keys("notepad.exe").len(), 4);
    }

    #[test]
    fn only_a_bare_link_yields_to_the_picture() {
        assert!(is_image_address("  https://example.com/a.png \n"));
        assert!(is_image_address("DATA:image/png;base64,AAAA"));
        assert!(!is_image_address("Total\t42\nMédia\t7"));
        assert!(!is_image_address("veja https://example.com"));
    }

    #[test]
    fn only_explicit_history_opt_in_allows_a_present_flag() {
        assert!(!history_opt_in(&0_u32.to_le_bytes()));
        assert!(history_opt_in(&[1, 0, 0, 0, 0, 0, 0, 0]));
        assert!(!history_opt_in(&[1, 0, 0]));
    }

    #[test]
    fn unicode_text_uses_first_terminator_and_preserves_content() {
        let bytes = "  Açúcar 😺\r\n\0ignored"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(decode_text(&bytes).as_deref(), Some("  Açúcar 😺\r\n"));
        assert_eq!(decode_text(&[65, 0]), None);
    }

    #[test]
    fn files_round_trip_through_escaped_windows_uris() {
        let disk = Path::new(r"C:\Copiados\Açúcar #100%.png");
        let share = Path::new(r"\\storage\share\Q&A.txt");
        let disk_uri = "file:///C:/Copiados/A%C3%A7%C3%BAcar%20%23100%25.png";
        let share_uri = "file://storage/share/Q&A.txt";

        assert_eq!(file_uri(disk).as_deref(), Some(disk_uri));
        assert_eq!(file_uri(share).as_deref(), Some(share_uri));
        assert_eq!(file_path(disk_uri).as_deref(), Some(disk));
        assert_eq!(file_path(share_uri).as_deref(), Some(share));
        assert!(file_path("https://example.org/file").is_none());
    }

    #[test]
    fn hdrop_contains_double_terminated_absolute_paths() {
        let uri = "file:///C:/A%C3%A7%C3%BAcar.txt";
        let bytes = encode_hdrop(&[uri.to_owned()]).unwrap();
        assert_eq!(&bytes[0..4], &20_u32.to_le_bytes());
        assert_eq!(&bytes[16..20], &1_u32.to_le_bytes());
        assert_eq!(&bytes[bytes.len() - 4..], &[0, 0, 0, 0]);
        assert_eq!(wide_file_list(&bytes[20..]).unwrap(), [uri]);
        assert!(wide_file_list(&bytes[20..bytes.len() - 2]).is_none());
    }

    #[test]
    fn bitmap_rows_account_for_padding_and_bottom_up_orientation() {
        let mut dib = vec![0; 40];
        dib[0..4].copy_from_slice(&40_u32.to_le_bytes());
        dib[4..8].copy_from_slice(&1_i32.to_le_bytes());
        dib[8..12].copy_from_slice(&2_i32.to_le_bytes());
        dib[12..14].copy_from_slice(&1_u16.to_le_bytes());
        dib[14..16].copy_from_slice(&24_u16.to_le_bytes());
        dib.extend_from_slice(&[255, 0, 0, 77, 0, 0, 255, 99]);
        let image = decode_dib(&dib).unwrap();
        assert_eq!((image.width, image.height), (1, 2));
        assert_eq!(image.rgba, [255, 0, 0, 255, 0, 0, 255, 255]);
        dib.pop();
        assert!(decode_dib(&dib).is_none());
    }

    #[test]
    fn dibv5_writes_top_down_bgra_and_preserves_straight_alpha() {
        let image = RgbaImage {
            width: 1,
            height: 1,
            rgba: vec![23, 45, 67, 5],
        };
        let dib = encode_dibv5(&image);
        assert_eq!(&dib[8..12], &(-1_i32).to_le_bytes());
        assert_eq!(&dib[124..], [67, 45, 23, 5]);
        assert_eq!(decode_dib(&dib).unwrap().rgba, [23, 45, 67, 5]);
    }

    #[test]
    fn png_restore_validates_pixels_and_ignores_allocator_padding() {
        let image = RgbaImage {
            width: 1,
            height: 1,
            rgba: vec![23, 45, 67, 5],
        };
        let mut png = encode_png(&image).unwrap();
        let png_size = png.len();
        png.extend_from_slice(&[0; 7]);
        let decoded = decode_png(&png).unwrap();
        assert_eq!(decoded.rgba, [23, 45, 67, 5]);
        assert_eq!(png_end(&png), Some(png_size));
        assert!(decode_png(&png[..png_size - 5]).is_none());
    }
}
// Port of isle clipboard.js: executable-based ignore rules and terminal paste chords.
pub fn is_ignored(executable: &str, ignored: &[String]) -> bool {
    let leaf = executable.rsplit(['/', '\\']).next().unwrap_or_default();
    !leaf.is_empty() && ignored.iter().any(|app| app.trim().eq_ignore_ascii_case(leaf))
}

pub fn window_executable(hwnd: Option<HWND>) -> String {
    use windows::core::PWSTR;
    use windows::Win32::{Foundation::CloseHandle, System::Threading::*, UI::WindowsAndMessaging::GetWindowThreadProcessId};
    let Some(hwnd) = hwnd else { return String::new(); };
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return String::new(); };
        let mut buf = vec![0u16; 32768];
        let mut len = buf.len() as u32;
        let result = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(process);
        if result.is_ok() { String::from_utf16_lossy(&buf[..len as usize]) } else { String::new() }
    }
}

// Port of Clipboard::pressPaste (isle clipboard.js), Windows virtual-key equivalents.
fn paste_keys(exe: &str) -> Vec<(u16, bool)> {
    let leaf = exe.rsplit(['/', '\\']).next().unwrap_or_default().to_ascii_lowercase();
    let terminal = ["windowsterminal.exe", "wt.exe", "conhost.exe", "cmd.exe", "powershell.exe", "pwsh.exe", "wezterm-gui.exe", "alacritty.exe", "mintty.exe"].contains(&leaf.as_str());
    let mut keys = vec![(0x11, false)];
    if terminal { keys.push((0x10, false)); }
    keys.push((b'V' as u16, false));
    let up = keys.iter().rev().map(|(key, _)| (*key, true)).collect::<Vec<_>>();
    keys.extend(up);
    keys
}

pub fn paste_target(own: HWND) -> Option<isize> {
    let foreground = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
    (!foreground.is_invalid() && foreground != own).then_some(foreground.0 as isize)
}

// Port of Clipboard::paste (isle clipboard.js): only paste if the same app still has focus.
pub fn paste_into(target: isize) -> Result<()> {
    use windows::Win32::UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::{GetForegroundWindow, IsWindow}};
    let hwnd = HWND(target as *mut _);
    std::thread::sleep(std::time::Duration::from_millis(100));
    unsafe {
        ensure!(IsWindow(hwnd).as_bool() && GetForegroundWindow() == hwnd, "a janela de destino mudou");
        ensure!([VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN].into_iter().all(|key| GetAsyncKeyState(key.0 as i32) >= 0), "há teclas modificadoras pressionadas");
        let keys = paste_keys(&window_executable(Some(hwnd))).into_iter().map(|(key, up)| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(key), dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) }, ..Default::default() } },
        }).collect::<Vec<_>>();
        ensure!(SendInput(&keys, std::mem::size_of::<INPUT>() as i32) == keys.len() as u32, "não foi possível colar na janela");
    }
    Ok(())
}

/// OLE on the UI thread for as long as this lives: SHDoDragDrop (the shelf's drag out) needs it.
pub struct Ole(bool);
#[link(name = "ole32")]
unsafe extern "system" { fn OleInitialize(reserved: *mut std::ffi::c_void) -> windows::core::HRESULT; fn OleUninitialize(); }
impl Ole {
    pub fn initialize() -> Self {
        let status = unsafe { OleInitialize(std::ptr::null_mut()) };
        if status.is_err() { tracing::warn!("OLE indisponível, a prateleira não arrasta arquivos: {status:?}"); }
        Self(status.is_ok())
    }
}
impl Drop for Ole { fn drop(&mut self) { if self.0 { unsafe { OleUninitialize() } } } }

// Port of ShelfPanel::DragOut (mac-island 3.1.1). Call on the OLE UI thread while
// the initiating mouse button is down. Shell owns the transfer; never move files here.
pub fn drag_out(hwnd: HWND, path: &Path, allow_move: bool) -> Result<bool> {
    use windows::core::{Interface, GUID};
    use windows::Win32::{System::Com::IDataObject, UI::Shell::{SHCreateItemFromParsingName, IShellItem, BHID_DataObject}};
    // The existing windows feature set omits Ole. These ABI declarations avoid
    // changing the other track's Cargo.toml just to access two stable OLE calls.
    #[link(name = "shell32")]
    unsafe extern "system" { fn SHDoDragDrop(hwnd: HWND, data: *mut std::ffi::c_void, source: *mut std::ffi::c_void, allowed: u32, effect: *mut u32) -> windows::core::HRESULT; }
    let _iid: GUID = IDataObject::IID;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)?;
        let data: IDataObject = item.BindToHandler(None, &BHID_DataObject)?;
        let mut effect = 0;
        SHDoDragDrop(hwnd, data.as_raw(), std::ptr::null_mut(), if allow_move { 3 } else { 1 }, &mut effect).ok()?;
        Ok(effect == 2 || !path.is_file())
    }
}

// Port of ThumbPlan (isle clipboard.rs), at twice the card size (272 x 216).
pub fn thumbnail(bytes: &[u8]) -> Option<isle_core::artwork::Artwork> {
    let image = decode_png(bytes)?;
    let (w, h) = (image.width, image.height);
    let (cw, ch, tw, th) = if w >= 272 && h >= 216 {
        let (cw, ch) = if u64::from(w) * 216 > u64::from(h) * 272 { (((u64::from(h) * 272 + 108) / 216) as u32, h) }
            else { (w, ((u64::from(w) * 216 + 136) / 272) as u32) };
        (cw, ch, 272, 216)
    } else { (w.min(272), h.min(216), w.min(272), h.min(216)) };
    let mut pixels = image.rgba;
    for p in pixels.chunks_exact_mut(4) { for channel in 0..3 { p[channel] = (p[channel] as u16 * p[3] as u16 / 255) as u8; } }
    let source = tiny_skia::Pixmap::from_vec(pixels, tiny_skia::IntSize::from_wh(w, h)?)?;
    let mut target = tiny_skia::Pixmap::new(tw, th)?;
    let sx = tw as f32 / cw as f32;
    let sy = th as f32 / ch as f32;
    target.draw_pixmap(0, 0, source.as_ref(), &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() },
        tiny_skia::Transform::from_translate(-(((w - cw) / 2) as f32), -(((h - ch) / 2) as f32)).post_scale(sx, sy), None);
    let mut rgba = target.data().to_vec();
    for p in rgba.chunks_exact_mut(4) { if p[3] > 0 { for channel in 0..3 { p[channel] = (p[channel] as u32 * 255 / p[3] as u32).min(255) as u8; } } }
    isle_core::artwork::Artwork::new(tw, th, rgba)
}

#[derive(Default)]
pub struct Stored {
    pub history: isle_core::clipboard::ClipboardHistory,
    pub shelf: isle_core::shelf::Shelf,
    /// Shared with the island's own map: a save clones no image.
    pub images: std::collections::HashMap<String, std::sync::Arc<Vec<u8>>>,
    pub thumbnails: std::collections::HashMap<String, std::sync::Arc<isle_core::artwork::Artwork>>,
    pub shelf_thumbnails: std::collections::HashMap<String, std::sync::Arc<isle_core::artwork::Artwork>>,
    pub keep_clipboard: bool,
    pub keep_shelf: bool,
}

pub struct DiskStore { dir: PathBuf, clipboard_readonly: bool, shelf_readonly: bool, lost: bool }
impl DiskStore {
    pub fn at(dir: PathBuf) -> Self { Self { dir, clipboard_readonly: false, shelf_readonly: false, lost: false } }
    // Port of Store::load (isle store.rs): preserve corrupt/unreadable indices.
    fn index<T: serde::de::DeserializeOwned + Default>(&mut self, name: &str, clipboard: bool) -> T {
        let path = self.dir.join(format!("{name}.json"));
        let result = std::fs::read(&path);
        let bytes = match result {
            Ok(bytes) if bytes.len() <= 16 * 1024 * 1024 => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return T::default(),
            _ => { self.lost = true; if clipboard { self.clipboard_readonly = true; } else { self.shelf_readonly = true; } return T::default(); }
        };
        serde_json::from_slice(&bytes).unwrap_or_else(|_| {
            self.lost = true;
            if std::fs::rename(&path, path.with_extension("json.corrupt")).is_err() {
                if clipboard { self.clipboard_readonly = true; } else { self.shelf_readonly = true; }
            }
            T::default()
        })
    }
    // Port of ClipboardService::load and ShelfService::prune (isle): missing data is dropped.
    pub fn load(&mut self, clipboard: bool, shelf: bool) -> Stored {
        let mut value = Stored::default();
        if clipboard {
            value.history = self.index("clipboard", true);
            value.history.prune();
            let mut loaded_bytes = 0usize;
            for hash in value.history.image_hashes() {
                if !valid_hash(hash) { continue; }
                if let Ok(bytes) = std::fs::read(self.dir.join("clips").join(format!("{hash}.png"))) {
                    if bytes.len() <= MAX_IMAGE_BYTES && loaded_bytes + bytes.len() <= 64 * 1024 * 1024 && isle_core::clipboard::content_hash(&bytes) == hash {
                        if let Some(thumb) = thumbnail(&bytes) {
                            loaded_bytes += bytes.len();
                            value.images.insert(hash.to_owned(), std::sync::Arc::new(bytes));
                            value.thumbnails.insert(hash.to_owned(), std::sync::Arc::new(thumb));
                        }
                    }
                }
            }
            value.history.retain(|item| match &item.clip {
                isle_core::clipboard::Clip::Image { hash, .. } => value.images.contains_key(hash),
                isle_core::clipboard::Clip::Files { uris } => uris.iter().all(|uri| file_path(uri).is_some_and(|p| path_exists(&p))),
                _ => true,
            });
            value.history.prune();
        }
        if shelf {
            value.shelf = self.index("shelf", false);
            value.shelf.retain_existing(|item| file_path(&item.uri).is_some_and(|p| path_exists(&p)));
            for item in value.shelf.items().iter().take(6) {
                if let Some(thumb) = file_path(&item.uri).and_then(|p| file_thumbnail(&p)) {
                    value.shelf_thumbnails.insert(item.uri.clone(), std::sync::Arc::new(thumb));
                }
            }
        }
        value
    }
    // Port of Store::save + ClipFiles::remove_orphans (isle): images first, then atomic JSON.
    pub fn save(&mut self, history: &isle_core::clipboard::ClipboardHistory, shelf: &isle_core::shelf::Shelf,
        images: &std::collections::HashMap<String, std::sync::Arc<Vec<u8>>>, clipboard: bool, keep_shelf: bool) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        if clipboard && !self.clipboard_readonly {
            let dir = self.dir.join("clips");
            std::fs::create_dir_all(&dir)?;
            for hash in history.image_hashes().filter(|hash| valid_hash(hash)) {
                if let Some(bytes) = images.get(hash) {
                    let path = dir.join(format!("{hash}.png"));
                    // Thumbnails are made again from the image at load: none are written.
                    if !path.exists() { write_binary(&path, bytes)?; }
                }
            }
            isle_core::config::write_atomic(&self.dir.join("clipboard.json"), &serde_json::to_string(history)?)?;
            if !self.lost {
                let keep: std::collections::HashSet<_> = history.image_hashes().collect();
                for entry in std::fs::read_dir(&dir)?.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let hash = name.strip_suffix(".thumb.png").or_else(|| name.strip_suffix(".png"));
                    if hash.is_some_and(|hash| valid_hash(hash) && !keep.contains(hash)) { let _ = std::fs::remove_file(entry.path()); }
                }
            }
        }
        if keep_shelf && !self.shelf_readonly {
            // The shelf's previews are read from its files at load: none are written.
            isle_core::config::write_atomic(&self.dir.join("shelf.json"), &serde_json::to_string(shelf)?)?;
        }
        Ok(())
    }
}

// Port of the shelf's thumbnails (isle thumbnails.js): bounded image reads on a worker.
pub fn file_thumbnail(path: &Path) -> Option<isle_core::artwork::Artwork> {
    if path.to_string_lossy().starts_with(r"\\") { return None; }
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    if !matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "gif" | "webp") { return None; }
    let mut file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > isle_core::artwork::MAX_ENCODED { return None; }
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut std::io::Read::take(&mut file, isle_core::artwork::MAX_ENCODED + 1), &mut bytes).ok()?;
    if bytes.len() as u64 > isle_core::artwork::MAX_ENCODED { return None; }
    if extension == "png" { return thumbnail(&bytes); }
    unsafe {
        use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
        let initialized = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let decoded = crate::artwork::decode(&bytes).ok();
        if initialized { CoUninitialize(); }
        let image = decoded?;
        thumbnail(&encode_png(&RgbaImage { width: image.width, height: image.height, rgba: image.rgba })?)
    }
}
fn valid_hash(hash: &str) -> bool { hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit()) }
fn path_exists(path: &Path) -> bool {
    // As isle's bounded check: don't discard an unavailable network share.
    if path.to_string_lossy().starts_with(r"\\") { true } else { path.is_file() }
}
fn write_binary(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temp = path.with_extension("png.tmp");
    { use std::io::Write; let mut file = std::fs::File::create(&temp)?; file.write_all(bytes)?; file.sync_all()?; }
    std::fs::rename(temp, path)
}

pub struct StoreWorker {
    shared: std::sync::Arc<(std::sync::Mutex<(Option<Stored>, bool)>, std::sync::Condvar)>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl StoreWorker {
    // Port of Store::save coalescing (isle store.rs): one writer, newest snapshot last.
    pub fn start(dir: PathBuf, clipboard: bool, shelf: bool, loaded: impl FnOnce(Stored) + Send + 'static) -> Self {
        let shared: std::sync::Arc<(std::sync::Mutex<(Option<Stored>, bool)>, std::sync::Condvar)> = std::sync::Arc::new((std::sync::Mutex::new((None, false)), std::sync::Condvar::new()));
        let thread_shared = shared.clone();
        let thread = std::thread::spawn(move || {
            let mut store = DiskStore::at(dir);
            loaded(store.load(clipboard, shelf));
            loop {
                let (lock, ready) = &*thread_shared;
                let mut state = lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                while state.0.is_none() && !state.1 { state = ready.wait(state).unwrap_or_else(std::sync::PoisonError::into_inner); }
                let next = state.0.take();
                let stop = state.1;
                drop(state);
                if let Some(next) = next {
                    if let Err(err) = store.save(&next.history, &next.shelf, &next.images, next.keep_clipboard, next.keep_shelf) { tracing::warn!(%err, "Não foi possível salvar a prateleira/histórico"); }
                }
                if stop { break; }
            }
        });
        Self { shared, thread: Some(thread) }
    }
    pub fn save(&self, state: Stored) {
        self.shared.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).0 = Some(state);
        self.shared.1.notify_one();
    }
}
impl Drop for StoreWorker {
    fn drop(&mut self) {
        self.shared.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).1 = true;
        self.shared.1.notify_one();
        if let Some(thread) = self.thread.take() { let _ = thread.join(); }
    }
}
