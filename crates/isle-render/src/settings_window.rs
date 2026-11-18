//! The settings window: a normal Windows window (moves, resizes, taskbar, Alt+Tab), dark title bar,
//! drawn by `PillRenderer::render_settings` and painted from its last frame. One at a time; closing
//! only hides it. Inputs come out in logical pixels.
use crate::renderer::PillFrameBuffer;
use isle_core::config::WindowPlacement;

/// Logical size the first time it opens.
pub const DEFAULT_SIZE: (i32, i32) = (1180, 760);
/// Smallest logical size.
pub const MIN_SIZE: (i32, i32) = (600, 480);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingsInput {
    Down { x: f32, y: f32 },
    Up { x: f32, y: f32 },
    Move { x: f32, y: f32 },
    Leave,
    Wheel { delta: i32, x: f32, y: f32 },
    Char(char),
    /// Tab, arrows, Home, End, Delete, Page Up/Down (virtual key), and whether Shift was down.
    Key(u16, bool),
    /// Any key but a modifier, with the modifiers held (`isle_core::shortcut::code`): what a
    /// shortcut row records.
    Chord(u32),
    Resized,
    /// The mouse was taken away mid-drag (Alt+Tab, a dialog): no button-up will come.
    CaptureLost,
    Closed,
}

/// Where the window opens (physical pixels): where it was, when the middle of it still falls on a
/// monitor; else centered on the main one (`work[0]`, work areas as left, top, right, bottom) at
/// `scale`, never bigger than that work area.
pub fn fit_placement(saved: Option<&WindowPlacement>, work: &[(i32, i32, i32, i32)], scale: f32) -> (i32, i32, i32, i32) {
    if let Some(p) = saved {
        let (cx, cy) = (p.x + p.w / 2, p.y + p.h / 2);
        if p.w > 0 && p.h > 0 && work.iter().any(|(l, t, r, b)| cx >= *l && cx < *r && cy >= *t && cy < *b) {
            return (p.x, p.y, p.w, p.h);
        }
    }
    let (l, t, r, b) = work.first().copied().unwrap_or((0, 0, 1920, 1040));
    let w = ((DEFAULT_SIZE.0 as f32 * scale).round() as i32).min(r - l);
    let h = ((DEFAULT_SIZE.1 as f32 * scale).round() as i32).min(b - t);
    (l + (r - l - w) / 2, t + (b - t - h) / 2, w, h)
}

#[cfg(windows)]
pub use win::SettingsWindow;

#[cfg(not(windows))]
#[derive(Default)]
pub struct SettingsWindow;

#[cfg(not(windows))]
impl SettingsWindow {
    pub fn open(&mut self, _: Option<&WindowPlacement>) -> anyhow::Result<()> {
        Ok(())
    }
    pub fn close(&mut self) {}
    pub fn is_open(&self) -> bool {
        false
    }
    pub fn is_visible(&self) -> bool {
        false
    }
    pub fn size(&self) -> (f32, f32, f32) {
        (DEFAULT_SIZE.0 as f32, DEFAULT_SIZE.1 as f32, 1.0)
    }
    pub fn present(&mut self, _: &PillFrameBuffer) {}
    pub fn take_inputs(&mut self) -> Vec<SettingsInput> {
        Vec::new()
    }
    pub fn placement(&self) -> Option<WindowPlacement> {
        None
    }
    pub fn set_cursor(&mut self, _: bool) {}
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::cell::{Cell, RefCell};
    use windows::Win32::Foundation::{POINT, BOOL, COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Dwm::{DWMWA_CAPTION_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyState, ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_SHIFT,
    };
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::core::{PCWSTR, w};

    const WM_MOUSELEAVE: u32 = 0x02A3;

    thread_local! {
        static INPUTS: RefCell<Vec<SettingsInput>> = const { RefCell::new(Vec::new()) };
        static SCALE: Cell<f32> = const { Cell::new(1.0) };
        static FRAME: RefCell<Option<(u32, u32, Vec<u8>)>> = const { RefCell::new(None) };
        static HAND: Cell<bool> = const { Cell::new(false) };
        static TRACKING: Cell<bool> = const { Cell::new(false) };
        static SURROGATE: Cell<Option<u16>> = const { Cell::new(None) };
        static RELEASING: Cell<bool> = const { Cell::new(false) };
    }

    fn push(input: SettingsInput) {
        INPUTS.with(|i| i.borrow_mut().push(input));
    }

    fn logical(lparam: LPARAM) -> (f32, f32) {
        let s = SCALE.with(|s| s.get());
        let x = (lparam.0 & 0xFFFF) as i16 as f32;
        let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
        (x / s, y / s)
    }

    fn paint(hdc: HDC) {
        FRAME.with(|f| {
            if let Some((w, h, bgra)) = f.borrow().as_ref() {
                let bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: *w as i32,
                        biHeight: -(*h as i32),
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                unsafe {
                    SetDIBitsToDevice(hdc, 0, 0, *w, *h, 0, 0, 0, *h, bgra.as_ptr().cast(), &bmi, DIB_RGB_COLORS);
                }
            }
        });
    }

    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WM_LBUTTONDOWN => {
                unsafe { SetCapture(hwnd) };
                let (x, y) = logical(lparam);
                push(SettingsInput::Down { x, y });
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let (x, y) = logical(lparam);
                push(SettingsInput::Up { x, y });
                RELEASING.with(|r| r.set(true));
                let _ = unsafe { ReleaseCapture() };
                RELEASING.with(|r| r.set(false));
                LRESULT(0)
            }
            WM_CAPTURECHANGED => {
                if !RELEASING.with(|r| r.get()) {
                    push(SettingsInput::CaptureLost);
                }
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                if !TRACKING.with(|t| t.replace(true)) {
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = unsafe { TrackMouseEvent(&mut track) };
                }
                let (x, y) = logical(lparam);
                push(SettingsInput::Move { x, y });
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                TRACKING.with(|t| t.set(false));
                push(SettingsInput::Leave);
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let mut pt = POINT {
                    x: (lparam.0 & 0xFFFF) as i16 as i32,
                    y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
                };
                let _ = unsafe { ScreenToClient(hwnd, &mut pt) };
                let scale = SCALE.with(|value| value.get());
                push(SettingsInput::Wheel {
                    delta: (wparam.0 >> 16) as u16 as i16 as i32,
                    x: pt.x as f32 / scale,
                    y: pt.y as f32 / scale,
                });
                LRESULT(0)
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let vk = wparam.0 as u16;
                if let Some(code) = chord(vk) {
                    push(SettingsInput::Chord(code));
                }
                if msg == WM_SYSKEYDOWN {
                    return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
                }
                const KEYS: [u16; 10] = [0x09, 0x25, 0x26, 0x27, 0x28, 0x24, 0x23, 0x2E, 0x21, 0x22];
                if KEYS.contains(&vk) {
                    let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
                    push(SettingsInput::Key(vk, shift));
                    return LRESULT(0);
                }
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_CHAR => {
                let unit = wparam.0 as u16;
                if unit == 0x09 {
                    return LRESULT(0);
                }
                if (0xD800..0xDC00).contains(&unit) {
                    SURROGATE.with(|s| s.set(Some(unit)));
                } else if let Some(high) = SURROGATE.with(|s| s.take()).filter(|_| (0xDC00..0xE000).contains(&unit)) {
                    if let Some(Ok(c)) = char::decode_utf16([high, unit]).next() {
                        push(SettingsInput::Char(c));
                    }
                } else if let Some(c) = char::from_u32(unit as u32) {
                    push(SettingsInput::Char(c));
                }
                LRESULT(0)
            }
            WM_SIZE => {
                push(SettingsInput::Resized);
                LRESULT(0)
            }
            WM_DPICHANGED => {
                SCALE.with(|s| s.set(((wparam.0 & 0xFFFF) as f32 / 96.0).max(0.5)));
                let r = unsafe { &*(lparam.0 as *const RECT) };
                let _ = unsafe {
                    SetWindowPos(hwnd, None, r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE)
                };
                push(SettingsInput::Resized);
                LRESULT(0)
            }
            WM_GETMINMAXINFO => {
                let s = SCALE.with(|s| s.get());
                let info = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
                info.ptMinTrackSize.x = (MIN_SIZE.0 as f32 * s) as i32;
                info.ptMinTrackSize.y = (MIN_SIZE.1 as f32 * s) as i32;
                LRESULT(0)
            }
            WM_SETCURSOR if (lparam.0 & 0xFFFF) as u32 == HTCLIENT => {
                let cursor = if HAND.with(|h| h.get()) { IDC_HAND } else { IDC_ARROW };
                if let Ok(c) = unsafe { LoadCursorW(None, cursor) } {
                    unsafe { SetCursor(c) };
                }
                LRESULT(1)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
                paint(hdc);
                let _ = unsafe { EndPaint(hwnd, &ps) };
                LRESULT(0)
            }
            WM_CLOSE => {
                push(SettingsInput::Closed);
                let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    #[derive(Default)]
    pub struct SettingsWindow {
        hwnd: isize,
        shown: bool,
    }

    impl SettingsWindow {
        fn hwnd(&self) -> HWND {
            HWND(self.hwnd as *mut _)
        }

        /// Shows it (made the first time, at `saved` when that is still on a monitor), or only
        /// brings it to the front when it is already open.
        pub fn open(&mut self, saved: Option<&WindowPlacement>) -> anyhow::Result<()> {
            if self.hwnd == 0 {
                self.create(saved)?;
            }
            unsafe {
                let _ = ShowWindow(self.hwnd(), if IsIconic(self.hwnd()).as_bool() { SW_RESTORE } else { SW_SHOW });
                let _ = SetForegroundWindow(self.hwnd());
            }
            self.shown = true;
            Ok(())
        }

        fn create(&mut self, saved: Option<&WindowPlacement>) -> anyhow::Result<()> {
            unsafe {
                let instance = GetModuleHandleW(None)?;
                let class = w!("CimoriSettingsWindow");
                let icon = LoadIconW(instance, PCWSTR(std::ptr::without_provenance(1))).unwrap_or_default();
                let wc = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    lpfnWndProc: Some(wnd_proc),
                    hInstance: instance.into(),
                    hCursor: LoadCursorW(None, IDC_ARROW)?,
                    hIcon: icon,
                    hIconSm: icon,
                    lpszClassName: class,
                    ..Default::default()
                };
                let _ = RegisterClassExW(&wc);
                let (work, scale) = work_areas();
                let (x, y, w, h) = fit_placement(saved, &work, scale);
                let hwnd = CreateWindowExW(
                    WS_EX_APPWINDOW,
                    class,
                    w!("Cimori — Configurações"),
                    WS_OVERLAPPEDWINDOW,
                    x,
                    y,
                    w,
                    h,
                    None,
                    None,
                    instance,
                    None,
                )?;
                self.hwnd = hwnd.0 as isize;
                SCALE.with(|s| s.set(GetDpiForWindow(hwnd).max(48) as f32 / 96.0));
                let dark = BOOL(1);
                let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, (&dark as *const BOOL).cast(), 4);
                // The side bar's color (23, 25, 30), as COLORREF 0x00BBGGRR.
                let caption = COLORREF(0x001E_1917);
                let _ = DwmSetWindowAttribute(hwnd, DWMWA_CAPTION_COLOR, (&caption as *const COLORREF).cast(), 4);
            }
            Ok(())
        }

        pub fn close(&mut self) {
            if self.hwnd != 0 {
                unsafe {
                    let _ = ShowWindow(self.hwnd(), SW_HIDE);
                }
            }
            self.shown = false;
            FRAME.with(|f| f.borrow_mut().take());
        }

        pub fn is_open(&self) -> bool {
            self.shown
        }

        /// Open and on screen: neither hidden nor minimized.
        pub fn is_visible(&self) -> bool {
            self.shown && unsafe { IsWindowVisible(self.hwnd()).as_bool() && !IsIconic(self.hwnd()).as_bool() }
        }

        /// Logical width and height of the client area, and the scale.
        pub fn size(&self) -> (f32, f32, f32) {
            let s = SCALE.with(|s| s.get());
            let mut r = RECT::default();
            unsafe {
                let _ = GetClientRect(self.hwnd(), &mut r);
            }
            (((r.right - r.left) as f32 / s).max(1.0), ((r.bottom - r.top) as f32 / s).max(1.0), s)
        }

        pub fn present(&mut self, fb: &PillFrameBuffer) {
            FRAME.with(|f| *f.borrow_mut() = Some((fb.width, fb.height, fb.bgra_premultiplied.clone())));
            unsafe {
                let hdc = GetDC(self.hwnd());
                paint(hdc);
                ReleaseDC(self.hwnd(), hdc);
            }
        }

        pub fn take_inputs(&mut self) -> Vec<SettingsInput> {
            let inputs = INPUTS.with(|i| std::mem::take(&mut *i.borrow_mut()));
            if inputs.contains(&SettingsInput::Closed) {
                self.shown = false;
            }
            inputs
        }

        /// Where it is now (physical pixels); the app adds the tab.
        pub fn placement(&self) -> Option<WindowPlacement> {
            if self.hwnd == 0 {
                return None;
            }
            // The restored rectangle, also when it was closed maximized or minimized.
            let mut wp = WINDOWPLACEMENT { length: std::mem::size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
            unsafe { GetWindowPlacement(self.hwnd(), &mut wp).ok()? };
            let r = wp.rcNormalPosition;
            Some(WindowPlacement { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top, tab: String::new() })
        }

        pub fn set_cursor(&mut self, hand: bool) {
            HAND.with(|h| h.set(hand));
        }
    }

    /// Work areas, the primary monitor's first, and its scale.
    fn work_areas() -> (Vec<(i32, i32, i32, i32)>, f32) {
        let mut out = Vec::new();
        let mut primary_scale = 1.0;
        for m in crate::window::list_monitors() {
            let (l, t) = (m.left, m.top);
            let monitor = unsafe {
                MonitorFromPoint(windows::Win32::Foundation::POINT { x: l + 1, y: t + 1 }, MONITOR_DEFAULTTONEAREST)
            };
            let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
                let r = info.rcWork;
                let area = (r.left, r.top, r.right, r.bottom);
                if m.primary {
                    out.insert(0, area);
                    primary_scale = m.scale;
                } else {
                    out.push(area);
                }
            }
        }
        (out, primary_scale)
    }
}

/// A key pressed with the modifiers held now, as a shortcut row records it; none for a modifier alone.
#[cfg(windows)]
fn chord(vk: u16) -> Option<u32> {
    use isle_core::shortcut::{ALT, CTRL, SHIFT, WIN, code};
    use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState;
    // Shift, Ctrl, Alt (either side) and the Windows keys.
    if matches!(vk, 0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0..=0xA5) || vk > 0xFF {
        return None;
    }
    let down = |key: i32| unsafe { GetKeyState(key) } < 0;
    let mut mods = 0;
    if down(0x5B) || down(0x5C) { mods |= WIN; }
    if down(0x11) { mods |= CTRL; }
    if down(0x12) { mods |= ALT; }
    if down(0x10) { mods |= SHIFT; }
    Some(code(mods, vk as u8))
}
