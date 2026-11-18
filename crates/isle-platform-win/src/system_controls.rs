// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Adapted for isle-win from Mac Island & Dock by caliberda v3.1.0 (Island::QueryNetwork,
// RadioThreadProc, ParseNightLight, SetNightLight, RunControlShortcut/PressKeys): Rust workers,
// checked CloudStore records and settings fallbacks for unavailable controls.

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsbDriveInfo {
    pub letter: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemSnapshot {
    pub ram_usage_pct: u8,
    pub usb_drives: Vec<UsbDriveInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickAction {
    TaskManager,
    Explorer,
    NetworkSettings,
    BluetoothSettings,
    NightLightSettings,
    NotificationSettings,
    Captions,
    Accessibility,
    SoundSettings,
    ProjectDisplay,
    CastDisplay,
    PinnedFolder(&'static str),
    /// Port of RunControlShortcut's ControlId::WindowsSettings (mac-island 3.1.1).
    WindowsSettings,
    /// The control center's catalog (mac-island 3.1.1's ControlId): Windows' own page or app where the
    /// mod has a toggle of its own (hotspot, airplane mode, brightness, battery, power mode).
    Hotspot,
    Airplane,
    Display,
    Battery,
    PowerMode,
    Calculator,
    RecycleBin,
}

impl QuickAction {
    pub fn windows_command(&self) -> (&'static str, Vec<&'static str>) {
        match self {
            QuickAction::TaskManager => ("taskmgr.exe", vec![]),
            QuickAction::Explorer => ("explorer.exe", vec![]),
            QuickAction::NetworkSettings => ("explorer.exe", vec!["ms-settings:network-status"]),
            QuickAction::BluetoothSettings => ("explorer.exe", vec!["ms-settings:bluetooth"]),
            QuickAction::NightLightSettings => ("explorer.exe", vec!["ms-settings:nightlight"]),
            QuickAction::NotificationSettings => ("explorer.exe", vec!["ms-settings:notifications"]),
            QuickAction::Captions => ("explorer.exe", vec!["ms-settings:easeofaccess-closedcaptioning"]),
            QuickAction::Accessibility => ("explorer.exe", vec!["ms-settings:easeofaccess"]),
            QuickAction::SoundSettings => ("explorer.exe", vec!["ms-settings:apps-volume"]),
            QuickAction::ProjectDisplay => ("DisplaySwitch.exe", vec![]),
            QuickAction::CastDisplay => ("explorer.exe", vec!["ms-settings:connecteddevices"]),
            QuickAction::WindowsSettings => ("explorer.exe", vec!["ms-settings:"]),
            QuickAction::Hotspot => ("explorer.exe", vec!["ms-settings:network-mobilehotspot"]),
            QuickAction::Airplane => ("explorer.exe", vec!["ms-settings:network-airplanemode"]),
            QuickAction::Display => ("explorer.exe", vec!["ms-settings:display"]),
            QuickAction::Battery => ("explorer.exe", vec!["ms-settings:batterysaver"]),
            QuickAction::PowerMode => ("explorer.exe", vec!["ms-settings:powersleep"]),
            QuickAction::Calculator => ("calc.exe", vec![]),
            QuickAction::RecycleBin => ("explorer.exe", vec!["shell:RecycleBinFolder"]),
            QuickAction::PinnedFolder(place) => match *place {
                "desktop" => ("explorer.exe", vec!["shell:Desktop"]),
                "documents" => ("explorer.exe", vec!["shell:Personal"]),
                "downloads" => ("explorer.exe", vec!["shell:Downloads"]),
                "thispc" => ("explorer.exe", vec!["shell:MyComputerFolder"]),
                "pictures" => ("explorer.exe", vec!["shell:My Pictures"]),
                "screenshots" => ("explorer.exe", vec!["shell:Screenshots"]),
                _ => ("explorer.exe", vec![]),
            },
        }
    }
}

pub fn execute_quick_action(action: QuickAction) -> Result<()> {
    let (program, args) = action.windows_command();
    #[cfg(windows)]
    {
        std::process::Command::new(program).args(args).spawn()?;
    }
    #[cfg(not(windows))]
    {
        let _ = (program, args);
        anyhow::bail!("Atalho disponível no Windows");
    }
    #[cfg(windows)]
    Ok(())
}

#[cfg(windows)]
pub fn poll_system_snapshot() -> SystemSnapshot {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    let ram_usage_pct = unsafe {
        let mut mem = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        if GlobalMemoryStatusEx(&mut mem).is_ok() {
            mem.dwMemoryLoad.min(100) as u8
        } else {
            0
        }
    };

    SystemSnapshot {
        ram_usage_pct,
        usb_drives: list_usb_drives(),
    }
}

#[cfg(not(windows))]
pub fn poll_system_snapshot() -> SystemSnapshot {
    SystemSnapshot {
        ram_usage_pct: 42,
        usb_drives: Vec::new(),
    }
}

fn usb_drive_letter(drive: &str) -> Result<char> {
    match drive.as_bytes() {
        [letter] | [letter, b':'] | [letter, b':', b'\\'] if letter.is_ascii_alphabetic() => {
            Ok(letter.to_ascii_uppercase() as char)
        }
        _ => anyhow::bail!("Letra de unidade USB inválida"),
    }
}

pub fn list_usb_drives() -> Vec<UsbDriveInfo> {
    #[cfg(windows)]
    return usb::list();
    #[cfg(not(windows))]
    Vec::new()
}

pub fn open_usb_drive(drive: &str) -> Result<()> {
    let letter = usb_drive_letter(drive)?;
    #[cfg(windows)]
    return usb::open(letter);
    #[cfg(not(windows))]
    {
        let _ = letter;
        anyhow::bail!("A abertura de unidades USB requer o Windows");
    }
}

pub fn eject_usb_drive(drive: &str) -> Result<()> {
    let letter = usb_drive_letter(drive)?;
    #[cfg(windows)]
    return usb::eject(letter);
    #[cfg(not(windows))]
    {
        let _ = letter;
        anyhow::bail!("A ejeção de unidades USB requer o Windows");
    }
}

// Adapted from Mac Island & Dock by caliberda (IsUsbDrive, DrivesThreadProc,
// DiskNumber and EjectThreadProc). Volume letters are mapped to the disk's
// PnP node and safe removal is requested through Configuration Manager.
#[cfg(windows)]
mod usb {
    use super::{Result, UsbDriveInfo};
    use anyhow::{anyhow, bail, ensure};
    use std::ffi::c_void;
    use std::mem::{offset_of, size_of, size_of_val};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows::core::{w, PCWSTR};
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        CM_Get_DevNode_Registry_PropertyW, CM_Get_Parent, CM_Request_Device_EjectW,
        PNP_VetoTypeUnknown, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces,
        SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW, CM_DEVCAP_REMOVABLE,
        CM_DRP_CAPABILITIES, CM_DRP_COMPATIBLEIDS, CM_DRP_ENUMERATOR_NAME, CR_SUCCESS,
        DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO, SP_DEVICE_INTERFACE_DATA,
        SP_DEVICE_INTERFACE_DETAIL_DATA_W, SP_DEVINFO_DATA,
    };
    use windows::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, HANDLE, HWND};
    use windows::Win32::Storage::FileSystem::{
        BusTypeUsb, CreateFileW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
        FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::Diagnostics::Debug::{
        GetThreadErrorMode, SetThreadErrorMode, SEM_FAILCRITICALERRORS, THREAD_ERROR_MODE,
    };
    use windows::Win32::System::Ioctl::{
        PropertyStandardQuery, StorageDeviceProperty, FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME,
        GUID_DEVINTERFACE_DISK, IOCTL_STORAGE_EJECT_MEDIA, IOCTL_STORAGE_GET_DEVICE_NUMBER,
        IOCTL_STORAGE_MEDIA_REMOVAL, IOCTL_STORAGE_QUERY_PROPERTY, PREVENT_MEDIA_REMOVAL,
        STORAGE_DESCRIPTOR_HEADER, STORAGE_DEVICE_DESCRIPTOR, STORAGE_DEVICE_NUMBER,
        STORAGE_PROPERTY_QUERY,
    };
    use windows::Win32::System::WindowsProgramming::{DRIVE_FIXED, DRIVE_REMOVABLE};
    use windows::Win32::System::IO::DeviceIoControl;
    use windows::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

    struct CriticalErrors(Option<THREAD_ERROR_MODE>);

    impl CriticalErrors {
        fn suppress() -> Self {
            unsafe {
                let mode = THREAD_ERROR_MODE(GetThreadErrorMode()) | SEM_FAILCRITICALERRORS;
                let mut previous = THREAD_ERROR_MODE::default();
                Self(
                    SetThreadErrorMode(mode, Some(&mut previous))
                        .ok()
                        .map(|_| previous),
                )
            }
        }
    }

    impl Drop for CriticalErrors {
        fn drop(&mut self) {
            if let Some(previous) = self.0 {
                unsafe {
                    let _ = SetThreadErrorMode(previous, None);
                }
            }
        }
    }

    fn root_path(letter: char) -> [u16; 4] {
        [letter as u16, b':' as u16, b'\\' as u16, 0]
    }

    fn volume_path(letter: char) -> [u16; 7] {
        [
            b'\\' as u16,
            b'\\' as u16,
            b'.' as u16,
            b'\\' as u16,
            letter as u16,
            b':' as u16,
            0,
        ]
    }

    fn device_handle(path: PCWSTR) -> windows::core::Result<OwnedHandle> {
        unsafe {
            let handle = CreateFileW(
                path,
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                HANDLE::default(),
            )?;
            Ok(OwnedHandle::from_raw_handle(handle.0))
        }
    }

    fn is_usb_bus(letter: char) -> bool {
        let path = volume_path(letter);
        let Ok(device) = device_handle(PCWSTR(path.as_ptr())) else {
            return false;
        };
        let query = STORAGE_PROPERTY_QUERY {
            PropertyId: StorageDeviceProperty,
            QueryType: PropertyStandardQuery,
            ..Default::default()
        };
        let handle = HANDLE(device.as_raw_handle());
        let input = Some((&query as *const STORAGE_PROPERTY_QUERY).cast::<c_void>());
        let input_size = size_of::<STORAGE_PROPERTY_QUERY>() as u32;
        let mut header = STORAGE_DESCRIPTOR_HEADER::default();
        let mut returned = 0;
        let result = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_QUERY_PROPERTY,
                input,
                input_size,
                Some((&mut header as *mut STORAGE_DESCRIPTOR_HEADER).cast()),
                size_of::<STORAGE_DESCRIPTOR_HEADER>() as u32,
                Some(&mut returned),
                None,
            )
        };
        if result.is_err()
            || returned < size_of::<STORAGE_DESCRIPTOR_HEADER>() as u32
            || !(size_of::<STORAGE_DEVICE_DESCRIPTOR>() as u32..=1024 * 1024).contains(&header.Size)
        {
            return false;
        }
        // Device descriptors include variable-length strings after the fixed header.
        let mut buffer = vec![0u32; (header.Size as usize).div_ceil(size_of::<u32>())];
        unsafe {
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_QUERY_PROPERTY,
                input,
                input_size,
                Some(buffer.as_mut_ptr().cast()),
                header.Size,
                Some(&mut returned),
                None,
            )
            .is_ok()
                && returned as usize >= size_of::<STORAGE_DEVICE_DESCRIPTOR>()
                && (*buffer.as_ptr().cast::<STORAGE_DEVICE_DESCRIPTOR>()).BusType == BusTypeUsb
        }
    }

    fn drive_label(letter: char) -> Option<String> {
        let root = root_path(letter);
        let root = PCWSTR(root.as_ptr());
        let kind = unsafe { GetDriveTypeW(root) };
        if kind != DRIVE_REMOVABLE && (kind != DRIVE_FIXED || !is_usb_bus(letter)) {
            return None;
        }
        let mut label = [0; 261];
        unsafe { GetVolumeInformationW(root, Some(&mut label), None, None, None, None) }.ok()?;
        let end = label
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(label.len());
        Some(if end == 0 {
            "Unidade USB".into()
        } else {
            String::from_utf16_lossy(&label[..end])
        })
    }

    pub(super) fn list() -> Vec<UsbDriveInfo> {
        let _errors = CriticalErrors::suppress();
        let present = unsafe { GetLogicalDrives() };
        (0..26)
            .filter(|&index| present & (1 << index) != 0)
            .filter_map(|index| {
                let letter = (b'A' + index) as char;
                Some(UsbDriveInfo {
                    letter: format!("{letter}:"),
                    label: drive_label(letter)?,
                })
            })
            .collect()
    }

    pub(super) fn open(letter: char) -> Result<()> {
        let _errors = CriticalErrors::suppress();
        ensure!(
            drive_label(letter).is_some(),
            "A unidade USB {letter}: não está disponível"
        );
        let path = root_path(letter);
        let result = unsafe {
            ShellExecuteW(
                HWND::default(),
                w!("open"),
                PCWSTR(path.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        ensure!(
            result.0 as isize > 32,
            "Não foi possível abrir a unidade USB {letter}:"
        );
        Ok(())
    }

    fn disk_number(path: PCWSTR) -> Option<(u32, u32)> {
        let device = device_handle(path).ok()?;
        let mut number = STORAGE_DEVICE_NUMBER::default();
        let mut returned = 0;
        unsafe {
            DeviceIoControl(
                HANDLE(device.as_raw_handle()),
                IOCTL_STORAGE_GET_DEVICE_NUMBER,
                None,
                0,
                Some((&mut number as *mut STORAGE_DEVICE_NUMBER).cast()),
                size_of::<STORAGE_DEVICE_NUMBER>() as u32,
                Some(&mut returned),
                None,
            )
            .ok()?;
        }
        (returned as usize >= size_of::<STORAGE_DEVICE_NUMBER>())
            .then_some((number.DeviceType, number.DeviceNumber))
    }

    struct DeviceSet(HDEVINFO);

    impl Drop for DeviceSet {
        fn drop(&mut self) {
            unsafe {
                let _ = SetupDiDestroyDeviceInfoList(self.0);
            }
        }
    }

    fn find_disk(wanted: (u32, u32)) -> Option<u32> {
        let devices = DeviceSet(unsafe {
            SetupDiGetClassDevsW(
                Some(&GUID_DEVINTERFACE_DISK),
                PCWSTR::null(),
                HWND::default(),
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            )
            .ok()?
        });
        for index in 0.. {
            let mut interface = SP_DEVICE_INTERFACE_DATA {
                cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                ..Default::default()
            };
            unsafe {
                SetupDiEnumDeviceInterfaces(
                    devices.0,
                    None,
                    &GUID_DEVINTERFACE_DISK,
                    index,
                    &mut interface,
                )
                .ok()?;
            }
            let mut bytes = 0;
            unsafe {
                let _ = SetupDiGetDeviceInterfaceDetailW(
                    devices.0,
                    &interface,
                    None,
                    0,
                    Some(&mut bytes),
                    None,
                );
            }
            let detail_size = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
            if (bytes as usize) < detail_size {
                continue;
            }
            let mut buffer = vec![0u32; (bytes as usize).div_ceil(size_of::<u32>())];
            let detail = buffer
                .as_mut_ptr()
                .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
            let mut info = SP_DEVINFO_DATA {
                cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
                ..Default::default()
            };
            unsafe {
                (*detail).cbSize = detail_size as u32;
                if SetupDiGetDeviceInterfaceDetailW(
                    devices.0,
                    &interface,
                    Some(detail),
                    bytes,
                    None,
                    Some(&mut info),
                )
                .is_ok()
                {
                    let path = buffer
                        .as_ptr()
                        .cast::<u8>()
                        .add(offset_of!(SP_DEVICE_INTERFACE_DETAIL_DATA_W, DevicePath))
                        .cast::<u16>();
                    if disk_number(PCWSTR(path)) == Some(wanted) {
                        return Some(info.DevInst);
                    }
                }
            }
        }
        None
    }

    fn node_property(node: u32, property: u32, buffer: *mut c_void, size: u32) -> bool {
        let mut size = size;
        unsafe {
            CM_Get_DevNode_Registry_PropertyW(node, property, None, Some(buffer), &mut size, 0)
                == CR_SUCCESS
        }
    }

    fn node_ids(node: u32, property: u32) -> Option<Vec<String>> {
        let mut buffer = [0u16; 1024];
        node_property(
            node,
            property,
            buffer.as_mut_ptr().cast(),
            size_of_val(&buffer) as u32,
        )
        .then(|| {
            buffer
                .split(|&value| value == 0)
                .take_while(|id| !id.is_empty())
                .map(|id| String::from_utf16_lossy(id).to_ascii_uppercase())
                .collect()
        })
    }

    fn removable(node: u32) -> bool {
        let mut caps = 0u32;
        node_property(
            node,
            CM_DRP_CAPABILITIES,
            (&mut caps as *mut u32).cast(),
            size_of::<u32>() as u32,
        ) && caps & CM_DEVCAP_REMOVABLE.0 != 0
    }

    fn parent(node: u32) -> Option<u32> {
        let mut parent = 0;
        (unsafe { CM_Get_Parent(&mut parent, node, 0) } == CR_SUCCESS && parent != 0)
            .then_some(parent)
    }

    fn eject_target(disk: u32, usb_bus: bool, fixed: bool) -> Option<u32> {
        if removable(disk) {
            return Some(disk);
        }
        if !usb_bus {
            return None;
        }
        let direct_parent = parent(disk);
        let mut node = direct_parent;
        let mut physical_usb_device = None;
        while let Some(ancestor) = node {
            let enumerator = node_ids(ancestor, CM_DRP_ENUMERATOR_NAME)?;
            if !enumerator
                .iter()
                .any(|name| name == "USB" || name == "USBSTOR" || name == "SCSI")
            {
                break;
            }
            if enumerator.iter().any(|name| name == "USB") {
                let compatible_ids = node_ids(ancestor, CM_DRP_COMPATIBLEIDS)?;
                if compatible_ids
                    .iter()
                    .any(|id| id.starts_with("USB\\CLASS_09"))
                {
                    break;
                }
                // A fixed-media USB disk can have no removable flag on its
                // disk node. Only the directly attached USB function qualifies.
                if fixed && Some(ancestor) == direct_parent {
                    physical_usb_device = Some(ancestor);
                }
            }
            if removable(ancestor) {
                return Some(ancestor);
            }
            node = parent(ancestor);
        }
        physical_usb_device
    }

    // If a removable card's reader stays attached, eject only the media.
    // FSCTL_LOCK_VOLUME fails while any files are open and flushes cached writes.
    fn eject_media(path: PCWSTR) -> windows::core::Result<()> {
        unsafe {
            let handle = CreateFileW(
                path,
                (GENERIC_READ | GENERIC_WRITE).0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                HANDLE::default(),
            )?;
            let volume = OwnedHandle::from_raw_handle(handle.0);
            let handle = HANDLE(volume.as_raw_handle());
            let mut returned = 0;
            for control in [FSCTL_LOCK_VOLUME, FSCTL_DISMOUNT_VOLUME] {
                DeviceIoControl(handle, control, None, 0, None, 0, Some(&mut returned), None)?;
            }
            let allow = PREVENT_MEDIA_REMOVAL::default();
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_MEDIA_REMOVAL,
                Some((&allow as *const PREVENT_MEDIA_REMOVAL).cast()),
                size_of::<PREVENT_MEDIA_REMOVAL>() as u32,
                None,
                0,
                Some(&mut returned),
                None,
            )?;
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_EJECT_MEDIA,
                None,
                0,
                None,
                0,
                Some(&mut returned),
                None,
            )
        }
    }

    pub(super) fn eject(letter: char) -> Result<()> {
        let _errors = CriticalErrors::suppress();
        ensure!(
            drive_label(letter).is_some(),
            "A unidade USB {letter}: não está disponível"
        );
        let volume = volume_path(letter);
        let volume = PCWSTR(volume.as_ptr());
        let wanted = disk_number(volume)
            .ok_or_else(|| anyhow!("Não foi possível identificar o disco de {letter}:"))?;
        let disk = find_disk(wanted)
            .ok_or_else(|| anyhow!("O dispositivo da unidade {letter}: não foi localizado"))?;
        let root = root_path(letter);
        let fixed = unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) } == DRIVE_FIXED;
        let target = eject_target(disk, is_usb_bus(letter), fixed);
        ensure!(
            disk_number(volume) == Some(wanted),
            "O dispositivo da unidade {letter}: mudou durante a ejeção"
        );
        let Some(device) = target else {
            if fixed {
                bail!("O Windows não disponibilizou a remoção segura para a unidade {letter}:");
            }
            return eject_media(volume).map_err(|err| {
                anyhow!("O Windows recusou a ejeção da mídia na unidade {letter}: ({err})")
            });
        };
        let mut veto = PNP_VetoTypeUnknown;
        let mut veto_name = [0; 261];
        let result =
            unsafe { CM_Request_Device_EjectW(device, Some(&mut veto), Some(&mut veto_name), 0) };
        ensure!(
            result == CR_SUCCESS && veto == PNP_VetoTypeUnknown,
            "O Windows recusou a ejeção da unidade {letter}: (código {}, motivo {})",
            result.0,
            veto.0
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_quick_actions_and_pinned_folders_to_windows_commands() {
        assert_eq!(QuickAction::TaskManager.windows_command(), ("taskmgr.exe", vec![]));
        assert_eq!(
            QuickAction::NetworkSettings.windows_command(),
            ("explorer.exe", vec!["ms-settings:network-status"])
        );
        assert_eq!(
            QuickAction::BluetoothSettings.windows_command(),
            ("explorer.exe", vec!["ms-settings:bluetooth"])
        );
        assert_eq!(
            QuickAction::PinnedFolder("downloads").windows_command(),
            ("explorer.exe", vec!["shell:Downloads"])
        );
    }
}

#[cfg(any(windows, test))]
fn night_light_state(data: &[u8]) -> Option<bool> { Some(parse_night_light(data)?.2) }

#[cfg(any(windows, test))]
fn parse_night_light(d: &[u8]) -> Option<(usize, usize, bool)> {
    if d.len() < 30 || d[..10] != [0x43,0x42,1,0,0x0a,2,1,0,0x2a,6] { return None; }
    let mut end = 10;
    while end < d.len() && d[end] & 0x80 != 0 && end < 19 { end += 1; }
    if end >= d.len() || d[end] & 0x80 != 0 || (end == 19 && d[end] > 1) { return None; }
    end += 1;
    if d.get(end..end+3)? != [0x2a,0x2b,0x0e] { return None; }
    let len_at = end + 3;
    let state = len_at + 1;
    let len = *d.get(len_at)? as usize;
    if len < 6 || state + len > d.len() || d.get(state..state+4)? != [0x43,0x42,1,0] { return None; }
    Some((end, len_at, d[state+4..state+6] == [0x10,0]))
}
#[cfg(any(windows, test))]
fn change_night_light(data: &[u8], on: bool, now: u64) -> Option<Vec<u8>> {
    let (end, len_at, before_on) = parse_night_light(data)?;
    let before = data[10..end].iter().enumerate().fold(0u64, |v,(i,b)| v | ((b & 0x7f) as u64) << (7*i));
    let mut stamp = now.max(before.checked_add(1)?);
    let mut encoded = Vec::new();
    loop {
        let byte = (stamp & 0x7f) as u8; stamp >>= 7;
        encoded.push(byte | if stamp > 0 { 0x80 } else { 0 });
        if stamp == 0 { break; }
    }
    let mut d = data.to_vec();
    let at = len_at + 5;
    if on && !before_on {
        d[len_at] = d[len_at].checked_add(2)?;
        d.splice(at..at, [0x10,0]);
    } else if !on && before_on {
        d[len_at] = d[len_at].checked_sub(2)?;
        d.drain(at..at+2);
    }
    d.splice(10..end, encoded);
    if night_light_state(&d) != Some(on) { return None; }
    Some(d)
}

#[cfg(test)]
mod night_tests {
    use super::*;
    fn off() -> Vec<u8> {
        // Unix timestamp 1, state length 15, with opaque tail preserved.
        vec![0x43,0x42,1,0,0x0a,2,1,0,0x2a,6,1,0x2a,0x2b,0x0e,15,
             0x43,0x42,1,0,0x20,0x30,0x40,0x50,0x60,0,0,0,0,0,0]
    }
    #[test]
    fn night_light_roundtrip_preserves_tail_and_advances_timestamp() {
        let original = off();
        assert_eq!(night_light_state(&original), Some(false));
        let on = change_night_light(&original, true, 127).unwrap();
        assert_eq!(night_light_state(&on), Some(true));
        assert_eq!(&on[19..21], &[0x10, 0]);
        let back = change_night_light(&on, false, 127).unwrap();
        assert_eq!(&back[10..12], &[0x80, 1]); // 128: newer even in the same second
        assert_eq!(night_light_state(&back), Some(false));
        assert_eq!(&back[16..], &original[15..]);
    }
    #[test]
    fn malformed_cloudstore_data_is_never_written() {
        let good = off();
        for end in 0..good.len() {
            assert!(change_night_light(&good[..end], true, 5).is_none());
        }
        let mut bad = good.clone(); bad[0] = 0;
        assert!(change_night_light(&bad, true, 5).is_none());
        let mut bad = good; bad[14] = 255;
        assert!(change_night_light(&bad, true, 5).is_none());
    }
}

use isle_core::state::ControlTile;
use std::sync::mpsc::{self, Sender};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlStatus {
    pub wifi_name: Option<String>,
    pub wifi_connected: Option<bool>,
    pub bluetooth_on: Option<bool>,
    pub night_light_on: Option<bool>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCommand { Panel(bool), Activate(ControlTile), OpenMixer }
#[derive(Debug)]
pub enum ControlEvent { Status(ControlStatus), Error(String) }

pub fn spawn(on_event: impl Fn(ControlEvent) + Send + 'static) -> Sender<ControlCommand> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        #[cfg(windows)] {
            if let Err(err) = win::run(rx, &on_event) { on_event(ControlEvent::Error(format!("Controles: {err:#}"))); }
        }
        #[cfg(not(windows))] {
            on_event(ControlEvent::Status(ControlStatus::default()));
            for command in rx {
                if matches!(command, ControlCommand::Activate(_) | ControlCommand::OpenMixer) {
                    on_event(ControlEvent::Error("Controles disponíveis no Windows".into()));
                }
            }
        }
    });
    tx
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::{ptr, time::{Duration, SystemTime, UNIX_EPOCH}, sync::mpsc::{Receiver, RecvTimeoutError}};
    use windows::{core::{PCWSTR, w}, Devices::Radios::*, Win32::{
        Foundation::HANDLE, NetworkManagement::WiFi::*, System::{Com::*, Registry::*},
    }};

    struct Key(HKEY);
    impl Drop for Key { fn drop(&mut self) { unsafe { let _ = RegCloseKey(self.0); } } }
    const STORE: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\CloudStore\\Store\\DefaultAccount\\Current");
    const STATE: &str = "windows.data.bluelightreduction.bluelightreductionstate";
    fn records() -> Vec<(String, Vec<u8>)> {
        unsafe {
            let mut key = HKEY::default();
            if RegOpenKeyExW(HKEY_CURRENT_USER, STORE, 0, KEY_READ, &mut key).is_err() { return Vec::new(); }
            let key = Key(key);
            let mut records = Vec::new();
            for index in 0.. {
                let mut name = [0u16; 256]; let mut len = name.len() as u32;
                if RegEnumKeyExW(key.0, index, windows::core::PWSTR(name.as_mut_ptr()), &mut len, None, windows::core::PWSTR::null(), None, None).is_err() { break; }
                let name = String::from_utf16_lossy(&name[..len as usize]);
                let Some((_, value)) = name.split_once('$') else { continue };
                if value != STATE && value != format!("{STATE}perdevice") { continue; }
                let path = format!("{name}\\{value}");
                let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
                let mut size = 0;
                if RegGetValueW(key.0, PCWSTR(wide.as_ptr()), w!("Data"), RRF_RT_REG_BINARY, None, None, Some(&mut size)).is_err() || size == 0 || size > 4096 { continue; }
                let mut data = vec![0u8; size as usize];
                if RegGetValueW(key.0, PCWSTR(wide.as_ptr()), w!("Data"), RRF_RT_REG_BINARY, None, Some(data.as_mut_ptr().cast()), Some(&mut size)).is_ok() {
                    data.truncate(size as usize);
                    if night_light_state(&data).is_some() { records.push((path, data)); }
                }
            }
            records.sort_by_key(|(path,_)| !path.ends_with("perdevice"));
            records
        }
    }
    fn night_light() -> Option<bool> { records().first().and_then(|(_,d)| night_light_state(d)) }
    fn toggle_night_light() -> Result<()> {
        let records = records();
        let on = !records.first().and_then(|(_,d)| night_light_state(d)).ok_or_else(|| anyhow::anyhow!("Formato da luz noturna indisponível"))?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        for (path, data) in records {
            let changed = change_night_light(&data, on, now).ok_or_else(|| anyhow::anyhow!("Registro de luz noturna inválido"))?;
            let path: Vec<u16> = format!("{}\\{path}", unsafe { STORE.to_string()? }).encode_utf16().chain(Some(0)).collect();
            // Only existing, validated Data values; never create an invented blob.
            unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr()), w!("Data"), REG_BINARY.0, Some(changed.as_ptr().cast()), changed.len() as u32).ok()?; }
        }
        Ok(())
    }
    fn bluetooth(toggle: bool) -> Result<Option<bool>> {
        let radios = Radio::GetRadiosAsync()?.get()?;
        let mut bluetooth = Vec::new();
        for radio in &radios {
            if radio.Kind()? == RadioKind::Bluetooth { bluetooth.push(radio); }
        }
        let mut on = false;
        for radio in &bluetooth { on |= radio.State()? == RadioState::On; }
        if toggle {
            anyhow::ensure!(!bluetooth.is_empty(), "Rádio Bluetooth indisponível");
            anyhow::ensure!(Radio::RequestAccessAsync()?.get()? == RadioAccessStatus::Allowed, "Acesso ao Bluetooth negado");
            for radio in &bluetooth {
                let result = radio.SetStateAsync(if on { RadioState::Off } else { RadioState::On })?.get()?;
                anyhow::ensure!(result == RadioAccessStatus::Allowed, "Windows recusou mudar o Bluetooth");
            }
        }
        if bluetooth.is_empty() { Ok(None) } else { Ok(Some(on)) }
    }
    struct Wlan(HANDLE);
    impl Drop for Wlan { fn drop(&mut self) { unsafe { WlanCloseHandle(self.0, None); } } }
    struct WlanMemory(*mut std::ffi::c_void);
    impl Drop for WlanMemory { fn drop(&mut self) { unsafe { WlanFreeMemory(self.0); } } }
    fn wifi() -> Result<(bool, Option<String>)> {
        unsafe {
            let mut handle = HANDLE::default(); let mut version = 0;
            let error = WlanOpenHandle(2, None, &mut version, &mut handle);
            anyhow::ensure!(error == 0, "WLAN: {error}");
            let handle = Wlan(handle);
            let mut list = ptr::null_mut();
            let error = WlanEnumInterfaces(handle.0, None, &mut list);
            anyhow::ensure!(error == 0 && !list.is_null(), "Interfaces WLAN: {error}");
            let _list = WlanMemory(list.cast());
            let interfaces = std::slice::from_raw_parts((*list).InterfaceInfo.as_ptr(), (*list).dwNumberOfItems as usize);
            for interface in interfaces {
                if interface.isState != wlan_interface_state_connected { continue; }
                let mut size = 0; let mut data = ptr::null_mut();
                let error = WlanQueryInterface(handle.0, &interface.InterfaceGuid, wlan_intf_opcode_current_connection, None, &mut size, &mut data, None);
                if error != 0 || data.is_null() { return Ok((true, None)); }
                let _data = WlanMemory(data);
                if (size as usize) < std::mem::size_of::<WLAN_CONNECTION_ATTRIBUTES>() { return Ok((true, None)); }
                let ssid = &(*(data as *const WLAN_CONNECTION_ATTRIBUTES)).wlanAssociationAttributes.dot11Ssid;
                let length = (ssid.uSSIDLength as usize).min(ssid.ucSSID.len());
                return Ok((true, Some(String::from_utf8_lossy(&ssid.ucSSID[..length]).into_owned())));
            }
            Ok((false, None))
        }
    }
    fn status() -> ControlStatus {
        let wifi = wifi().ok();
        ControlStatus {
            wifi_connected: wifi.as_ref().map(|w| w.0), wifi_name: wifi.and_then(|w| w.1),
            bluetooth_on: bluetooth(false).ok().flatten(), night_light_on: night_light(),
        }
    }
    fn captions() -> Result<()> {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        let inputs = caption_key_sequence().map(|(key,up)| INPUT { r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(key),
                dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) }, ..Default::default() } } });
        let count = unsafe { SendInput(&inputs,std::mem::size_of::<INPUT>() as i32) };
        if count != inputs.len() as u32 {
            // A partial send must still release every modifier before opening the fallback.
            unsafe { SendInput(&inputs[3..],std::mem::size_of::<INPUT>() as i32); }
            anyhow::bail!("Windows recusou o atalho de legendas");
        }
        Ok(())
    }
    fn activate(tile: ControlTile) -> Result<()> {
        let quick = match tile {
            ControlTile::Wifi => QuickAction::NetworkSettings,
            ControlTile::Bluetooth => {
                if let Err(err) = bluetooth(true) { tracing::warn!("Bluetooth: {err:#}"); }
                else { return Ok(()); }
                QuickAction::BluetoothSettings
            }
            ControlTile::NightLight => {
                if let Err(err) = toggle_night_light() { tracing::warn!("luz noturna: {err:#}"); }
                else { return Ok(()); }
                QuickAction::NightLightSettings
            }
            // shortcut: the mod uses private QuietHours COM/WNF; open Notifications until a supported toggle exists.
            ControlTile::DoNotDisturb => QuickAction::NotificationSettings,
            ControlTile::Project => QuickAction::ProjectDisplay,
            ControlTile::Cast => QuickAction::CastDisplay,
            ControlTile::Captions => {
                if let Err(err) = captions() { tracing::warn!("legendas ao vivo: {err:#}"); }
                else { return Ok(()); }
                QuickAction::Captions
            }
            ControlTile::Accessibility => QuickAction::Accessibility,
            ControlTile::TaskManager => QuickAction::TaskManager,
            ControlTile::Explorer => QuickAction::Explorer,
            ControlTile::WindowsSettings => QuickAction::WindowsSettings,
            ControlTile::Hotspot => QuickAction::Hotspot,
            ControlTile::Airplane => QuickAction::Airplane,
            ControlTile::Brightness => QuickAction::Display,
            ControlTile::Battery => QuickAction::Battery,
            ControlTile::PowerMode => QuickAction::PowerMode,
            ControlTile::Calculator => QuickAction::Calculator,
            ControlTile::RecycleBin => QuickAction::RecycleBin,
            ControlTile::Drives => QuickAction::PinnedFolder("thispc"),
            ControlTile::DarkMode => return switch_dark_mode(),
            ControlTile::Lock => return crate::power::run(isle_core::power::PowerAction::Lock),
            ControlTile::OnScreenKeyboard => return shell_open(w!("osk.exe")),
            ControlTile::TouchKeyboard => return touch_keyboard(),
            ControlTile::Screenshot => {
                // After the island is gone, so it isn't in the picture (the mod waits 450 ms).
                std::thread::sleep(Duration::from_millis(450));
                return press_keys(&SCREENSHOT_KEYS);
            }
            ControlTile::Emoji => return press_keys(&EMOJI_KEYS),
            ControlTile::Clipboard => return press_keys(&CLIPBOARD_KEYS),
            ControlTile::ShowDesktop => return press_keys(&SHOW_DESKTOP_KEYS),
            ControlTile::WinX => return press_keys(&WIN_X_KEYS),
            // The island's own (its menu, its tabs, the microphone): never sent here.
            ControlTile::Power | ControlTile::Microphone | ControlTile::Timer | ControlTile::Pomodoro
            | ControlTile::Alarms | ControlTile::Stopwatch | ControlTile::Weather | ControlTile::NextEvent => {
                anyhow::bail!("{tile:?} abre na ilha")
            }
        };
        execute_quick_action(quick)
    }
    /// Port of PressKeys (mac-island 3.1.1): each key down in order, then up in reverse.
    fn press_keys(keys: &[u16]) -> Result<()> {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        let inputs: Vec<INPUT> = key_sequence(keys).into_iter().map(|(key, up)| INPUT { r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(key),
                dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) }, ..Default::default() } } }).collect();
        let count = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if count != inputs.len() as u32 {
            // Never leave a modifier held down.
            unsafe { SendInput(&inputs[keys.len()..], std::mem::size_of::<INPUT>() as i32); }
            anyhow::bail!("Windows recusou o atalho");
        }
        Ok(())
    }
    /// ShellExecute "open": osk.exe asks for UI access, which a plain CreateProcess cannot give.
    fn shell_open(file: PCWSTR) -> Result<()> {
        use windows::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
        let done = unsafe { ShellExecuteW(None, w!("open"), file, None, None, SW_SHOWNORMAL) };
        anyhow::ensure!(done.0 as isize > 32, "Windows não abriu o programa ({})", done.0 as isize);
        Ok(())
    }
    /// Port of ToggleTouchKeyboard's fallback (mac-island 3.1.1): TabTip.exe from the 64-bit
    /// Common Files. shortcut: the mod first toggles it over COM (ITipInvocation).
    fn touch_keyboard() -> Result<()> {
        let base = std::env::var("CommonProgramW6432").or_else(|_| std::env::var("CommonProgramFiles"))?;
        let path: Vec<u16> = format!(r"{base}\microsoft shared\ink\TabTip.exe").encode_utf16().chain(Some(0)).collect();
        shell_open(PCWSTR(path.as_ptr()))
    }
    /// Port of SetDarkMode and DarkModeThreadProc (mac-island 3.1.1): apps and Windows both switched,
    /// then every window told the colors changed.
    fn switch_dark_mode() -> Result<()> {
        use windows::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE};
        const KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
        let mut light = 1u32;
        let mut size = 4u32;
        unsafe {
            let _ = RegGetValueW(HKEY_CURRENT_USER, KEY, w!("AppsUseLightTheme"), RRF_RT_REG_DWORD, None, Some((&mut light as *mut u32).cast()), Some(&mut size));
            let next: u32 = if light == 0 { 1 } else { 0 };
            for name in [w!("AppsUseLightTheme"), w!("SystemUsesLightTheme")] {
                RegSetKeyValueW(HKEY_CURRENT_USER, KEY, name, REG_DWORD.0, Some((&next as *const u32).cast()), 4).ok()?;
            }
            let colors = w!("ImmersiveColorSet");
            SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, windows::Win32::Foundation::WPARAM(0),
                windows::Win32::Foundation::LPARAM(colors.as_ptr() as isize), SMTO_ABORTIFHUNG, 500, None);
        }
        Ok(())
    }
    pub fn run(rx: Receiver<ControlCommand>, on_event: &impl Fn(ControlEvent)) -> Result<()> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()?; }
        let mut panel = false;
        loop {
            // shortcut: WLAN/radio/CloudStore are read every 3 s only while open; add notifications if needed outside the panel.
            let cmd = if panel { rx.recv_timeout(Duration::from_secs(3)) } else { rx.recv().map_err(|_| RecvTimeoutError::Disconnected) };
            match cmd {
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {},
                Ok(ControlCommand::Panel(open)) => { panel = open; if !open { continue; } }
                Ok(ControlCommand::OpenMixer) => if let Err(err) = execute_quick_action(QuickAction::SoundSettings) { on_event(ControlEvent::Error(format!("Mixer: {err:#}"))); },
                Ok(ControlCommand::Activate(tile)) => if let Err(err) = activate(tile) { on_event(ControlEvent::Error(format!("Controles: {err:#}"))); },
            }
            on_event(ControlEvent::Status(status()));
        }
        unsafe { CoUninitialize(); }
        Ok(())
    }
}

#[cfg(test)]
mod shortcut_tests {
    use super::*;
    #[test]
    fn captions_press_win_ctrl_l_then_release_in_reverse() {
        assert_eq!(caption_key_sequence(), [(0x5b,false),(0x11,false),(0x4c,false),(0x4c,true),(0x11,true),(0x5b,true)]);
    }
    #[test]
    fn settings_fallbacks_are_explicit_windows_pages() {
        assert_eq!(QuickAction::NotificationSettings.windows_command(), ("explorer.exe",vec!["ms-settings:notifications"]));
        assert_eq!(QuickAction::SoundSettings.windows_command(), ("explorer.exe",vec!["ms-settings:apps-volume"]));
        assert_eq!(QuickAction::CastDisplay.windows_command(), ("explorer.exe",vec!["ms-settings:connecteddevices"]));
        assert_eq!(QuickAction::WindowsSettings.windows_command(), ("explorer.exe",vec!["ms-settings:"]));
        assert_eq!(QuickAction::Hotspot.windows_command(), ("explorer.exe",vec!["ms-settings:network-mobilehotspot"]));
        assert_eq!(QuickAction::RecycleBin.windows_command(), ("explorer.exe",vec!["shell:RecycleBinFolder"]));
    }
    #[test]
    fn shortcuts_press_the_mods_keys_and_release_them_in_reverse() {
        assert_eq!(key_sequence(&SCREENSHOT_KEYS), [(0x5b,false),(0x10,false),(0x53,false),(0x53,true),(0x10,true),(0x5b,true)]);
        assert_eq!(key_sequence(&EMOJI_KEYS), [(0x5b,false),(0xbe,false),(0xbe,true),(0x5b,true)]);
        for keys in [CLIPBOARD_KEYS, SHOW_DESKTOP_KEYS, WIN_X_KEYS] { assert_eq!(keys[0], 0x5b, "with Win"); }
    }
}
/// Port of RunControlShortcut's keys (mac-island 3.1.1): Win+Shift+S, Win+., Win+V, Win+D, Win+X.
#[cfg(any(windows, test))]
const SCREENSHOT_KEYS: [u16; 3] = [0x5b, 0x10, 0x53];
#[cfg(any(windows, test))]
const EMOJI_KEYS: [u16; 2] = [0x5b, 0xbe];
#[cfg(any(windows, test))]
const CLIPBOARD_KEYS: [u16; 2] = [0x5b, 0x56];
#[cfg(any(windows, test))]
const SHOW_DESKTOP_KEYS: [u16; 2] = [0x5b, 0x44];
#[cfg(any(windows, test))]
const WIN_X_KEYS: [u16; 2] = [0x5b, 0x58];

/// Each key down in order, then each up in reverse (PressKeys).
#[cfg(any(windows, test))]
fn key_sequence(keys: &[u16]) -> Vec<(u16, bool)> {
    keys.iter().map(|k| (*k, false)).chain(keys.iter().rev().map(|k| (*k, true))).collect()
}

#[cfg(any(windows,test))]
fn caption_key_sequence() -> [(u16,bool);6] {
    [(0x5b,false),(0x11,false),(0x4c,false),(0x4c,true),(0x11,true),(0x5b,true)]
}

#[cfg(test)]
mod usb_tests {
    use super::*;

    #[test]
    fn normalizes_drive_letters_and_rejects_paths_and_shell_syntax() {
        for drive in ["f", "f:", "F:\\"] {
            assert_eq!(usb_drive_letter(drive).unwrap(), 'F');
        }
        for drive in ["", "1:", "é:", "C:\\Windows", "C:'; exit", "C://", " C:"] {
            assert!(usb_drive_letter(drive).is_err(), "{drive:?}");
        }
    }
}
