pub mod audio_osd;
pub mod locale;
pub mod sound;
pub mod lyrics;
pub mod media;
pub mod notifications;
pub mod notification_icons;
pub mod system_controls;
pub mod power;
pub mod tray;
pub mod weather;

pub mod artwork;
pub mod files;
pub mod icon_packs;

#[cfg(windows)]
pub mod clipboard;
#[cfg(windows)]
pub mod agent_window;
pub mod downloads;
pub mod shelf_share;

/// The system's caret blink period in ms (530 when unknown); the system's "no blink" setting becomes a steady caret.
pub fn caret_blink_ms() -> u64 {
    #[cfg(windows)]
    let ms = unsafe { windows::Win32::UI::WindowsAndMessaging::GetCaretBlinkTime() };
    #[cfg(not(windows))]
    let ms = 0;
    match ms { 0 => 530, u32::MAX => u64::MAX, ms => u64::from(ms) }
}
