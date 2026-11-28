// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Port of the power button's actions from Mac Island & Dock 3.1.1 (RunPowerAction,
// EnableShutdownPrivilege, IsFastStartupOn, IsHibernateAllowed, HasClassicSleep). `run` is the only
// place that touches Windows: tests check the flags it would use, never call it.

use isle_core::i18n::{tr};
use anyhow::Result;
use isle_core::power::PowerAction;

const EWX_LOGOFF: u32 = 0x0;
const EWX_SHUTDOWN: u32 = 0x1;
const EWX_REBOOT: u32 = 0x2;
const EWX_POWEROFF: u32 = 0x8;
const EWX_HYBRID_SHUTDOWN: u32 = 0x0040_0000;

/// ExitWindowsEx's flags for an action (None: not one of its), and whether it needs the shutdown
/// privilege (signing out does not). With fast startup on, shutting down saves the session as the
/// Start menu's does.
pub fn exit_flags(action: PowerAction, fast_startup: bool) -> Option<(u32, bool)> {
    match action {
        PowerAction::Restart => Some((EWX_REBOOT, true)),
        PowerAction::SignOut => Some((EWX_LOGOFF, false)),
        PowerAction::ShutDown => Some((EWX_SHUTDOWN | EWX_POWEROFF | if fast_startup { EWX_HYBRID_SHUTDOWN } else { 0 }, true)),
        _ => None,
    }
}

/// Whether Windows can hibernate now (the menu offers Hibernate only then).
pub fn hibernate_allowed() -> bool {
    #[cfg(windows)]
    unsafe {
        windows::Win32::System::Power::IsPwrHibernateAllowed().as_bool()
    }
    #[cfg(not(windows))]
    false
}

/// Runs the action. Sleeping returns once the PC wakes up: call it off the UI thread.
pub fn run(action: PowerAction) -> Result<()> {
    #[cfg(windows)]
    {
        win::run(action)
    }
    #[cfg(not(windows))]
    {
        let _ = action;
        anyhow::bail!("Disponível no Windows")
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use windows::Win32::Foundation::{BOOLEAN, CloseHandle, HANDLE, LPARAM, WPARAM};
    use windows::Win32::Security::{
        AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED, SE_SHUTDOWN_NAME,
        TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows::Win32::System::Power::{GetPwrCapabilities, SetSuspendState, SYSTEM_POWER_CAPABILITIES};
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD};
    use windows::Win32::System::Shutdown::{
        ExitWindowsEx, LockWorkStation, EXIT_WINDOWS_FLAGS, SHTDN_REASON_FLAG_PLANNED, SHTDN_REASON_MAJOR_OTHER,
        SHTDN_REASON_MINOR_OTHER,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, HWND_BROADCAST, SC_MONITORPOWER, WM_SYSCOMMAND};
    use windows::core::w;

    /// Port of EnableShutdownPrivilege: a privilege every user has, off until a program asks for it.
    fn enable_shutdown_privilege() -> bool {
        unsafe {
            let mut token = HANDLE::default();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token).is_err() {
                return false;
            }
            let mut privileges = TOKEN_PRIVILEGES { PrivilegeCount: 1, Privileges: [LUID_AND_ATTRIBUTES::default()] };
            let mut ok = LookupPrivilegeValueW(None, SE_SHUTDOWN_NAME, &mut privileges.Privileges[0].Luid).is_ok();
            if ok {
                privileges.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;
                ok = AdjustTokenPrivileges(token, false, Some(&privileges), 0, None, None).is_ok();
            }
            let _ = CloseHandle(token);
            ok
        }
    }

    /// Port of IsFastStartupOn (read only).
    fn fast_startup_on() -> bool {
        let mut value = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        unsafe {
            RegGetValueW(HKEY_LOCAL_MACHINE, w!(r"SYSTEM\CurrentControlSet\Control\Session Manager\Power"),
                w!("HiberbootEnabled"), RRF_RT_REG_DWORD, None, Some((&mut value as *mut u32).cast()), Some(&mut size))
                .is_ok() && value != 0
        }
    }

    /// Port of HasClassicSleep: Modern Standby PCs have no S1–S3 (true when it can't be read).
    fn has_classic_sleep() -> bool {
        let mut caps = SYSTEM_POWER_CAPABILITIES::default();
        unsafe {
            if !GetPwrCapabilities(&mut caps).as_bool() {
                return true;
            }
        }
        caps.SystemS1.as_bool() || caps.SystemS2.as_bool() || caps.SystemS3.as_bool()
    }

    /// Port of RunPowerAction.
    pub fn run(action: PowerAction) -> Result<()> {
        match action {
            PowerAction::Lock => unsafe { LockWorkStation()? },
            PowerAction::Sleep | PowerAction::Hibernate => {
                let hibernate = action == PowerAction::Hibernate;
                enable_shutdown_privilege();
                let slept = (hibernate || has_classic_sleep())
                    && unsafe { SetSuspendState(BOOLEAN(hibernate as u8), BOOLEAN(0), BOOLEAN(0)).as_bool() };
                if !slept {
                    anyhow::ensure!(!hibernate, "{}", tr("errors.hibernate"));
                    // Modern Standby sleeps by turning the screen off, as Start → Suspender does.
                    unsafe { PostMessageW(HWND_BROADCAST, WM_SYSCOMMAND, WPARAM(SC_MONITORPOWER as usize), LPARAM(2))? };
                }
            }
            _ => {
                let (flags, privilege) = exit_flags(action, fast_startup_on()).expect("restart, shut down or sign out");
                if privilege {
                    enable_shutdown_privilege();
                }
                let reason = if privilege { SHTDN_REASON_MAJOR_OTHER | SHTDN_REASON_MINOR_OTHER | SHTDN_REASON_FLAG_PLANNED } else { Default::default() };
                unsafe { ExitWindowsEx(EXIT_WINDOWS_FLAGS(flags), reason)? };
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_action_asks_windows_for_what_the_mod_did() {
        assert_eq!(exit_flags(PowerAction::Restart, false), Some((EWX_REBOOT, true)));
        assert_eq!(exit_flags(PowerAction::SignOut, true), Some((EWX_LOGOFF, false)));
        assert_eq!(exit_flags(PowerAction::ShutDown, false), Some((EWX_SHUTDOWN | EWX_POWEROFF, true)));
        assert_eq!(exit_flags(PowerAction::ShutDown, true).unwrap().0 & EWX_HYBRID_SHUTDOWN, EWX_HYBRID_SHUTDOWN);
        assert_eq!(exit_flags(PowerAction::Lock, false), None);
        assert_eq!(exit_flags(PowerAction::Sleep, false), None);
    }
}
