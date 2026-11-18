// SPDX-License-Identifier: GPL-3.0-or-later
// Port of the reminder/timer sound from Mac Island & Dock 3.1.1 (Island::ShowReminder, CheckTimer:
// MessageBeep(MB_ICONASTERISK)). ponytail: the mod's looping "Notification.Looping.Alarm" until
// Stop is not ported; one beep per alert.

/// Plays Windows' "asterisk" sound (asynchronous; silent if the user turned system sounds off).
pub fn alert_beep() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Diagnostics::Debug::MessageBeep;
        use windows::Win32::UI::WindowsAndMessaging::MB_ICONASTERISK;
        let _ = MessageBeep(MB_ICONASTERISK);
    }
}
