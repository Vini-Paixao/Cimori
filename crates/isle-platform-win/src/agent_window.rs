//! The window an AI agent runs in (a terminal, VS Code, Claude's app), brought forward from the island.
//! Port of isle's controller.js `_agentWindow` and its alerts' `show` (Main.activateWindow).
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP, VK_MENU};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::Foundation::BOOL;

/// Brings the agent's window forward: false when none was found or Windows refused.
pub fn focus(pids: &[u32], project: &str) -> bool {
    let Some(hwnd) = find(pids, project) else { return false };
    unsafe {
        if IsIconic(hwnd).as_bool() { let _ = ShowWindow(hwnd, SW_RESTORE); }
        if SetForegroundWindow(hwnd).as_bool() { return true; }
        // Windows lets a process take the foreground once it has the last input: an Alt tap gives it.
        keybd_event(VK_MENU.0 as u8, 0, Default::default(), 0);
        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        SetForegroundWindow(hwnd).as_bool()
    }
}

/// The window of the nearest parent that has one (the terminal before an editor that started it); of
/// several, the one naming the project. With no parent's window, the topmost naming the project.
fn find(pids: &[u32], project: &str) -> Option<HWND> {
    let mut windows: Vec<(HWND, u32, String)> = Vec::new();
    unsafe { let _ = EnumWindows(Some(collect), LPARAM(&mut windows as *mut _ as isize)); }
    let names = |title: &str| !project.is_empty() && title.to_lowercase().contains(&project.to_lowercase());
    for pid in pids {
        let own: Vec<_> = windows.iter().filter(|w| w.1 == *pid).collect();
        if let Some(first) = own.first() {
            return Some(own.iter().find(|w| names(&w.2)).unwrap_or(first).0);
        }
    }
    windows.iter().find(|w| names(&w.2)).map(|w| w.0)
}

/// Visible, unowned windows with a title, topmost first (EnumWindows' order).
unsafe extern "system" fn collect(hwnd: HWND, list: LPARAM) -> BOOL {
    unsafe {
        let list = &mut *(list.0 as *mut Vec<(HWND, u32, String)>);
        if IsWindowVisible(hwnd).as_bool() && GetWindow(hwnd, GW_OWNER).is_err() {
            let mut buf = [0u16; 256];
            let len = GetWindowTextW(hwnd, &mut buf) as usize;
            if len > 0 {
                let mut pid = 0;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                list.push((hwnd, pid, String::from_utf16_lossy(&buf[..len])));
            }
        }
    }
    true.into()
}
