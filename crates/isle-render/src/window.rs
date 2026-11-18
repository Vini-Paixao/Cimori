// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Adapted for isle-win from Mac Island & Dock by caliberda v3.1.0 (Island::HandleMessage):
// wheel and captured slider input with per-monitor DPI; existing isle-win behavior retained.
// Right-button tray input adapted from Mac Island & Dock v3.1.1.

//! The pill's windows: one layered, topmost, no-activate window per chosen monitor, drawn at that monitor's
//! DPI (Per-Monitor V2), placed top-centre on the monitor matched by its device name ("\\.\DISPLAY1"), as
//! the Mac Island & Dock mod (caliberda, GPL-3.0) places its islands. The UI thread sleeps in
//! `MsgWaitForMultipleObjects` until a window message, a wake from the services, or a deadline.

pub use crate::pages::UiAction;
use crate::pages::{compute_hitboxes, slider_percent};
use crate::renderer::{LINE_REACH_X, LINE_REACH_Y, PillRenderer};
use anyhow::Result;
use isle_core::config::IsleConfig;
use isle_core::spring::PillGeometry;
use isle_core::state::{IslandState, PillMode};
use std::sync::Mutex;
use std::time::Duration;

/// The canvas each window draws the pill in, in logical (100%) pixels.
pub const CANVAS_MAX_WIDTH: i32 = 720;
pub const CANVAS_MAX_HEIGHT: i32 = 520;

pub fn point_inside_pill(
    client_x: i32,
    client_y: i32,
    canvas_width: i32,
    pill_w: f32,
    pill_h: f32,
    top_gap: f32,
) -> bool {
    let half_canvas = (canvas_width as f32) * 0.5;
    let half_pill = pill_w * 0.5;
    let left = (half_canvas - half_pill).floor() as i32;
    let right = (half_canvas + half_pill).ceil() as i32;
    let top = top_gap.floor() as i32;
    let bottom = (top_gap + pill_h).ceil() as i32;

    client_x >= left && client_x <= right && client_y >= top && client_y <= bottom
}

pub fn hit_test_action(
    client_x: i32,
    client_y: i32,
    canvas_width: i32,
    pill_w: f32,
    pill_h: f32,
    top_gap: f32,
    state: &IslandState,
) -> Option<UiAction> {
    if !point_inside_pill(client_x, client_y, canvas_width, pill_w, pill_h, top_gap) {
        return None;
    }
    let px = client_x as f32;
    let py = client_y as f32;
    compute_hitboxes(canvas_width as f32, pill_w, pill_h, top_gap, state)
        .into_iter()
        .find(|hb| hb.contains(px, py))
        .map(|hb| match hb.action {
            UiAction::SetVolume(_) => UiAction::SetVolume(slider_percent(px, hb.x, hb.w)),
            UiAction::AppVolume(i, _) => UiAction::AppVolume(i, slider_percent(px, hb.x, hb.w)),
            action => action,
        })
}

/// Where the pill takes the mouse, in logical pixels of its canvas: centred, `width` wide, `top` to `bottom`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitArea {
    pub width: f32,
    pub top: f32,
    pub bottom: f32,
}

impl HitArea {
    /// The pill as it is now; the hidden line takes the mouse from the screen's top edge (see the renderer).
    pub fn of(geom: &PillGeometry, gap: f32, mode: &PillMode) -> Self {
        let (w, h) = (geom.width.value, geom.height.value);
        if *mode == PillMode::Minimized {
            Self { width: w + LINE_REACH_X * 2.0, top: 0.0, bottom: gap + h + LINE_REACH_Y,
            }
        } else {
            Self { width: w, top: gap, bottom: gap + h,
            }
        }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        let middle = CANVAS_MAX_WIDTH as f32 * 0.5;
        (x - middle).abs() <= self.width * 0.5 && y >= self.top && y <= self.bottom
    }

    /// Packed with the window's scale into its GWLP_USERDATA, for WM_NCHITTEST (0: nothing yet).
    #[cfg_attr(not(windows), allow(dead_code))]
    fn pack(&self, scale: f32) -> u64 {
        let field = |v: f32| (v.round().clamp(0.0, 65_535.0)) as u64;
        field(self.width) | field(self.top) << 16 | field(self.bottom) << 32 | field(scale * 100.0) << 48
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn unpack(packed: u64) -> Option<(Self, f32)> {
        let field = |shift: u32| ((packed >> shift) & 0xFFFF) as f32;
        let scale = field(48) / 100.0;
        (scale > 0.0).then(|| {
            (Self { width: field(0), top: field(16), bottom: field(32),
                },
                scale,
            )
        })
    }
}

/// A monitor the pill can be on, in physical pixels of the virtual screen.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorPlace {
    /// "\\.\DISPLAY1", as Windows and the Windhawk backup name it.
    pub device: String,
    pub left: i32,
    pub top: i32,
    pub width: i32,
    /// DPI / 96: 1.0 at 100%, 1.5 at 150%.
    pub scale: f32,
    pub primary: bool,
}

/// Mac Island & Dock's CoversMonitor (rects as left, top, right, bottom): a game in borderless or
/// exclusive full screen, or a video in full screen, covers its monitor and the island steps aside.
/// `busy`: Windows itself reports a full-screen app (then a captioned window stretched over it counts).
pub fn covers_monitor(window: (i32, i32, i32, i32), monitor: (i32, i32, i32, i32), zoomed: bool, caption: bool, busy: bool,
) -> bool {
    let ((l, t, r, b), (ml, mt, mr, mb)) = (window, monitor);
    if l > ml || t > mt || r < mr || b < mb {
        return false;
    }
    if window == monitor {
        return true;
    }
    // Bigger than the screen: a maximized window's borders spill past the edges; not full screen.
    !zoomed && (!caption || busy)
}

/// The main island's monitor first: the one picked (`main`, by name), else the primary. Its island gets
/// the notifications, alerts and permission cards, as Mac Island & Dock's main island.
fn main_first(mut places: Vec<MonitorPlace>, main: Option<&str>) -> Vec<MonitorPlace> {
    let picked = |m: &MonitorPlace| main.is_some_and(|name| name.trim().eq_ignore_ascii_case(&m.device));
    let has_picked = places.iter().any(picked);
    places.sort_by_key(|m| if has_picked { !picked(m) } else { !m.primary });
    places
}

/// The configured monitors that are connected, by name, whatever the case; none of them there (unplugged,
/// renumbered), the primary one, so the pill never goes missing.
pub fn choose_monitors(wanted: &[String], all: &[MonitorPlace], main: Option<&str>,
) -> Vec<MonitorPlace> {
    let chosen: Vec<MonitorPlace> = all
        .iter()
        .filter(|m| {
            wanted.iter().any(|w| w.trim().eq_ignore_ascii_case(&m.device))
        })
        .cloned()
        .collect();
    if !chosen.is_empty() {
        return main_first(chosen, main);
    }
    all.iter()
        .find(|m| m.primary)
        .or(all.first())
        .cloned()
        .into_iter()
        .collect()
}

/// Every connected monitor ("an island on every monitor").
pub fn choose_every_monitor(all: &[MonitorPlace], main: Option<&str>) -> Vec<MonitorPlace> {
    main_first(all.to_vec(), main)
}

/// What the windows tell the app, in logical pixels.
#[derive(Debug, Clone, PartialEq)]
pub enum UiInput {
    /// On island `window` (0 is the main one).
    Click { window: usize, x: f32, y: f32,
    },
    PointerDown {
        window: usize,
        x: f32,
        y: f32,
    },
    RightClick {
        window: usize,
        x: f32,
        y: f32,
    },
    Drag {
        window: usize,
        x: f32,
        y: f32,
    },
    CancelDrag,
    Wheel { window: usize, delta: i32, x: f32, y: f32 },
    /// The mouse reached (`true`) or left island `window`.
    Hover {
        window: usize,
        inside: bool,
    },
    /// Monitors were added, removed, moved or rescaled: the windows are made again.
    MonitorsChanged,
    /// A character typed while island `window` has the keyboard (Enter, Backspace, Esc and
    /// Ctrl+V come as their control characters: CR, BS, ESC and SYN).
    Char { window: usize, ch: char,
    },
    /// The island stopped being the window in front: it no longer gets what's typed.
    KeyboardLost,
    /// Another `Cimori --settings` asked this one to show the settings window.
    OpenSettings,
    ClipboardChanged,
    DevicesChanged,
    /// Files dragged over island `window`'s pill (`true`), or gone from it (left, or let go).
    FilesOver { window: usize, over: bool },
    FilesDropped { window: usize, paths: Vec<std::path::PathBuf> },
    SessionUnavailable,
    Hotkey(i32),
}

#[derive(Debug, Clone)]
#[cfg_attr(not(windows), allow(dead_code))]
enum Raw {
    Click(isize, i32, i32),
    RightClick(isize, i32, i32),
    Down(isize, i32, i32),
    Drag(isize, i32, i32),
    Move(isize, i32, i32),
    CancelDrag,
    Wheel(isize, i32, i32, i32),
    Enter(isize),
    Leave(isize),
    Changed,
    /// The window in front changed, or moved: maybe a full-screen app came or went.
    Foreground,
    Char(isize, u16),
    KeyboardLost,
    OpenSettings,
    ClipboardChanged,
    DevicesChanged,
    DragFiles(isize, bool),
    FilesDropped(isize, Vec<std::path::PathBuf>),
    SessionUnavailable,
    Hotkey(i32),
}

/// Filled by the window procedure (on the UI thread), drained by `take_inputs`.
static RAW: Mutex<Vec<Raw>> = Mutex::new(Vec::new());

#[cfg_attr(not(windows), allow(dead_code))]
fn push_raw(raw: Raw) {
    RAW.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(raw);
}

#[cfg_attr(not(windows), allow(dead_code))]
struct IslandWindow {
    #[cfg(windows)]
    glass: crate::glass_win::Backdrop,
    hwnd: isize,
    place: MonitorPlace,
    renderer: PillRenderer,
    inside: bool,
    /// Out of the way of a full-screen app on its monitor.
    hidden: bool,
    /// Whether its drop target takes files now (set as it's drawn).
    #[cfg(windows)]
    drops: std::rc::Rc<std::cell::Cell<bool>>,
    /// The bitmap UpdateLayeredWindow reads, kept between frames.
    #[cfg(windows)]
    surface: Option<win::Surface>,
}

pub struct MultiMonitorOverlay {
    pub monitors: Vec<String>,
    pub all_monitors: bool,
    pub main_monitor: Option<String>,
    /// Stay over borderless full-screen apps instead of stepping aside (settings).
    pub over_fullscreen: bool,
    pub follow_mouse: bool,
    windows: Vec<IslandWindow>,
    local: Vec<UiInput>,
    #[cfg(windows)]
    drag_foreground: Option<(isize, isize)>,
    /// While a reply is typed: the island's window and the one that was in front before.
    #[cfg(windows)]
    keyboard_foreground: Option<(isize, isize)>,
    /// A UTF-16 high surrogate waiting for its pair (an emoji comes as two WM_CHARs).
    #[cfg(windows)]
    surrogate: Option<u16>,
    #[cfg(windows)]
    hooks: Vec<isize>,
}

impl MultiMonitorOverlay {
    pub fn new(monitors: Vec<String>, all_monitors: bool, main_monitor: Option<String>) -> Self {
        Self {
            monitors,
            all_monitors,
            main_monitor,
            over_fullscreen: false,
            follow_mouse: false,
            windows: Vec::new(),
            local: Vec::new(),
            #[cfg(windows)]
            drag_foreground: None,
            #[cfg(windows)]
            keyboard_foreground: None,
            #[cfg(windows)]
            surrogate: None,
            #[cfg(windows)]
            hooks: Vec::new(),
        }
    }

    /// A click as if on a window at 100% (tests, and the Linux build that has no windows).
    pub fn enqueue_click(&mut self, x: f32, y: f32) {
        self.local.push(UiInput::Click { window: 0, x, y });
    }

    /// How many islands there are (one per chosen monitor; none without Windows).
    /// Available logical work area for the panel, with space for its shadow.
    pub fn available_size(&self,window:usize,gap:f32)->(f32,f32){
        #[cfg(windows)] {use windows::Win32::Graphics::Gdi::*;use windows::Win32::Foundation::HWND;
            if let Some(w)=self.windows.get(window){unsafe{let monitor=MonitorFromWindow(HWND(w.hwnd as *mut _),MONITOR_DEFAULTTONEAREST);let mut info=MONITORINFO{cbSize:std::mem::size_of::<MONITORINFO>()as u32,..Default::default()};if GetMonitorInfoW(monitor,&mut info).as_bool(){return(((info.rcWork.right-info.rcWork.left)as f32/w.place.scale-32.).min(CANVAS_MAX_WIDTH as f32-8.),((info.rcWork.bottom-w.place.top)as f32/w.place.scale-gap-16.).min(CANVAS_MAX_HEIGHT as f32-16.));}}}
        }
        let _=window;(CANVAS_MAX_WIDTH as f32-8.,CANVAS_MAX_HEIGHT as f32-16.)
    }
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// What happened on the windows since last time, physical pixels made logical by each window's scale.
    pub fn take_inputs(&mut self) -> Vec<UiInput> {
        let mut inputs = std::mem::take(&mut self.local);
        let raws = std::mem::take(&mut *RAW.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        if raws.iter().any(|r| matches!(r, Raw::Foreground)) {
            self.step_aside();
        }
        for raw in raws {
            match raw {
                Raw::Move(hwnd, x, y) => {
                    if let Some(w) = self.windows.iter_mut().find(|w| w.hwnd == hwnd) {
                        w.renderer.pointer =
                            Some((x as f32 / w.place.scale, y as f32 / w.place.scale));
                    }
                }
                Raw::Click(hwnd, x, y) | Raw::RightClick(hwnd, x, y) | Raw::Down(hwnd, x, y) | Raw::Drag(hwnd, x, y) => {
                    if let Some(window) = self.windows.iter().position(|w| w.hwnd == hwnd) {
                        self.windows[window].renderer.pressed =
                            matches!(raw, Raw::Down(..) | Raw::Drag(..));
                        let s = self.windows[window].place.scale;
                        let (x, y) = (x as f32 / s, y as f32 / s);
                        inputs.push(match raw {
                            Raw::RightClick(..) => UiInput::RightClick { window, x, y },
                            Raw::Down(..) => UiInput::PointerDown { window, x, y },
                            Raw::Drag(..) => UiInput::Drag { window, x, y },
                            _ => UiInput::Click { window, x, y },
                        });
                    }
                }
                Raw::Enter(hwnd) | Raw::Leave(hwnd) => {
                    let inside = matches!(raw, Raw::Enter(_));
                    if let Some(window) = self.windows.iter().position(|w| w.hwnd == hwnd)
                        && self.windows[window].inside != inside
                    {
                        self.windows[window].inside = inside;
                        if !inside {
                            self.windows[window].renderer.pointer = None;
                            self.windows[window].renderer.pressed = false;
                        }
                        inputs.push(UiInput::Hover { window, inside });
                    }
                }
                Raw::Wheel(hwnd, delta, x, y) => {
                    if let Some(window) = self.windows.iter().position(|w| w.hwnd == hwnd) {
                        let scale = self.windows[window].place.scale;
                        inputs.push(UiInput::Wheel { window, delta, x: x as f32 / scale, y: y as f32 / scale });
                    }
                }
                Raw::CancelDrag => inputs.push(UiInput::CancelDrag),
                Raw::Changed => inputs.push(UiInput::MonitorsChanged),
                Raw::Foreground => {}
                Raw::KeyboardLost => inputs.push(UiInput::KeyboardLost),
                Raw::OpenSettings => inputs.push(UiInput::OpenSettings),
                Raw::ClipboardChanged => inputs.push(UiInput::ClipboardChanged),
                Raw::DevicesChanged if !inputs.contains(&UiInput::DevicesChanged) => inputs.push(UiInput::DevicesChanged),
                Raw::DevicesChanged => {},
                Raw::SessionUnavailable => inputs.push(UiInput::SessionUnavailable),
                Raw::Hotkey(id) => inputs.push(UiInput::Hotkey(id)),
                Raw::DragFiles(hwnd, over) => {
                    if let Some(window) = self.windows.iter().position(|w| w.hwnd == hwnd) { inputs.push(UiInput::FilesOver { window, over }); }
                }
                Raw::FilesDropped(hwnd, paths) => {
                    if let Some(window) = self.windows.iter().position(|w| w.hwnd == hwnd) { inputs.push(UiInput::FilesDropped { window, paths }); }
                }
                #[cfg(windows)]
                Raw::Char(hwnd, unit) => {
                    let Some(window) = self.windows.iter().position(|w| w.hwnd == hwnd) else { continue; };
                    let units = match (self.surrogate.take(), unit) {
                        (_, 0xD800..=0xDBFF) => { self.surrogate = Some(unit); continue; }
                        (Some(high), 0xDC00..=0xDFFF) => vec![high, unit],
                        _ => vec![unit],
                    };
                    for ch in char::decode_utf16(units).flatten() { inputs.push(UiInput::Char { window, ch }); }
                }
                #[cfg(not(windows))]
                Raw::Char(..) => {}
            }
        }
        inputs
    }
}

#[cfg(not(windows))]
impl MultiMonitorOverlay {
    pub fn create_windows(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn destroy_windows(&mut self) {}
    fn step_aside(&mut self) {}
    pub fn begin_slider_drag(&mut self, _: usize) -> Result<()> { Ok(()) }
    pub fn take_keyboard(&mut self, _: usize) -> Result<()> { Ok(()) }
    pub fn release_keyboard(&mut self) {}
    pub fn end_slider_drag(&mut self) {}

    /// Window messages: none without Windows; `false` would mean quit.
    pub fn pump(&mut self) -> bool {
        true
    }

    pub fn present(&mut self, _: usize, _: &PillGeometry, _: &IslandState, _: &IsleConfig, _: HitArea,
    ) -> Result<()> {
        Ok(())
    }
}

/// Wakes the UI thread from another thread (a service's event), or lets it sleep until then.
#[derive(Clone)]
pub struct Waker {
    #[cfg(windows)]
    thread: u32,
    #[cfg(not(windows))]
    signal: std::sync::Arc<(Mutex<bool>, std::sync::Condvar)>,
}

impl Waker {
    /// Made on the UI thread, after its windows (which give it a message queue).
    pub fn for_this_thread() -> Self {
        Self {
            #[cfg(windows)]
            thread: unsafe { windows::Win32::System::Threading::GetCurrentThreadId() },
            #[cfg(not(windows))]
            signal: Default::default(),
        }
    }

    pub fn wake(&self) {
        #[cfg(windows)]
        unsafe {
            use windows::Win32::Foundation::{LPARAM, WPARAM};
            use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_APP};
            let _ = PostThreadMessageW(self.thread, WM_APP, WPARAM(0), LPARAM(0));
        }
        #[cfg(not(windows))]
        {
            let (lock, cvar) = &*self.signal;
            *lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = true;
            cvar.notify_one();
        }
    }

    /// Sleeps until a window message, a `wake`, or `timeout` (`None`: no deadline).
    /// A deadline is kept on a high-resolution waitable timer (port of mac-island 3.1.1's FrameTimer):
    /// a plain timeout ends on the system's 15.6 ms tick, so a 16 ms frame wait took ~31 ms.
    pub fn wait(&self, timeout: Option<Duration>) {
        #[cfg(windows)]
        unsafe {
            use windows::Win32::Foundation::HANDLE;
            use windows::Win32::System::Threading::{CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateWaitableTimerExW, SetWaitableTimer, TIMER_MODIFY_STATE};
            use windows::Win32::UI::WindowsAndMessaging::{MsgWaitForMultipleObjects, QS_ALLINPUT};
            // One per thread, for the thread's life (SYNCHRONIZE: 0x0010_0000, to wait on it).
            thread_local! { static TIMER: Option<HANDLE> = unsafe {
                CreateWaitableTimerExW(None, None, CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_MODIFY_STATE.0 | 0x0010_0000).ok()
            }; }
            let ms = timeout.map_or(u32::MAX, |t| t.as_millis().min(u32::MAX as u128 - 1) as u32);
            let timer = timeout.filter(|t| *t < Duration::from_secs(1)).and_then(|t| TIMER.with(|timer| {
                let due = -((t.as_nanos() / 100).max(1) as i64);
                timer.filter(|timer| SetWaitableTimer(*timer, &due, 0, None, None, false).is_ok())
            }));
            match timer {
                // The message wake still comes first; `ms` + 1 only guards a timer that never fires.
                Some(timer) => MsgWaitForMultipleObjects(Some(&[timer]), false, ms + 1, QS_ALLINPUT),
                None => MsgWaitForMultipleObjects(None, false, ms, QS_ALLINPUT),
            };
        }
        #[cfg(not(windows))]
        {
            let (lock, cvar) = &*self.signal;
            let woken = lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut woken = match timeout {
                Some(t) => cvar
                    .wait_timeout_while(woken, t, |w| !*w)
                    .map_or_else(|e| e.into_inner().0, |r| r.0),
                None => cvar
                    .wait_while(woken, |w| !*w)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            };
            *woken = false;
        }
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
    };
    use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
    use windows::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor, MDT_EFFECTIVE_DPI,
        SetProcessDpiAwarenessContext,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetCapture, ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
    };
    use windows::Win32::UI::Shell::{QUNS_ACCEPTS_NOTIFICATIONS, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
    };
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::core::{PWSTR, w};

    /// A top-down 32-bit DIB selected into its own memory DC: what UpdateLayeredWindow reads. Made once
    /// per window instead of each frame (with a copy of the whole canvas into it).
    pub(super) struct Surface {
        pub(super) dc: HDC,
        bitmap: HBITMAP,
        old: HGDIOBJ,
        pub(super) bits: *mut u8,
        pub(super) width: u32,
        pub(super) height: u32,
    }

    impl Surface {
        pub(super) fn new(width: u32, height: u32) -> Option<Self> {
            unsafe {
                let bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: width as i32,
                        biHeight: -(height as i32), // top-down
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
                let screen = GetDC(None);
                let bitmap = CreateDIBSection(screen, &bmi, DIB_RGB_COLORS, &mut bits, None, 0);
                let dc = CreateCompatibleDC(screen);
                ReleaseDC(None, screen);
                let bitmap = match bitmap {
                    Ok(bitmap) if !bits.is_null() && !dc.is_invalid() => bitmap,
                    other => {
                        if let Ok(bitmap) = other { let _ = DeleteObject(bitmap); }
                        if !dc.is_invalid() { let _ = DeleteDC(dc); }
                        return None;
                    }
                };
                let old = SelectObject(dc, bitmap);
                Some(Self { dc, bitmap, old, bits: bits as *mut u8, width, height })
            }
        }
    }

    impl Drop for Surface {
        fn drop(&mut self) {
            unsafe {
                SelectObject(self.dc, self.old);
                let _ = DeleteObject(self.bitmap);
                let _ = DeleteDC(self.dc);
            }
        }
    }

    /// In `Win32::UI::Controls`, a feature not needed for anything else.
    const WM_MOUSELEAVE: u32 = 0x02A3;
    thread_local! { static RELEASING_CAPTURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

    /// The message `Cimori --settings` posts to a running island (registered once per process).
    pub fn show_settings_message() -> u32 {
        static MESSAGE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
        *MESSAGE.get_or_init(|| unsafe { RegisterWindowMessageW(w!("CimoriShowSettings")) })
    }

    /// Asks a Cimori already running to show its settings; false when none runs.
    pub fn ask_running_to_open_settings() -> bool {
        unsafe {
            match FindWindowW(w!("CimoriPillOverlayClass"), None) {
                Ok(hwnd) if !hwnd.is_invalid() => PostMessageW(hwnd, show_settings_message(), WPARAM(0), LPARAM(0)).is_ok(),
                _ => false,
            }
        }
    }

    /// Sharp on every monitor: the process draws at each one's own DPI. Once, before any window.
    pub fn init_dpi() {
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
    }

    unsafe extern "system" fn collect(monitor: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM,
    ) -> BOOL {
        let places = unsafe { &mut *(data.0 as *mut Vec<MonitorPlace>) };
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if unsafe { GetMonitorInfoW(monitor, &mut info as *mut _ as *mut MONITORINFO) }.as_bool() {
            let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
            let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
            let end = info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len());
            let rc = info.monitorInfo.rcMonitor;
            places.push(MonitorPlace {
                device: String::from_utf16_lossy(&info.szDevice[..end]),
                left: rc.left,
                top: rc.top,
                width: rc.right - rc.left,
                scale: dpi_x.max(48) as f32 / 96.0,
                primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }
        true.into()
    }

    fn cursor_monitor() -> Option<String> {
        let mut point = POINT::default();
        unsafe { GetCursorPos(&mut point) }.ok()?;
        let monitor = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST) };
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if !unsafe { GetMonitorInfoW(monitor, &mut info as *mut _ as *mut MONITORINFO) }.as_bool() {
            return None;
        }
        let end = info.szDevice.iter().position(|&ch| ch == 0).unwrap_or(info.szDevice.len());
        Some(String::from_utf16_lossy(&info.szDevice[..end]))
    }

    pub fn list_monitors() -> Vec<MonitorPlace> {
        let mut places: Vec<MonitorPlace> = Vec::new();
        unsafe {
            let _ = EnumDisplayMonitors(HDC::default(), None, Some(collect), LPARAM(&mut places as *mut _ as isize),
            );
        }
        places
    }

    /// Raw input for `take_inputs`; HTTRANSPARENT outside the pill so clicks reach the windows below.
    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
    ) -> LRESULT {
        let point = || {
            ((lparam.0 & 0xFFFF) as i16 as i32, ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
            )
        };
        match msg {
            WM_NCHITTEST => {
                let (x, y) = point();
                LRESULT(if pill_contains(hwnd, x, y) { HTCLIENT as isize } else { HTTRANSPARENT as isize })
            }
            WM_MOUSEMOVE => {
                let mut track = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = unsafe { TrackMouseEvent(&mut track) };
                push_raw(Raw::Enter(hwnd.0 as isize));
                let (px, py) = point();
                push_raw(Raw::Move(hwnd.0 as isize, px, py));
                if wparam.0 & 1 != 0 {
                    let (x,y) = point(); push_raw(Raw::Drag(hwnd.0 as isize, x, y));
                }
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                push_raw(Raw::Leave(hwnd.0 as isize));
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let delta = (wparam.0 >> 16) as u16 as i16 as i32;
                let (x, y) = point();
                let mut pt = POINT { x, y };
                let _ = unsafe { ScreenToClient(hwnd, &mut pt) };
                push_raw(Raw::Wheel(hwnd.0 as isize, delta, pt.x, pt.y));
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let (x,y) = point(); push_raw(Raw::Down(hwnd.0 as isize, x, y));
                LRESULT(0)
            }
            WM_CAPTURECHANGED => {
                if !RELEASING_CAPTURE.get() { push_raw(Raw::CancelDrag); }
                LRESULT(0)
            }
            WM_RBUTTONUP => {
                let (x,y) = point(); push_raw(Raw::RightClick(hwnd.0 as isize,x,y));
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let (x, y) = point();
                push_raw(Raw::Click(hwnd.0 as isize, x, y));
                LRESULT(0)
            }
            WM_CHAR => {
                push_raw(Raw::Char(hwnd.0 as isize, wparam.0 as u16));
                LRESULT(0)
            }
            WM_ACTIVATE => {
                if wparam.0 & 0xFFFF == WA_INACTIVE as usize { push_raw(Raw::KeyboardLost); }
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            m if m == show_settings_message() => {
                push_raw(Raw::OpenSettings);
                LRESULT(0)
            }
            WM_CLIPBOARDUPDATE => {
                push_raw(Raw::ClipboardChanged);
                LRESULT(0)
            }
            WM_DEVICECHANGE => {
                push_raw(Raw::DevicesChanged);
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_WTSSESSION_CHANGE if wparam.0 == WTS_SESSION_LOCK as usize => {
                push_raw(Raw::SessionUnavailable);
                LRESULT(0)
            }
            WM_POWERBROADCAST if wparam.0 == PBT_APMSUSPEND as usize => {
                push_raw(Raw::SessionUnavailable);
                LRESULT(1)
            }
            WM_HOTKEY => {
                push_raw(Raw::Hotkey(wparam.0 as i32));
                LRESULT(0)
            }
            WM_DISPLAYCHANGE | WM_DPICHANGED => {
                push_raw(Raw::Changed);
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    /// Whether screen point `x`, `y` is on the pill as last drawn (the rest of the window lets the mouse through).
    fn pill_contains(hwnd: HWND, x: i32, y: i32) -> bool {
        let packed = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as u64;
        let Some((area, scale)) = HitArea::unpack(packed) else { return false; };
        let mut pt = POINT { x, y };
        let _ = unsafe { ScreenToClient(hwnd, &mut pt) };
        area.contains(pt.x as f32 / scale, pt.y as f32 / scale)
    }

    use windows::Win32::Foundation::POINTL;
    use windows::Win32::System::Com::{FORMATETC, IDataObject};
    use windows::Win32::System::Ole::{DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl, RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop};
    use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;

    /// CF_HDROP in an HGLOBAL (DVASPECT_CONTENT, TYMED_HGLOBAL): files from Explorer and most apps.
    const HDROP_FORMAT: FORMATETC = FORMATETC { cfFormat: 15, ptd: std::ptr::null_mut(), dwAspect: 1, lindex: -1, tymed: 1 };

    /// Port of mac-island's DropTarget and Island::DragOverAt/DragGone/DropAt (3.1.1): files dragged over
    /// the pill, closed or open, go on the shelf when let go. Called by OLE on the UI thread (registered there),
    /// so it reports through `push_raw` like the window procedure.
    #[windows::core::implement(IDropTarget)]
    struct DropTarget {
        hwnd: isize,
        accepts: std::rc::Rc<std::cell::Cell<bool>>,
        files: std::cell::Cell<bool>,
        over: std::cell::Cell<bool>,
    }

    impl DropTarget {
        fn follow(&self, pt: &POINTL, effect: *mut DROPEFFECT) {
            let over = self.files.get() && self.accepts.get() && pill_contains(HWND(self.hwnd as *mut _), pt.x, pt.y);
            if over != self.over.replace(over) { push_raw(Raw::DragFiles(self.hwnd, over)); }
            if let Some(effect) = unsafe { effect.as_mut() } {
                *effect = if over && effect.0 & DROPEFFECT_COPY.0 != 0 { DROPEFFECT_COPY } else { DROPEFFECT_NONE };
            }
        }
    }

    impl IDropTarget_Impl for DropTarget_Impl {
        fn DragEnter(&self, data: Option<&IDataObject>, _: MODIFIERKEYS_FLAGS, pt: &POINTL, effect: *mut DROPEFFECT) -> windows::core::Result<()> {
            self.files.set(data.is_some_and(|data| unsafe { data.QueryGetData(&HDROP_FORMAT) }.is_ok()));
            self.follow(pt, effect);
            Ok(())
        }
        fn DragOver(&self, _: MODIFIERKEYS_FLAGS, pt: &POINTL, effect: *mut DROPEFFECT) -> windows::core::Result<()> {
            self.follow(pt, effect);
            Ok(())
        }
        fn DragLeave(&self) -> windows::core::Result<()> {
            if self.over.replace(false) { push_raw(Raw::DragFiles(self.hwnd, false)); }
            Ok(())
        }
        fn Drop(&self, data: Option<&IDataObject>, _: MODIFIERKEYS_FLAGS, pt: &POINTL, effect: *mut DROPEFFECT) -> windows::core::Result<()> {
            self.follow(pt, effect);
            if self.over.replace(false) {
                push_raw(Raw::DragFiles(self.hwnd, false));
                if let Some(paths) = data.and_then(dropped_files) { push_raw(Raw::FilesDropped(self.hwnd, paths)); }
            }
            Ok(())
        }
    }

    fn dropped_files(data: &IDataObject) -> Option<Vec<std::path::PathBuf>> {
        use std::os::windows::ffi::OsStringExt;
        use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
        let mut medium = unsafe { data.GetData(&HDROP_FORMAT) }.ok()?;
        let drop = HDROP(unsafe { medium.u.hGlobal }.0);
        let mut files = Vec::new();
        unsafe {
            for i in 0..DragQueryFileW(drop, u32::MAX, None).min(32) {
                let size = DragQueryFileW(drop, i, None) as usize;
                if !(1..=32_767).contains(&size) { continue; }
                let mut name = vec![0; size + 1];
                let copied = DragQueryFileW(drop, i, Some(&mut name)) as usize;
                if copied == size {
                    files.push(std::ffi::OsString::from_wide(&name[..copied]).into());
                }
            }
            ReleaseStgMedium(&mut medium);
        }
        Some(files)
    }

    impl MultiMonitorOverlay {
        pub fn smart_hide(&self, index: usize, mode: isle_core::config::HideWhen, width: f32, height: f32, gap: f32) -> bool {
            use isle_core::config::HideWhen;
            if mode == HideWhen::MouseAway { return true; }
            let Some(window) = self.windows.get(index) else { return false; };
            let place = &window.place;
            let w = (width * place.scale).round() as i32;
            let h = (height * place.scale).round() as i32;
            let left = place.left + (place.width - w) / 2;
            let top = place.top + (gap * place.scale).round() as i32;
            let mut query = HideQuery {
                monitor: unsafe { MonitorFromPoint(POINT { x: place.left + 1, y: place.top + 1 }, MONITOR_DEFAULTTONULL) },
                target: RECT { left, top, right: left + w, bottom: top + h },
                maximized: mode == HideWhen::Maximized,
                found: false,
            };
            unsafe { let _ = EnumWindows(Some(find_hiding_window), LPARAM((&mut query as *mut HideQuery) as isize)); }
            query.found
        }

        pub fn cursor_monitor_changed(&self) -> bool {
            cursor_monitor().is_some_and(|device| {
                self.windows.first().is_some_and(|window| window.place.device != device)
            })
        }

        pub fn native_window(&self) -> Option<HWND> {
            self.windows.first().map(|window| HWND(window.hwnd as *mut _))
        }

        pub fn begin_slider_drag(&mut self, window: usize) -> Result<()> {
            self.end_slider_drag();
            let Some(window) = self.windows.get(window) else { anyhow::bail!("Janela removida"); };
            let hwnd = HWND(window.hwnd as *mut _);
            unsafe {
                let previous = GetForegroundWindow();
                // Background no-activate windows cannot capture outside their bounds: activate only during a slider drag.
                anyhow::ensure!(previous == hwnd || SetForegroundWindow(hwnd).as_bool(), "Windows recusou ativar o slider");
                self.drag_foreground = Some((window.hwnd, previous.0 as isize));
                SetCapture(hwnd);
                if GetCapture() != hwnd {
                    self.end_slider_drag();
                    anyhow::bail!("Windows recusou capturar o mouse");
                }
            }
            Ok(())
        }

        /// For a reply: the island comes to the front with the keyboard (it's made never to
        /// activate by itself), like a slider drag. Only after a click on it (Windows' rule).
        pub fn take_keyboard(&mut self, window: usize) -> Result<()> {
            let Some(island) = self.windows.get(window) else { anyhow::bail!("Janela removida"); };
            let hwnd = HWND(island.hwnd as *mut _);
            unsafe {
                let previous = GetForegroundWindow();
                anyhow::ensure!(previous == hwnd || SetForegroundWindow(hwnd).as_bool(), "Windows recusou dar o teclado à ilha");
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(hwnd);
                if self.keyboard_foreground.is_none_or(|(w, _)| w != island.hwnd) {
                    self.keyboard_foreground = Some((island.hwnd, previous.0 as isize));
                }
            }
            Ok(())
        }

        /// The reply is over: the window that was in front gets the keyboard back.
        pub fn release_keyboard(&mut self) {
            if let Some((window, previous)) = self.keyboard_foreground.take() {
                unsafe {
                    if GetForegroundWindow().0 as isize == window && previous != 0 && previous != window {
                        let _ = SetForegroundWindow(HWND(previous as *mut _));
                    }
                }
            }
        }

        pub fn end_slider_drag(&mut self) {
            if let Some((window, previous)) = self.drag_foreground.take() {
                unsafe {
                    let hwnd = HWND(window as *mut _);
                    if GetCapture() == hwnd {
                        // ReleaseCapture synchronously sends WM_CAPTURECHANGED; do not cancel a later queued drag.
                        RELEASING_CAPTURE.set(true);
                        let _ = ReleaseCapture();
                        RELEASING_CAPTURE.set(false);
                    }
                    if GetForegroundWindow() == hwnd && previous != 0 && previous != window {
                        let _ = SetForegroundWindow(HWND(previous as *mut _));
                    }
                }
            }
        }

        pub fn create_windows(&mut self) -> Result<()> {
            unsafe {
                let instance = GetModuleHandleW(None)?;
                let class_name = w!("CimoriPillOverlayClass");
                let wc = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    lpfnWndProc: Some(wnd_proc),
                    hInstance: instance.into(),
                    hCursor: LoadCursorW(None, IDC_ARROW)?,
                    lpszClassName: class_name,
                    ..Default::default()
                };
                // Registered once; again (after monitors changed) it reports "already exists", harmlessly.
                let _ = RegisterClassExW(&wc);

                let found = list_monitors();
                let cursor_place = self.follow_mouse.then(cursor_monitor).flatten()
                    .and_then(|device| found.iter().find(|place| place.device == device));
                let chosen = if let Some(place) = cursor_place {
                    vec![place.clone()]
                } else if self.all_monitors {
                    choose_every_monitor(&found, self.main_monitor.as_deref())
                } else {
                    choose_monitors(&self.monitors, &found, self.main_monitor.as_deref())
                };
                tracing::info!(
                    "monitores: {:?}; ilhas em {:?} (a primeira é a principal)",
                    found.iter().map(|m| format!("{} {}x{}%", m.device, m.width, (m.scale * 100.0).round())).collect::<Vec<_>>(),
                    chosen.iter().map(|m| m.device.as_str()).collect::<Vec<_>>()
                );
                for place in chosen {
                    let renderer = PillRenderer::with_scale(CANVAS_MAX_WIDTH as u32, CANVAS_MAX_HEIGHT as u32, place.scale,
                    );
                    let fb_w = (CANVAS_MAX_WIDTH as f32 * renderer_scale(&place)).round() as i32;
                    let fb_h = (CANVAS_MAX_HEIGHT as f32 * renderer_scale(&place)).round() as i32;
                    let hwnd = CreateWindowExW(
                        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
                        class_name,
                        w!("Cimori"),
                        WS_POPUP | WS_VISIBLE,
                        place.left + (place.width - fb_w) / 2,
                        place.top,
                        fb_w,
                        fb_h,
                        None,
                        None,
                        instance,
                        None,
                    )?;
                    let drops = std::rc::Rc::new(std::cell::Cell::new(false));
                    let target: IDropTarget = DropTarget { hwnd: hwnd.0 as isize, accepts: drops.clone(), files: Default::default(), over: Default::default() }.into();
                    if let Err(err) = RegisterDragDrop(hwnd, &target) { tracing::warn!("a ilha não recebe arquivos arrastados: {err}"); }
                    self.windows.push(IslandWindow {
                        glass: Default::default(),
                        hwnd: hwnd.0 as isize,
                        place,
                        renderer,
                        inside: false,
                        hidden: false,
                        drops,
                        surface: None,
                    });
                }
                // Out of context: delivered through this thread's message queue, no DLL injected.
                let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
                for (first, last) in [(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND), (EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE),
                ] {
                    let hook = SetWinEventHook(first, last, None, Some(window_event), 0, 0, flags);
                    if !hook.is_invalid() { self.hooks.push(hook.0 as isize); }
                }
            }
            self.step_aside();
            Ok(())
        }

        /// Hides each island whose monitor has a full-screen app in front, shows it again after.
        pub(super) fn step_aside(&mut self) {
            let stay = self.over_fullscreen;
            for w in &mut self.windows {
                let hide = !stay && unsafe { full_screen_on(&w.place) };
                if hide != w.hidden {
                    w.hidden = hide;
                    w.glass.restore_visibility(!hide);
                    unsafe { let _ = ShowWindow(HWND(w.hwnd as *mut _), if hide { SW_HIDE } else { SW_SHOWNOACTIVATE },
                        ); }
                }
            }
        }

        pub fn destroy_windows(&mut self) {
            for hook in self.hooks.drain(..) {
                unsafe { let _ = UnhookWinEvent(HWINEVENTHOOK(hook as *mut _)); }
            }
            self.end_slider_drag();
            for w in self.windows.drain(..) {
                unsafe {
                    let _ = RevokeDragDrop(HWND(w.hwnd as *mut _));
                    let _ = DestroyWindow(HWND(w.hwnd as *mut _));
                }
            }
        }

        /// Window messages, dispatched to `wnd_proc`; `false` once the app is asked to quit.
        pub fn pump(&mut self) -> bool {
            unsafe {
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    if msg.message == WM_QUIT {
                        return false;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            true
        }

        /// Draws island `window` at its monitor's scale, and tells it where the mouse is taken.
        pub fn present(&mut self, window: usize, geom: &PillGeometry, state: &IslandState, cfg: &IsleConfig, hit: HitArea,
        ) -> Result<()> {
            if let Some(w) = self.windows.get_mut(window) {
                // Files go on the shelf from the closed pill (shelf_drop_pill) or its open Shelf tab.
                w.drops.set(cfg.modules.shelf && (cfg.shelf_drop_pill || state.mode == PillMode::Expanded(isle_core::state::TabPage::Shelf)));
                let scale=renderer_scale(&w.place);
                let glass_on=if cfg.liquid_glass && !w.hidden && state.mode!=PillMode::Minimized {
                    let width=geom.width.value.clamp(4.,CANVAS_MAX_WIDTH as f32-8.);
                    let height=geom.height.value.clamp(2.,CANVAS_MAX_HEIGHT as f32-12.);
                    crate::glass::glass_path(CANVAS_MAX_WIDTH as f32,width,height,geom.corner_radius.value,cfg.top_gap(),
                        cfg.ui.pill_style==isle_core::config::PillStyle::Notch,cfg.notch_curve,scale)
                        .is_some_and(|path|w.glass.update(HWND(w.hwnd as *mut _),&path,matches!(state.mode,PillMode::Expanded(_)),true))
                } else {w.glass.disable();false};
                // Windows may refuse its host backdrop (e.g. transparency effects off): preserve readability.
                let mut fallback;
                let render_cfg=if cfg.liquid_glass && !glass_on {fallback=cfg.clone();fallback.liquid_glass=false;&fallback}else{cfg};
                let started = std::time::Instant::now();
                let fb = w.renderer.render(geom, state, render_cfg);
                let drawn = started.elapsed();
                let (x0, y0, x1, y1) = fb.dirty;
                unsafe {
                    let hwnd = HWND(w.hwnd as *mut _);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, hit.pack(w.place.scale) as isize);
                    // One DIB per window, kept: only the rows and columns the frame changed are copied
                    // into it, and only they are handed to the compositor (prcDirty).
                    if w.surface.as_ref().is_none_or(|s| (s.width, s.height) != (fb.width, fb.height)) {
                        w.surface = Surface::new(fb.width, fb.height);
                    }
                    let Some(surface) = &w.surface else { return Ok(()) };
                    let stride = fb.width as usize * 4;
                    if x1 > x0 && y1 > y0 {
                        for y in y0..y1 {
                            let (from, to) = (y * stride + x0 * 4, y * stride + x1 * 4);
                            std::ptr::copy_nonoverlapping(fb.bgra_premultiplied[from..to].as_ptr(), surface.bits.add(from), to - from);
                        }
                        let size = SIZE { cx: fb.width as i32, cy: fb.height as i32 };
                        let src = POINT { x: 0, y: 0 };
                        let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
                        let dirty = RECT { left: x0 as i32, top: y0 as i32, right: x1 as i32, bottom: y1 as i32 };
                        let info = UPDATELAYEREDWINDOWINFO {
                            cbSize: std::mem::size_of::<UPDATELAYEREDWINDOWINFO>() as u32,
                            psize: &size,
                            hdcSrc: surface.dc,
                            pptSrc: &src,
                            pblend: &blend,
                            dwFlags: ULW_ALPHA,
                            prcDirty: &dirty,
                            ..Default::default()
                        };
                        let _ = UpdateLayeredWindowIndirect(hwnd, &info);
                    }
                    tracing::debug!(target: "cimori::frame", "ilha {window}: desenho {:.2} ms, tela {:.2} ms, {}x{} px",
                        drawn.as_secs_f64() * 1e3, (started.elapsed() - drawn).as_secs_f64() * 1e3, x1.saturating_sub(x0), y1.saturating_sub(y0));
                }
            }
            Ok(())
        }
    }

    unsafe extern "system" fn window_event(_: HWINEVENTHOOK, event: u32, hwnd: HWND, object: i32, child: i32, _: u32, _: u32,
    ) {
        // Location changes fire for every caret and cursor: only the front window's own matter.
        if event == EVENT_SYSTEM_FOREGROUND || (object == OBJID_WINDOW.0 && child == 0 && hwnd == unsafe { GetForegroundWindow() }) {
            push_raw(Raw::Foreground);
        }
    }

    /// Windows' screens over everything for a moment (Start, search, the desktop), not apps.
    unsafe fn is_shell(hwnd: HWND) -> bool {
        let mut class = [0u16; 32];
        let n = unsafe { GetClassNameW(hwnd, &mut class) } as usize;
        let class = String::from_utf16_lossy(&class[..n]);
        if ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd",
        ].iter().any(|c| c.eq_ignore_ascii_case(&class)) {
            return true;
        }
        let mut cloaked = 0u32;
        let _ = unsafe { DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut _ as *mut _, 4) };
        if cloaked != 0 {
            return true;
        }
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        let Ok(process) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else { return false;
        };
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let ok = unsafe { QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len,
            ) }.is_ok();
        let _ = unsafe { CloseHandle(process) };
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        let exe = path.rsplit('\\').next().unwrap_or_default();
        ok && ["explorer.exe", "ShellExperienceHost.exe", "StartMenuExperienceHost.exe", "SearchHost.exe",
            "ScreenClippingHost.exe", "SnippingTool.exe",
        ].iter().any(|e| e.eq_ignore_ascii_case(exe))
    }

    unsafe fn covers(hwnd: HWND, monitor: HMONITOR, area: RECT, busy: bool) -> bool {
        let hwnd = unsafe { GetAncestor(hwnd, GA_ROOT) };
        let mut rect = RECT::default();
        !hwnd.is_invalid() && unsafe { IsWindowVisible(hwnd) }.as_bool()
            && unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL) } == monitor
            && unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok()
            && !unsafe { is_shell(hwnd) }
            && covers_monitor((rect.left, rect.top, rect.right, rect.bottom), (area.left, area.top, area.right, area.bottom),
                unsafe { IsZoomed(hwnd) }.as_bool(),
                (unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32 & WS_CAPTION.0) == WS_CAPTION.0, busy,
            )
    }

    struct HideQuery {
        monitor: HMONITOR,
        target: RECT,
        maximized: bool,
        found: bool,
    }

    // Adapted from Mac Island & Dock by caliberda, GPL-3.0 (WindowInTheWay).
    unsafe extern "system" fn find_hiding_window(hwnd: HWND, data: LPARAM) -> BOOL {
        let query = unsafe { &mut *(data.0 as *mut HideQuery) };
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)); }
        if pid == unsafe { windows::Win32::System::Threading::GetCurrentProcessId() }
            || !unsafe { IsWindowVisible(hwnd) }.as_bool()
            || unsafe { IsIconic(hwnd) }.as_bool()
            || unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL) } != query.monitor
        {
            return true.into();
        }
        let matched = if query.maximized {
            unsafe { IsZoomed(hwnd) }.as_bool()
        } else {
            let mut bounds = RECT::default();
            unsafe { GetWindowRect(hwnd, &mut bounds) }.is_ok()
                && bounds.left < query.target.right && bounds.right > query.target.left
                && bounds.top < query.target.bottom && bounds.bottom > query.target.top
        };
        if matched && !unsafe { is_shell(hwnd) } {
            query.found = true;
            return false.into();
        }
        true.into()
    }

    /// Mac Island & Dock's IsFullscreenAppOn: Direct3D exclusive or a presentation, as Windows reports it;
    /// else the window in front, or the one in the middle of the screen, covering this monitor.
    unsafe fn full_screen_on(place: &MonitorPlace) -> bool {
        let monitor = unsafe { MonitorFromPoint(POINT { x: place.left + 1, y: place.top + 1,
                },
                MONITOR_DEFAULTTONULL,
            ) };
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if monitor.is_invalid() || !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            return false;
        }
        let state = unsafe { SHQueryUserNotificationState() }.unwrap_or(QUNS_ACCEPTS_NOTIFICATIONS);
        let front = unsafe { GetForegroundWindow() };
        // Exclusive full screen only counts on the monitor of the window in front.
        if (state == QUNS_RUNNING_D3D_FULL_SCREEN || state == QUNS_PRESENTATION_MODE)
            && unsafe { MonitorFromWindow(front, MONITOR_DEFAULTTONULL) } == monitor {
            return true;
        }
        let busy = state == QUNS_BUSY;
        let area = info.rcMonitor;
        let middle = POINT { x: (area.left + area.right) / 2, y: (area.top + area.bottom) / 2,
        };
        unsafe { covers(front, monitor, area, busy) || covers(WindowFromPoint(middle), monitor, area, false) }
    }

    /// The scale the renderer clamps to, so the window is exactly its framebuffer's size.
    fn renderer_scale(place: &MonitorPlace) -> f32 {
        place.scale.clamp(0.5, 4.0)
    }
}

#[cfg(windows)]
pub use win::{ask_running_to_open_settings, init_dpi, list_monitors};

#[cfg(not(windows))]
pub fn init_dpi() {}
#[cfg(not(windows))]
pub fn ask_running_to_open_settings() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_screen_means_covering_the_monitor_like_the_mod() {
        let monitor = (0, 0, 1920, 1080);
        // Borderless game or browser video: exactly the screen, whatever its style.
        assert!(covers_monitor((0, 0, 1920, 1080), monitor, true, true, false));
        // A maximized window spills past the edges: not full screen.
        assert!(!covers_monitor((-8, -8, 1928, 1088), monitor, true, true, false));
        // Captioned window stretched over it: only when Windows says an app is busy full screen.
        assert!(!covers_monitor((-1, -1, 1921, 1081), monitor, false, true, false));
        assert!(covers_monitor((-1, -1, 1921, 1081), monitor, false, true, true));
        // Borderless and bigger than the screen still covers it.
        assert!(covers_monitor((-1, -1, 1921, 1081), monitor, false, false, false));
        // Smaller, or on the other monitor: no.
        assert!(!covers_monitor((0, 0, 1920, 1040), monitor, false, false, false));
        assert!(!covers_monitor((1920, 0, 3840, 1080), monitor, false, false, false));
    }
    use isle_core::state::PermissionDecision;

    fn place(device: &str, primary: bool) -> MonitorPlace {
        MonitorPlace { device: device.into(), left: 0, top: 0, width: 1920, scale: 1.0, primary,
        }
    }

    #[test]
    fn point_inside_pill_respects_dynamic_spring_bounds_and_top_gap() {
        assert!(!point_inside_pill(100, 20, 580, 220.0, 36.0, 6.0));
        assert!(!point_inside_pill(290, 2, 580, 220.0, 36.0, 6.0));
        assert!(point_inside_pill(290, 20, 580, 220.0, 36.0, 6.0));
        assert!(!point_inside_pill(290, 80, 580, 220.0, 36.0, 6.0));
    }

    #[test]
    fn permission_card_buttons_resolve_to_allow_always_and_deny() {
        let mut state = IslandState::default();
        state.mode = PillMode::PermissionPrompt;
        let allow = hit_test_action(180, 100, 580, 360.0, 120.0, 6.0, &state);
        let always = hit_test_action(290, 100, 580, 360.0, 120.0, 6.0, &state);
        let deny = hit_test_action(390, 100, 580, 360.0, 120.0, 6.0, &state);

        assert_eq!(allow, Some(UiAction::Permission(PermissionDecision::Allow, 0)));
        assert_eq!(always, Some(UiAction::Permission(PermissionDecision::Always, 0)));
        assert_eq!(deny, Some(UiAction::Permission(PermissionDecision::Deny, 0)));
    }

    #[test]
    fn monitors_are_found_by_name_or_fall_back_to_the_primary() {
        let all = vec![place(r"\\.\DISPLAY1", false), place(r"\\.\DISPLAY6", true), place(r"\\.\DISPLAY7", false),
        ];
        let both = choose_monitors(&[r"\\.\display1".into(), r"\\.\DISPLAY6".into()], &all, None,
        );
        assert_eq!(both.iter().map(|m| m.device.as_str()).collect::<Vec<_>>(), [r"\\.\DISPLAY6", r"\\.\DISPLAY1"]);
        // Configured names gone (unplugged, renumbered): the primary, never no pill.
        assert_eq!(choose_monitors(&[r"\\.\DISPLAY9".into()], &all, None)[0].device, r"\\.\DISPLAY6");
        assert_eq!(choose_monitors(&[], &[place(r"\\.\DISPLAY2", false)], None)[0].device, r"\\.\DISPLAY2");
        assert!(choose_monitors(&[], &[], None).is_empty());
        // "An island on every monitor" (Mac Island & Dock's iislandAllMonitors), whatever their names.
        let every = choose_every_monitor(&all, None);
        assert_eq!(every.len(), 3);
        assert_eq!(every[0].device, r"\\.\DISPLAY6", "no main chosen: the primary's island is the main one");
        // The main island is the one the user picked (sislandMonitor), first; gone, the primary again.
        assert_eq!(choose_every_monitor(&all, Some(r"\\.\display7"))[0].device, r"\\.\DISPLAY7");
        assert_eq!(choose_every_monitor(&all, Some(r"\\.\DISPLAY9"))[0].device, r"\\.\DISPLAY6");
        assert_eq!(choose_monitors(&[r"\\.\DISPLAY1".into(), r"\\.\DISPLAY6".into()], &all, Some(r"\\.\DISPLAY1"))[0].device, r"\\.\DISPLAY1");
    }

    #[test]
    fn the_hit_area_survives_packing_and_grows_for_the_hidden_line() {
        let area = HitArea { width: 220.0, top: 6.0, bottom: 40.0,
        };
        assert_eq!(HitArea::unpack(area.pack(1.5)), Some((area, 1.5)));
        assert_eq!(HitArea::unpack(0), None);
        assert!(area.contains(360.0, 20.0) && !area.contains(360.0, 2.0) && !area.contains(100.0, 20.0));

        let mut geom = PillGeometry::default();
        (geom.width.value, geom.height.value) = (120.0, 5.0);
        let line = HitArea::of(&geom, 6.0, &PillMode::Minimized);
        assert!(line.contains(360.0, 0.0),
            "the screen's top edge brings it back"
        );
        assert!(line.contains(360.0 - 60.0 - 15.0, 3.0));
        assert!(!line.contains(360.0, 30.0));
    }

    #[test]
    fn queued_clicks_come_out_as_inputs() {
        let mut overlay = MultiMonitorOverlay::new(vec![r"\\.\DISPLAY1".into()], false, None);
        overlay.enqueue_click(180.0, 100.0);
        assert_eq!(overlay.take_inputs(), vec![UiInput::Click { window: 0, x: 180.0, y: 100.0 }]);
        assert!(overlay.take_inputs().is_empty());
    }
}

#[cfg(test)]
mod control_input_tests {
    use super::*;
    #[test]
    fn volume_click_uses_slider_position_and_ignores_unavailable_audio() {
        let mut state = IslandState::default();
        state.mode = PillMode::Expanded(isle_core::state::TabPage::ControlCenter);
        state.audio_available = true;
        let slider = compute_hitboxes(720., 600., 470., 6., &state)
            .into_iter()
            .find(|b| matches!(b.action, UiAction::SetVolume(_)))
            .unwrap();
        let (px, py) = (
            (slider.x + slider.w / 2.) as i32,
            (slider.y + slider.h / 2.) as i32,
        );
        assert_eq!(
            hit_test_action(px, py, 720, 600., 470., 6., &state), Some(UiAction::SetVolume(50)));
        state.audio_available = false;
        assert_eq!(hit_test_action(px, py, 720, 600., 470., 6., &state), None);
    }
}

thread_local! {
    static ANIMATIONS: std::cell::Cell<isle_core::config::AnimationLevel> =
        const { std::cell::Cell::new(isle_core::config::AnimationLevel::System) };
}
/// The settings' animation level for this (the UI) thread.
pub fn set_animation_level(level: isle_core::config::AnimationLevel) {
    ANIMATIONS.with(|a| a.set(level));
}

/// The settings' level, or Windows' preference when it follows the system.
pub fn animations_enabled() -> bool {
    match ANIMATIONS.with(|a| a.get()) {
        isle_core::config::AnimationLevel::On => true,
        isle_core::config::AnimationLevel::Off => false,
        isle_core::config::AnimationLevel::System => system_animations(),
    }
}

/// Windows animation preference. Queried only while processing a UI event/frame.
fn system_animations() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::{Foundation::BOOL, UI::WindowsAndMessaging::*};
        let mut enabled = BOOL(1);
        unsafe {
            let _ = SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some(&mut enabled as *mut _ as *mut _),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            );
        }
        enabled.as_bool()
    }
    #[cfg(not(windows))]
    {
        true
    }
}
