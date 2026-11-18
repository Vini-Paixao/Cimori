//! The shortcuts recorded with any keys (Mac Island & Dock's islandMinimizeShortcut and its other
//! slots, registered by RegisterIslandShortcuts). The old list's keys stay with phase8's binding
//! (Win+J and Win+N through the keyboard hook); any other combination is registered here, on the
//! main island's window, as hotkeys 3 (minimize) and 4 (notices).
use super::*;

/// Hotkey ids for the recorded combinations, and the island action each one is (1 minimize, 2 notices).
pub(super) const RECORDED: [(i32, i32); 2] = [(3, 1), (4, 2)];

/// What a hotkey id does: the recorded ones do what the listed ones do.
pub(super) fn island_hotkey(id: i32) -> i32 {
    RECORDED.iter().find(|(own, _)| *own == id).map_or(id, |(_, action)| *action)
}

impl IsleApp {
    /// The keys to register here: (id, keys), only those the old list doesn't cover.
    pub(super) fn recorded_shortcuts(&self) -> Vec<(i32, u32)> {
        let ui = self.config.ui;
        let notices = if self.config.win_n && self.config.modules.notifications { ui.notices_keys } else { 0 };
        [(RECORDED[0].0, ui.minimize_keys), (RECORDED[1].0, notices)]
            .into_iter()
            .filter(|(_, keys)| *keys != 0 && isle_core::shortcut::to_hotkey(*keys) == isle_core::config::Hotkey::None)
            .collect()
    }

    #[cfg(windows)]
    pub(super) fn bind_shortcuts(&mut self) {
        use windows::Win32::UI::Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey};
        let Some(hwnd) = self.overlay.native_window() else { return };
        for (id, _) in RECORDED {
            unsafe { let _ = UnregisterHotKey(hwnd, id); }
        }
        for (id, keys) in self.recorded_shortcuts() {
            let (mods, key) = isle_core::shortcut::register_args(keys);
            if let Err(err) = unsafe { RegisterHotKey(hwnd, id, HOT_KEY_MODIFIERS(mods) | MOD_NOREPEAT, key) } {
                // Taken by Windows or another app (the mod says so under the row).
                tracing::warn!("atalho {}: {err}", isle_core::shortcut::text(keys));
            }
        }
    }

    #[cfg(not(windows))]
    pub(super) fn bind_shortcuts(&mut self) {}
}
