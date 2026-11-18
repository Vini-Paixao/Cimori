// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Ported from Mac Island & Dock 3.1.1: ReceiveTrayIcons, IpcReader,
// PostTrayIconMessage, PostTrayIconRelease and TrayIconNewer.
//
// The optional Windhawk bridge (windhawk/cimori-tray-bridge.wh.cpp) reads the
// tray icons apps give the taskbar inside explorer.exe and sends them to this
// hidden window. With it, every icon is known with its owner window, ID and
// callback, and clicks go to the app as the taskbar sends them: no taskbar
// buttons needed (the mac-island hides the taskbar). Without it, nothing
// arrives and the UI Automation path in win.rs is used.

use super::win::program_name;
use isle_core::tray::TrayItem;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

// Keep in sync with the bridge mod.
const HOST_CLASS: PCWSTR = w!("CimoriTrayHost");
const IPC_TRAY_ICONS: usize = 0x43494D01;
const IPC_HELLO: usize = 0x43494D02;
const VERSION_UNKNOWN: u32 = u32::MAX;
const HELLO_TIMER: usize = 1;

/// Where a click goes: the icon's owner window, its ID, callback and version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Target {
    pub owner: isize,
    pub id: u32,
    pub callback: u32,
    pub version: u32,
}

/// The bridge's icons (None: no bridge has spoken).
pub(super) type Shared = Arc<Mutex<Option<Vec<(TrayItem, Target)>>>>;

/// One icon as the bridge sends it (the mod's SendTrayIcons order).
#[derive(Debug, Default, PartialEq)]
pub(super) struct Received {
    pub owner: u64,
    pub id: u32,
    pub guid: Option<[u8; 16]>,
    pub callback: u32,
    pub version: u32,
    pub icon: u64,
    pub serial: u32,
    pub hidden: bool,
    pub pid: u32,
    pub tip: String,
    pub key: String,
}

/// The mod's IpcReader: plain values and UTF-16 strings, one after another.
pub(super) fn parse(bytes: &[u8]) -> Option<Vec<Received>> {
    let mut at = 0usize;
    let mut take = |n: usize| -> Option<&[u8]> {
        let slice = bytes.get(at..at.checked_add(n)?)?;
        at += n;
        Some(slice)
    };
    fn u32_of(b: &[u8]) -> u32 { u32::from_le_bytes(b.try_into().unwrap()) }
    let count = u32_of(take(4)?);
    let mut icons = Vec::new();
    for _ in 0..count.min(512) {
        let owner = u64::from_le_bytes(take(8)?.try_into().ok()?);
        let id = u32_of(take(4)?);
        let guid: [u8; 16] = take(16)?.try_into().ok()?;
        let has_guid = take(1)?[0] != 0;
        let callback = u32_of(take(4)?);
        let version = u32_of(take(4)?);
        let icon = u64::from_le_bytes(take(8)?.try_into().ok()?);
        let serial = u32_of(take(4)?);
        let hidden = take(1)?[0] != 0;
        let pid = u32_of(take(4)?);
        let mut string = || -> Option<String> {
            let len = u32_of(take(4)?) as usize;
            let raw = take(len.checked_mul(2)?)?;
            let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            Some(String::from_utf16_lossy(&units))
        };
        let tip = string()?;
        let key = string()?;
        icons.push(Received { owner, id, guid: has_guid.then_some(guid), callback, version, icon, serial, hidden, pid, tip, key });
    }
    Some(icons)
}

/// GUID bytes (little-endian Data1-3) as Windows writes it: {XXXXXXXX-...}.
fn guid_text(g: &[u8; 16]) -> String {
    format!("{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        u32::from_le_bytes(g[0..4].try_into().unwrap()), u16::from_le_bytes([g[4], g[5]]), u16::from_le_bytes([g[6], g[7]]),
        g[8], g[9], g[10], g[11], g[12], g[13], g[14], g[15])
}

/// `c:\...` on a drive letter; not `\\server\...`, `\\?\...` or relative.
fn is_local_path(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() > 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\'
}

/// What the bridge sent, as the tray's items (the program's name, the app's tip under it).
pub(super) fn to_item(icon: &Received, png: Vec<u8>) -> TrayItem {
    // A local drive path only: reading a network path's version info would reach out to that
    // server (and hand it the user's credentials).
    let exe_path = icon.key.rsplit_once('|').map_or("", |(path, _)| path);
    let exe_path = if is_local_path(exe_path) { exe_path.to_owned() } else { String::new() };
    let tip = icon.tip.lines().next().unwrap_or("").trim().to_owned();
    let name = if exe_path.is_empty() { String::new() } else { program_name(&exe_path) };
    TrayItem {
        key: format!("bridge:{}", icon.key),
        display_name: if name.is_empty() { tip.clone() } else { name },
        tooltip: icon.tip.clone(),
        exe_path,
        guid: icon.guid.as_ref().map(guid_text),
        uid: icon.id,
        png,
        pinned: false,
        confirmed: true,
    }
}

/// The icon, as a PNG for the renderer. Tray icons come at 16 px; one made from an app's resource
/// is taken again from it at 32 px, sharp when drawn smaller (the others stay as they came).
fn icon_png(icon: HICON) -> Vec<u8> {
    unsafe {
        let mut ex = ICONINFOEXW { cbSize: std::mem::size_of::<ICONINFOEXW>() as u32, ..Default::default() };
        let from_resource = GetIconInfoExW(icon, &mut ex).as_bool() && (ex.wResID != 0 || ex.szResName[0] != 0);
        // Windows reopens the module named in the icon to read the resource: a local file only.
        let module = String::from_utf16_lossy(&ex.szModName[..ex.szModName.iter().position(|&c| c == 0).unwrap_or(0)]);
        let from_resource = from_resource && is_local_path(&module);
        for bitmap in [ex.hbmColor, ex.hbmMask] {
            if !bitmap.is_invalid() { let _ = DeleteObject(bitmap); }
        }
        if from_resource {
            if let Ok(big) = CopyImage(HANDLE(icon.0), IMAGE_ICON, 32, 32, LR_COPYFROMRESOURCE) {
                let big = HICON(big.0);
                let png = bitmap_png(big);
                let _ = DestroyIcon(big);
                if !png.is_empty() { return png; }
            }
        }
        bitmap_png(icon)
    }
}

/// An icon's own pixels as a PNG: its color bitmap, the mask's for icons without alpha. Empty if
/// Windows can't give them.
fn bitmap_png(icon: HICON) -> Vec<u8> {
    unsafe {
        let mut info = ICONINFO::default();
        if GetIconInfo(icon, &mut info).is_err() { return Vec::new(); }
        let mut bitmap = BITMAP::default();
        let read = |bitmap_handle: HBITMAP, w: i32, h: i32| -> Option<Vec<u8>> {
            let mut header = BITMAPINFO::default();
            header.bmiHeader = BITMAPINFOHEADER { biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w, biHeight: -h, biPlanes: 1, biBitCount: 32, biCompression: BI_RGB.0, ..Default::default() };
            let mut pixels = vec![0u8; (w * h * 4) as usize];
            let dc = GetDC(None);
            let lines = GetDIBits(dc, bitmap_handle, 0, h as u32, Some(pixels.as_mut_ptr().cast()), &mut header, DIB_RGB_COLORS);
            ReleaseDC(None, dc);
            (lines == h).then_some(pixels)
        };
        let mut png = Vec::new();
        if !info.hbmColor.is_invalid()
            && GetObjectW(info.hbmColor, std::mem::size_of::<BITMAP>() as i32, Some((&mut bitmap as *mut BITMAP).cast())) != 0
        {
            let (w, h) = (bitmap.bmWidth, bitmap.bmHeight);
            if (1..=256).contains(&w) && (1..=256).contains(&h) {
                if let Some(mut bgra) = read(info.hbmColor, w, h) {
                    // Old icons draw without alpha: the mask says what's shown.
                    if bgra.chunks_exact(4).all(|p| p[3] == 0) {
                        if let Some(mask) = read(info.hbmMask, w, h) {
                            for (p, m) in bgra.chunks_exact_mut(4).zip(mask.chunks_exact(4)) { p[3] = if m[0] == 0 { 255 } else { 0 }; }
                        }
                    }
                    // BGRA straight to RGBA premultiplied, as tiny-skia keeps it.
                    for p in bgra.chunks_exact_mut(4) {
                        let a = p[3] as u16;
                        let (b, g, r) = (p[0] as u16, p[1] as u16, p[2] as u16);
                        p[0] = (r * a / 255) as u8; p[1] = (g * a / 255) as u8; p[2] = (b * a / 255) as u8;
                    }
                    if let Some(size) = tiny_skia::IntSize::from_wh(w as u32, h as u32) {
                        if let Some(pixmap) = tiny_skia::Pixmap::from_vec(bgra, size) {
                            png = pixmap.encode_png().unwrap_or_default();
                        }
                    }
                }
            }
        }
        if !info.hbmColor.is_invalid() { let _ = DeleteObject(info.hbmColor); }
        if !info.hbmMask.is_invalid() { let _ = DeleteObject(info.hbmMask); }
        png
    }
}

/// The mod's PostTrayIconMessage: newer apps (version 4) get the cursor and the message with the
/// icon's ID; older ones the ID and the message. `newer` forces the newer form.
fn post(target: &Target, message: u32, newer: bool) {
    unsafe {
        let owner = HWND(target.owner as *mut _);
        let _ = AllowSetForegroundWindow(ASFW_ANY);
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let (wparam, lparam) = if newer || (target.version >= 4 && target.version != VERSION_UNKNOWN) {
            (((pt.x as u16 as u32) | ((pt.y as u16 as u32) << 16)) as usize, (message | (target.id << 16)) as isize)
        } else {
            (target.id as usize, message as isize)
        };
        let _ = PostMessageW(owner, target.callback, WPARAM(wparam), LPARAM(lparam));
    }
}

/// A click on the island, passed on as the taskbar does (the mod's ClickTrayIcon): down, up, then
/// NIN_SELECT after a left click or WM_CONTEXTMENU after a right one (PostTrayIconRelease).
pub(super) fn click(target: &Target, right: bool) -> anyhow::Result<()> {
    unsafe {
        anyhow::ensure!(target.callback != 0 && IsWindow(HWND(target.owner as *mut _)).as_bool(), "O app deste ícone não responde a cliques");
    }
    const NIN_SELECT: u32 = WM_USER;
    let unknown = target.version == VERSION_UNKNOWN;
    if right {
        post(target, WM_RBUTTONDOWN, false);
        post(target, WM_RBUTTONUP, false);
        if unknown || target.version >= 3 { post(target, WM_CONTEXTMENU, unknown); }
    } else {
        post(target, WM_LBUTTONDOWN, false);
        post(target, WM_LBUTTONUP, false);
        if unknown || target.version >= 4 { post(target, NIN_SELECT, unknown); }
    }
    Ok(())
}

thread_local! {
    static STATE: RefCell<Option<(Shared, Box<dyn Fn()>, std::collections::HashMap<(String, u32), Vec<u8>>)>> = const { RefCell::new(None) };
}

/// Tells the bridge (on the taskbar) the island is here: it answers with every icon.
unsafe fn hello(host: HWND) {
    let Ok(taskbar) = FindWindowW(w!("Shell_TrayWnd"), None) else { return; };
    let data = COPYDATASTRUCT { dwData: IPC_HELLO, cbData: 0, lpData: std::ptr::null_mut() };
    let mut result = 0usize;
    let _ = SendMessageTimeoutW(taskbar, WM_COPYDATA, WPARAM(host.0 as usize), LPARAM(&data as *const _ as isize),
        SMTO_ABORTIFHUNG | SMTO_ERRORONEXIT, 400, Some(&mut result));
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    static READY: std::sync::OnceLock<(u32, u32)> = std::sync::OnceLock::new();
    let (ready, taskbar_created) = *READY.get_or_init(|| unsafe {
        (RegisterWindowMessageW(w!("CimoriTrayBridge_Ready")), RegisterWindowMessageW(w!("TaskbarCreated")))
    });
    if msg == WM_COPYDATA && lparam.0 != 0 {
        let data = &*(lparam.0 as *const COPYDATASTRUCT);
        // Only the real taskbar (the first Shell_TrayWnd, Explorer's), not any window of that class.
        let from_taskbar = FindWindowW(w!("Shell_TrayWnd"), None).is_ok_and(|taskbar| taskbar.0 as usize == wparam.0);
        if data.dwData == IPC_TRAY_ICONS && from_taskbar && !data.lpData.is_null() {
            let bytes = std::slice::from_raw_parts(data.lpData as *const u8, data.cbData as usize);
            if let Some(icons) = parse(bytes) {
                STATE.with(|state| {
                    if let Some((shared, wake, pngs)) = state.borrow_mut().as_mut() {
                        // Each picture made once per icon change (its serial), while the bridge keeps
                        // the handle alive for this message.
                        let mut kept = std::collections::HashMap::new();
                        let items: Vec<_> = icons.iter().filter(|i| !i.hidden).map(|i| {
                            let png = pngs.remove(&(i.key.clone(), i.serial))
                                .unwrap_or_else(|| if i.icon != 0 { icon_png(HICON(i.icon as *mut _)) } else { Vec::new() });
                            kept.insert((i.key.clone(), i.serial), png.clone());
                            (to_item(i, png), Target { owner: i.owner as isize, id: i.id, callback: i.callback, version: i.version })
                        }).collect();
                        *pngs = kept;
                        // (PNG width: bytes 16..20 of its header.)
                        let sharp = items.iter().filter(|(i, _)| i.png.len() > 24 && u32::from_be_bytes(i.png[16..20].try_into().unwrap()) >= 32).count();
                        tracing::info!("bandeja pela ponte do Windhawk: {} ícones, {sharp} em 32 px", items.len());
                        *shared.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(items);
                        wake();
                    }
                });
            }
            return LRESULT(1);
        }
    }
    // The bridge just loaded, or Explorer restarted (the bridge loads a moment later): ask again.
    if (msg == ready && ready != 0) || (msg == taskbar_created && taskbar_created != 0) {
        hello(hwnd);
        if msg == taskbar_created { SetTimer(hwnd, HELLO_TIMER, 3000, None); }
        return LRESULT(0);
    }
    if msg == WM_TIMER && wparam.0 == HELLO_TIMER {
        let _ = KillTimer(hwnd, HELLO_TIMER);
        hello(hwnd);
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// The host window's thread: says hello once, then waits for the bridge's messages.
pub(super) fn spawn(shared: Shared, wake: impl Fn() + Send + 'static) {
    std::thread::spawn(move || unsafe {
        STATE.with(|state| *state.borrow_mut() = Some((shared, Box::new(wake), Default::default())));
        let instance = GetModuleHandleW(None).unwrap_or_default();
        let class = WNDCLASSW { lpfnWndProc: Some(wnd_proc), hInstance: instance.into(), lpszClassName: HOST_CLASS, ..Default::default() };
        RegisterClassW(&class);
        // A hidden top-level window: FindWindow and the TaskbarCreated broadcast reach it (a
        // message-only window gets neither).
        let Ok(host) = CreateWindowExW(WS_EX_TOOLWINDOW, HOST_CLASS, w!("Cimori"), WS_POPUP, 0, 0, 0, 0, None, None, instance, None) else {
            tracing::warn!("bandeja: janela da ponte não foi criada");
            return;
        };
        hello(host);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(icons: &[Received]) -> Vec<u8> {
        let mut out = (icons.len() as u32).to_le_bytes().to_vec();
        for i in icons {
            out.extend(i.owner.to_le_bytes()); out.extend(i.id.to_le_bytes());
            out.extend(i.guid.unwrap_or([0; 16])); out.push(i.guid.is_some() as u8);
            out.extend(i.callback.to_le_bytes()); out.extend(i.version.to_le_bytes());
            out.extend(i.icon.to_le_bytes()); out.extend(i.serial.to_le_bytes());
            out.push(i.hidden as u8); out.extend(i.pid.to_le_bytes());
            for s in [&i.tip, &i.key] {
                let units: Vec<u16> = s.encode_utf16().collect();
                out.extend((units.len() as u32).to_le_bytes());
                for u in units { out.extend(u.to_le_bytes()); }
            }
        }
        out
    }

    #[test]
    fn a_windows_icon_becomes_a_png_the_renderer_reads() {
        let icon = unsafe { LoadIconW(None, IDI_APPLICATION) }.unwrap();
        let png = icon_png(icon);
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let pixmap = tiny_skia::Pixmap::decode_png(&png).unwrap();
        assert!(pixmap.width() >= 16 && pixmap.pixels().iter().any(|p| p.alpha() > 0));
    }

    #[test]
    fn the_bridges_icon_list_reads_back_and_truncated_data_is_refused() {
        let steam = Received { owner: 0x1234, id: 7, callback: 0x8001, version: 4, icon: 0x99, serial: 3, pid: 42,
            tip: "Steam — Sincronizando".into(), key: r"c:\program files (x86)\steam\steam.exe|7".into(), ..Default::default() };
        let whatsapp = Received { guid: Some([0x78, 0x56, 0x34, 0x12, 0xBC, 0x9A, 0xF0, 0xDE, 1, 2, 3, 4, 5, 6, 7, 8]),
            version: VERSION_UNKNOWN, key: r"c:\apps\whatsapp.exe|{12345678-9ABC-DEF0-0102-030405060708}".into(), ..Default::default() };
        let bytes = write(&[steam, whatsapp]);
        let icons = parse(&bytes).unwrap();
        assert_eq!(icons[0].tip, "Steam — Sincronizando");
        assert_eq!((icons[0].owner, icons[0].callback, icons[0].version), (0x1234, 0x8001, 4));
        assert_eq!(guid_text(icons[1].guid.as_ref().unwrap()), "{12345678-9ABC-DEF0-0102-030405060708}");
        assert!(parse(&bytes[..bytes.len() - 1]).is_none());
        let item = to_item(&icons[1], Vec::new());
        assert_eq!(item.exe_path, r"c:\apps\whatsapp.exe");
        assert_eq!(item.guid.as_deref(), Some("{12345678-9ABC-DEF0-0102-030405060708}"));
        assert!(item.key.starts_with("bridge:") && item.confirmed);
        for remote in [r"\\server\share\app.exe|1", r"\\?\UNC\server\app.exe|1", "app.exe|1"] {
            let icon = Received { key: remote.into(), tip: "Remoto".into(), ..Default::default() };
            assert_eq!(to_item(&icon, Vec::new()).exe_path, "", "{remote}");
        }
    }
}
