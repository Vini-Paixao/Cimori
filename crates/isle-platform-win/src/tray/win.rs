// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Out-of-process adapter of Mac Island & Dock 3.1.1 tray behavior; see ../tray.rs.
// COM objects stay on this MTA worker; callbacks only coalesce a wake.

use super::*;
use anyhow::{bail, Context, Result};
use isle_core::tray::{
    correlate_buttons, label_pins, match_item_button, normalized_path, order_items,
};
use std::{
    collections::HashSet,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use windows::{
    core::{implement, w, Interface, GUID, PCWSTR, PWSTR, VARIANT},
    Win32::{
        Foundation::{CloseHandle, HANDLE, HWND, POINT, WAIT_FAILED, WAIT_OBJECT_0},
        Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW},
        System::{Com::*, Diagnostics::ToolHelp::*, Registry::*, Threading::*},
        UI::{
            Accessibility::*, HiDpi::*, Input::KeyboardAndMouse::*, Shell::*,
            WindowsAndMessaging::*,
        },
    },
};

pub(super) struct Signal(isize);
impl Signal {
    pub fn new() -> Result<Self> {
        Ok(Self(
            unsafe { CreateEventW(None, false, false, None)? }.0 as isize,
        ))
    }
    fn handle(&self) -> HANDLE {
        HANDLE(self.0 as *mut _)
    }
    pub fn set(&self) {
        let _ = unsafe { SetEvent(self.handle()) };
    }
}
impl Drop for Signal {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.handle()) };
    }
}

struct Com;
impl Com {
    fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

struct Key(HKEY);
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_folder_paths_expand_and_malformed_guids_never_panic() {
        assert!(
            expand_path(r"{6D809377-6AF0-444B-8957-A3773F02200E}\Steam\steam.exe")
                .ends_with(r"\Steam\steam.exe")
        );
        assert!(
            !expand_path(r"{6D809377-6AF0-444B-8957-A3773F02200E}\Steam\steam.exe")
                .starts_with('{')
        );
        assert_eq!(expand_path("{bad}\\app.exe"), "{bad}\\app.exe");
    }
    #[test]
    fn program_names_fall_back_to_the_filename_when_version_info_is_missing() {
        assert_eq!(program_name(r"C:\missing-isle-test\Example.EXE"), "Example");
    }
    #[test]
    fn xaml_selector_rejects_shell_controls_and_generic_buttons() {
        assert!(is_application_button("SystemTray.NormalButton", ""));
        assert!(is_application_button("", "NotifyItemIcon123"));
        for class in [
            "Button",
            "SystemTray.ChevronIcon",
            "SystemTray.OmniButton",
            "SystemTray.NotificationCenterButton",
        ] {
            assert!(!is_application_button(class, "SystemTrayIcon"));
        }
    }
}
impl Key {
    fn open(parent: HKEY, path: &str) -> Result<Self> {
        let wide: Vec<_> = path.encode_utf16().chain(Some(0)).collect();
        let mut key = HKEY::default();
        unsafe {
            RegOpenKeyExW(parent, PCWSTR(wide.as_ptr()), 0, KEY_READ, &mut key).ok()?;
        }
        Ok(Self(key))
    }
    fn value(&self, name: &str) -> Vec<u8> {
        let name: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
        let mut len = 0;
        unsafe {
            if RegQueryValueExW(
                self.0,
                PCWSTR(name.as_ptr()),
                None,
                None,
                None,
                Some(&mut len),
            )
            .is_err()
                || len > 1024 * 1024
            {
                return Vec::new();
            }
            let mut bytes = vec![0; len as usize];
            if RegQueryValueExW(
                self.0,
                PCWSTR(name.as_ptr()),
                None,
                None,
                Some(bytes.as_mut_ptr()),
                Some(&mut len),
            )
            .is_err()
            {
                return Vec::new();
            }
            bytes.truncate(len as usize);
            bytes
        }
    }
    fn string(&self, name: &str) -> String {
        let bytes = self.value(name);
        let wide: Vec<_> = bytes
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .take_while(|c| *c != 0)
            .collect();
        String::from_utf16_lossy(&wide)
    }
}
impl Drop for Key {
    fn drop(&mut self) {
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

fn parse_guid(text: &str) -> Option<GUID> {
    let wide: Vec<_> = text.encode_utf16().chain(Some(0)).collect();
    unsafe { CLSIDFromString(PCWSTR(wide.as_ptr())).ok() }
}

fn expand_path(path: &str) -> String {
    // Explorer stores known-folder GUIDs, not necessarily drive paths.
    if let Some((prefix, rest)) = path.split_once('}') {
        if let Some(guid) = parse_guid(&format!("{prefix}}}")) {
            unsafe {
                if let Ok(folder) = SHGetKnownFolderPath(&guid, KF_FLAG_DEFAULT, None) {
                    let expanded = format!("{}{}", folder.to_string().unwrap_or_default(), rest);
                    CoTaskMemFree(Some(folder.0 as _));
                    return expanded;
                }
            }
        }
    }
    path.to_owned()
}

// TrayIconName/GetProgramName in the mod: FileDescription, then the executable basename.
pub fn program_name(path: &str) -> String {
    let wide: Vec<_> = path.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(wide.as_ptr()), None);
        if size > 0 && size <= 1024 * 1024 {
            let mut data = vec![0u8; size as usize];
            let mut value = std::ptr::null_mut();
            let mut length = 0;
            if GetFileVersionInfoW(PCWSTR(wide.as_ptr()), 0, size, data.as_mut_ptr() as _).is_ok()
                && VerQueryValueW(
                    data.as_ptr() as _,
                    w!("\\VarFileInfo\\Translation"),
                    &mut value,
                    &mut length,
                )
                .as_bool()
                && length >= 4
                && !value.is_null()
            {
                let language = std::ptr::read_unaligned(value as *const u16);
                let codepage = std::ptr::read_unaligned((value as *const u16).add(1));
                let query =
                    format!("\\StringFileInfo\\{language:04x}{codepage:04x}\\FileDescription");
                let query: Vec<_> = query.encode_utf16().chain(Some(0)).collect();
                if VerQueryValueW(
                    data.as_ptr() as _,
                    PCWSTR(query.as_ptr()),
                    &mut value,
                    &mut length,
                )
                .as_bool()
                    && !value.is_null()
                    && (1..=16384).contains(&length)
                {
                    let text = String::from_utf16_lossy(std::slice::from_raw_parts(
                        value as *const u16,
                        length as usize,
                    ))
                    .trim_end_matches('\0')
                    .trim()
                    .to_owned();
                    if !text.is_empty() {
                        return text;
                    }
                }
            }
        }
    }
    let file = path.rsplit(['\\', '/']).next().unwrap_or("Aplicativo");
    if file.to_lowercase().ends_with(".exe") {
        file[..file.len() - 4].to_owned()
    } else {
        file.to_owned()
    }
}

fn registry_items() -> Result<Vec<TrayItem>> {
    let root = Key::open(HKEY_CURRENT_USER, r"Control Panel\NotifyIconSettings")?;
    let mut items = Vec::new();
    for index in 0..4096 {
        let mut name = [0u16; 256];
        let mut len = name.len() as u32;
        let status = unsafe {
            RegEnumKeyExW(
                root.0,
                index,
                PWSTR(name.as_mut_ptr()),
                &mut len,
                None,
                PWSTR::null(),
                None,
                None,
            )
        };
        if status == windows::Win32::Foundation::ERROR_NO_MORE_ITEMS {
            break;
        }
        status.ok()?;
        let name = String::from_utf16_lossy(&name[..len as usize]);
        let Ok(key) = Key::open(root.0, &name) else {
            continue;
        };
        let exe_path = expand_path(&key.string("ExecutablePath"));
        if exe_path.is_empty() {
            continue;
        }
        let guid = key.string("IconGuid");
        // The mod excludes shell volume/network/power icons too.
        if guid.to_lowercase().starts_with("{7820ae7") {
            continue;
        }
        let tooltip = key.string("InitialTooltip");
        let display_name = if tooltip.trim().is_empty() {
            program_name(&exe_path)
        } else {
            tooltip.clone()
        };
        let uid = key.value("UID");
        let uid = uid
            .get(..4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .unwrap_or(0);
        items.push(TrayItem {
            key: format!("registry:{name}"),
            tooltip,
            display_name,
            exe_path,
            guid: (!guid.is_empty()).then_some(guid),
            uid,
            png: key.value("IconSnapshot"),
            ..Default::default()
        });
    }
    Ok(items)
}

fn running_paths() -> HashSet<String> {
    let mut paths = HashSet::new();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return paths;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut next = Process32FirstW(snapshot, &mut entry);
        while next.is_ok() {
            if let Ok(process) = OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                entry.th32ProcessID,
            ) {
                let mut path = vec![0u16; 32768];
                let mut size = path.len() as u32;
                if QueryFullProcessImageNameW(
                    process,
                    PROCESS_NAME_WIN32,
                    PWSTR(path.as_mut_ptr()),
                    &mut size,
                )
                .is_ok()
                {
                    paths.insert(normalized_path(&String::from_utf16_lossy(
                        &path[..size as usize],
                    )));
                }
                let _ = CloseHandle(process);
            }
            next = Process32NextW(snapshot, &mut entry);
        }
        let _ = CloseHandle(snapshot);
    }
    paths
}

struct Button {
    element: IUIAutomationElement,
    name: String,
    id: String,
    legacy: bool,
}
fn is_application_button(class: &str, id: &str) -> bool {
    class.ends_with(".NormalButton") || id.starts_with("NotifyItemIcon")
}

struct Shell {
    uia: IUIAutomation,
}
impl Shell {
    fn new() -> Result<Self> {
        let uia: IUIAutomation =
            unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
        if let Ok(v2) = uia.cast::<IUIAutomation2>() {
            unsafe {
                v2.SetConnectionTimeout(1000)?;
                v2.SetTransactionTimeout(1000)?;
            }
        }
        Ok(Self { uia })
    }

    fn roots(&self) -> Vec<HWND> {
        let mut roots = Vec::new();
        // XAML overflow is a separate top-level HWND.
        for class in [
            w!("Shell_TrayWnd"),
            w!("Shell_SecondaryTrayWnd"),
            w!("TopLevelWindowForOverflowXamlIsland"),
            w!("NotifyIconOverflowWindow"),
        ] {
            let mut previous = HWND::default();
            while let Ok(hwnd) = unsafe { FindWindowExW(None, previous, class, PCWSTR::null()) } {
                if hwnd.0.is_null() {
                    break;
                }
                roots.push(hwnd);
                previous = hwnd;
            }
        }
        roots
    }

    fn descendants(&self, hwnd: HWND) -> Result<Vec<IUIAutomationElement>> {
        unsafe {
            let root = self.uia.ElementFromHandle(hwnd)?;
            let all = root.FindAll(TreeScope_Descendants, &self.uia.CreateTrueCondition()?)?;
            let mut elements = Vec::new();
            for i in 0..all.Length()?.min(2048) {
                if let Ok(e) = all.GetElement(i) {
                    elements.push(e);
                }
            }
            Ok(elements)
        }
    }

    fn add_button(&self, buttons: &mut Vec<Button>, element: IUIAutomationElement, legacy: bool) {
        unsafe {
            if element.CurrentControlType().ok() != Some(UIA_ButtonControlTypeId) {
                return;
            }
            let name = element
                .CurrentName()
                .map(|s| s.to_string())
                .unwrap_or_default();
            let id = element
                .CurrentAutomationId()
                .map(|s| s.to_string())
                .unwrap_or_default();
            if !name.is_empty()
                && !buttons.iter().any(|b| {
                    self.uia
                        .CompareElements(&b.element, &element)
                        .is_ok_and(|v| v.as_bool())
                })
            {
                buttons.push(Button {
                    element,
                    name,
                    id,
                    legacy,
                });
            }
        }
    }

    fn buttons(&self) -> Vec<Button> {
        let mut buttons = Vec::new();
        for root in self.roots() {
            for element in self.descendants(root).unwrap_or_default() {
                let class = unsafe { element.CurrentClassName() }
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let id = unsafe { element.CurrentAutomationId() }
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                if is_application_button(&class, &id) {
                    self.add_button(&mut buttons, element, false);
                }
            }
            // Classic compatibility only: scan application buttons INSIDE notification toolbars,
            // never all Buttons under TrayNotifyWnd (which includes the chevron/clock/quick settings).
            let mut class = [0u16; 128];
            let n = unsafe { GetClassNameW(root, &mut class) };
            let legacy_root =
                if String::from_utf16_lossy(&class[..n as usize]) == "NotifyIconOverflowWindow" {
                    Some(root)
                } else {
                    unsafe { FindWindowExW(root, None, w!("TrayNotifyWnd"), PCWSTR::null()) }.ok()
                };
            if let Some(legacy_root) = legacy_root {
                for toolbar in self.descendants(legacy_root).unwrap_or_default() {
                    if unsafe { toolbar.CurrentClassName() }
                        .is_ok_and(|c| c.to_string() == "ToolbarWindow32")
                    {
                        unsafe {
                            if let Ok(all) = self
                                .uia
                                .CreateTrueCondition()
                                .and_then(|c| toolbar.FindAll(TreeScope_Descendants, &c))
                            {
                                for i in 0..all.Length().unwrap_or(0).min(512) {
                                    if let Ok(element) = all.GetElement(i) {
                                        self.add_button(&mut buttons, element, true);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        buttons
    }

    fn overflow(&self) -> Option<HWND> {
        self.roots().into_iter().find(|hwnd| unsafe {
            let mut class = [0u16; 128];
            let n = GetClassNameW(*hwnd, &mut class);
            String::from_utf16_lossy(&class[..n as usize]).contains("Overflow")
                && IsWindowVisible(*hwnd).as_bool()
        })
    }

    fn chevron(&self) -> Option<IUIAutomationElement> {
        for root in self.roots() {
            for element in self.descendants(root).unwrap_or_default() {
                let class = unsafe { element.CurrentClassName() }
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let id = unsafe { element.CurrentAutomationId() }
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                if class.ends_with(".ChevronIcon")
                    || id == "SystemTrayIcon" && class.contains("Chevron")
                {
                    return Some(element);
                }
                if class == "Button" && id == "1502" {
                    return Some(element);
                }
            }
        }
        None
    }

    fn open_overflow(&self) -> Result<bool> {
        if self.overflow().is_some() {
            return Ok(false);
        }
        let Some(chevron) = self.chevron() else {
            return Ok(false);
        };
        unsafe {
            chevron
                .GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)?
                .Invoke()?;
        }
        // Bounded readiness wait after explicit user action only.
        for _ in 0..10 {
            if self.overflow().is_some() {
                return Ok(true);
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        bail!("O Windows não abriu os ícones ocultos")
    }

    fn close_owned_overflow(&self) {
        if self.overflow().is_some() {
            if let Some(button) = self.chevron() {
                let _ = unsafe {
                    button
                        .GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)
                        .and_then(|p| p.Invoke())
                };
            }
        }
    }

    fn snapshot(&self, pins: &[PinnedTrayApp]) -> TraySnapshot {
        tracing::debug!("bandeja: lendo registro");
        let records = registry_items();
        let mut error = records
            .as_ref()
            .err()
            .map(|e| format!("Não foi possível ler a bandeja: {e}"));
        let paths = running_paths();
        let mut records = records.unwrap_or_default();
        records.sort_by_key(|i| !paths.contains(&normalized_path(&i.exe_path)));
        let mut items = records;
        label_pins(&mut items, pins);
        tracing::debug!(
            "bandeja: {} registros, {} processos; lendo UIA",
            items.len(),
            paths.len()
        );
        let buttons = self.buttons();
        tracing::debug!("bandeja: {} botões UIA", buttons.len());
        let names: Vec<_> = buttons.iter().map(|b| b.name.clone()).collect();
        let mut used = HashSet::new();
        let matches = correlate_buttons(&items, &names);
        let mut index = 0;
        items.retain_mut(|item| {
            let button = matches[index];
            index += 1;
            // A real button with ambiguous registry identities is represented once by UIA below.
            if button.is_none() && match_item_button(item, &names).is_some() {
                return false;
            }
            item.confirmed = button.is_some();
            if let Some(i) = button {
                item.tooltip = buttons[i].name.clone();
                used.insert(i);
            }
            item.confirmed || paths.contains(&normalized_path(&item.exe_path))
        });
        for (i, button) in buttons
            .iter()
            .enumerate()
            .filter(|(i, b)| !used.contains(i) && !b.legacy)
        {
            items.push(TrayItem {
                key: format!("uia:{}:{}", button.id, button.name),
                tooltip: names[i].clone(),
                display_name: names[i].clone(),
                confirmed: true,
                ..Default::default()
            });
        }
        if buttons.is_empty() && self.roots().is_empty() {
            error = Some("A barra de tarefas não está acessível nesta sessão".into());
        }
        let snapshot = TraySnapshot { bridge: false,
            items: order_items(items, pins),
            error,
        };
        tracing::debug!("bandeja: snapshot pronto: {} ícones", snapshot.items.len());
        snapshot
    }

    fn activate(&self, item: &TrayItem, peers: &[TrayItem], right: bool) -> Result<()> {
        // No stale row number, COM element or rectangle is used for activation.
        let find = || -> Option<IUIAutomationElement> {
            let buttons = self.buttons();
            let names: Vec<_> = buttons.iter().map(|b| b.name.clone()).collect();
            let index = peers.iter().position(|p| p.key == item.key)?;
            correlate_buttons(peers, &names)[index].map(|i| buttons[i].element.clone())
        };
        let mut button = find();
        let mut opened = false;
        let visible = button
            .as_ref()
            .is_some_and(|b| unsafe { b.CurrentIsOffscreen().is_ok_and(|v| !v.as_bool()) });
        if !visible {
            opened = self.open_overflow()?;
            button = find();
        }
        let result = (|| {
            let button = button.context("Ícone não está mais disponível ou tem nome ambíguo")?;
            if !right {
                if let Ok(invoke) = unsafe {
                    button.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)
                } {
                    unsafe {
                        invoke.Invoke()?;
                    }
                    return Ok(());
                }
            }
            click_real_button(&button, right)
        })();
        // On success the shell owns the app menu/flyout; don't toggle underneath it.
        if opened && result.is_err() {
            self.close_owned_overflow();
        }
        result
    }
}

fn click_real_button(button: &IUIAutomationElement, right: bool) -> Result<()> {
    unsafe {
        if button.CurrentIsOffscreen()?.as_bool() {
            bail!("Ícone oculto; mostre a barra de tarefas");
        }
        let rect = button.CurrentBoundingRectangle()?;
        if rect.right <= rect.left || rect.bottom <= rect.top {
            bail!("Ícone sem posição na barra de tarefas");
        }
        let point = POINT {
            x: rect.left + (rect.right - rect.left) / 2,
            y: rect.top + (rect.bottom - rect.top) / 2,
        };
        let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
        let hit = uia.ElementFromPoint(point)?;
        // ElementFromPoint can return the icon's Image child rather than its Button.
        let walker = uia.ControlViewWalker()?;
        let mut hit = hit;
        let mut same = false;
        for _ in 0..8 {
            if uia.CompareElements(button, &hit)?.as_bool() {
                same = true;
                break;
            }
            let Ok(parent) = walker.GetParentElement(&hit) else {
                break;
            };
            hit = parent;
        }
        if !same {
            bail!("Ícone coberto por outra janela");
        }
        let mut original = POINT::default();
        GetCursorPos(&mut original)?;
        SetCursorPos(point.x, point.y)?;
        let (down, up) = if right {
            (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP)
        } else {
            (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP)
        };
        let inputs = [down, up].map(|flags| INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        });
        let sent = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        if sent == 1 {
            let _ = SendInput(&inputs[1..], std::mem::size_of::<INPUT>() as i32);
        }
        let mut current = POINT::default();
        if GetCursorPos(&mut current).is_ok() && current == point {
            let _ = SetCursorPos(original.x, original.y);
        }
        if sent != 2 {
            bail!("Windows recusou o clique (permissões/UIPI)");
        }
        Ok(())
    }
}

#[implement(
    IUIAutomationStructureChangedEventHandler,
    IUIAutomationPropertyChangedEventHandler
)]
struct Events {
    tx: mpsc::Sender<TrayCommand>,
    signal: Arc<Signal>,
    pending: Arc<AtomicBool>,
}
impl Events_Impl {
    fn changed(&self) -> windows::core::Result<()> {
        if !self.pending.swap(true, Ordering::AcqRel) {
            let _ = self.tx.send(TrayCommand::Changed);
            self.signal.set();
        }
        Ok(())
    }
}
impl IUIAutomationStructureChangedEventHandler_Impl for Events_Impl {
    fn HandleStructureChangedEvent(
        &self,
        _: Option<&IUIAutomationElement>,
        _: StructureChangeType,
        _: *const SAFEARRAY,
    ) -> windows::core::Result<()> {
        self.changed()
    }
}
impl IUIAutomationPropertyChangedEventHandler_Impl for Events_Impl {
    fn HandlePropertyChangedEvent(
        &self,
        _: Option<&IUIAutomationElement>,
        _: UIA_PROPERTY_ID,
        _: &VARIANT,
    ) -> windows::core::Result<()> {
        self.changed()
    }
}

pub(super) fn inspect(pins: &[PinnedTrayApp]) -> Result<TraySnapshot> {
    let pins = pins.to_vec();
    let (tx, rx) = mpsc::channel();
    // UIA can hang tearing down a disconnected provider. Diagnostics must still terminate;
    // don't join this COM thread. The normal service keeps its apartment for its lifetime.
    std::thread::spawn(move || {
        let result = (|| {
            let _com = Com::new()?;
            let shell = Shell::new()?;
            let snapshot = shell.snapshot(&pins);
            let _ = tx.send(Ok(snapshot));
            Ok::<_, anyhow::Error>(())
        })();
        if let Err(err) = result {
            let _ = tx.send(Err(err));
        }
    });
    rx.recv_timeout(Duration::from_secs(8))
        .context("O shell não respondeu à leitura da bandeja em 8 s")?
}

pub(super) fn run(
    mut pins: Vec<PinnedTrayApp>,
    rx: mpsc::Receiver<TrayCommand>,
    tx: mpsc::Sender<TrayCommand>,
    signal: Arc<Signal>,
    bridged: super::bridge::Shared,
    publish: &impl Fn(TraySnapshot),
) -> Result<()> {
    let bridge_items = || bridged.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    // UIA rectangles are physical pixels, including on mixed-DPI taskbars.
    unsafe {
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let _com = Com::new()?;
    let shell = Shell::new()?;
    let pending = Arc::new(AtomicBool::new(false));
    let events: IUIAutomationStructureChangedEventHandler = Events {
        tx,
        signal: signal.clone(),
        pending: pending.clone(),
    }
    .into();
    let properties: IUIAutomationPropertyChangedEventHandler = events.cast()?;
    let desktop = unsafe { shell.uia.GetRootElement() }.ok();
    // CHILDREN catches Explorer/overflow creation without watching all app subtrees.
    if let Some(desktop) = &desktop {
        if let Err(err) = unsafe {
            shell
                .uia
                .AddStructureChangedEventHandler(desktop, TreeScope_Children, None, &events)
        } {
            tracing::warn!(
                "bandeja: eventos UIA indisponíveis, mantendo eventos do registro: {err}"
            );
        }
    }
    let mut subscribed = Vec::new();
    let registry_signal = Signal::new()?;
    let watch = Key::open(HKEY_CURRENT_USER, "Control Panel").ok();
    let arm = || {
        watch.as_ref().is_some_and(|key| unsafe {
            RegNotifyChangeKeyValue(
                key.0,
                true,
                REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET,
                registry_signal.handle(),
                true,
            )
            .is_ok()
        })
    };
    let mut watching = arm();
    let mut panel = false;
    let mut last = TraySnapshot::default();
    let mut first = true;
    let mut action_error = None;
    loop {
        let roots = shell.roots();
        if roots != subscribed {
            unsafe {
                let _ = shell.uia.RemoveAllEventHandlers();
                if let Some(desktop) = &desktop {
                    let _ = shell.uia.AddStructureChangedEventHandler(
                        desktop,
                        TreeScope_Children,
                        None,
                        &events,
                    );
                }
                for hwnd in &roots {
                    if let Ok(element) = shell.uia.ElementFromHandle(*hwnd) {
                        let _ = shell.uia.AddStructureChangedEventHandler(
                            &element,
                            TreeScope_Subtree,
                            None,
                            &events,
                        );
                        let _ = shell.uia.AddPropertyChangedEventHandlerNativeArray(
                            &element,
                            TreeScope_Subtree,
                            None,
                            &properties,
                            &[UIA_NamePropertyId, UIA_IsOffscreenPropertyId],
                        );
                    }
                }
            }
            subscribed = roots;
        }
        // With the Windhawk bridge: every icon, clickable without the taskbar.
        let mut snapshot = match bridge_items() {
            Some(items) => TraySnapshot { bridge: true,
                items: isle_core::tray::order_items(items.into_iter().map(|(item, _)| item).collect(), &pins),
                error: None,
            },
            None => shell.snapshot(&pins),
        };
        if let Some(error) = action_error.take() {
            snapshot.error = Some(error);
        }
        if first || snapshot != last {
            tracing::info!("bandeja: {} ícones ({} confirmados pelo shell), painel={panel}, registro_eventos={watching}, status={:?}",snapshot.items.len(),snapshot.items.iter().filter(|i| i.confirmed).count(),snapshot.error);
            publish(snapshot.clone());
            last = snapshot;
            first = false;
        }
        let handles = if watching {
            vec![signal.handle(), registry_signal.handle()]
        } else {
            vec![signal.handle()]
        };
        let wait =
            unsafe { WaitForMultipleObjects(&handles, false, if panel { 3000 } else { INFINITE }) };
        if wait == WAIT_FAILED {
            bail!("Falha ao esperar eventos da bandeja");
        }
        if wait.0 == WAIT_OBJECT_0.0 + 1 {
            watching = arm();
        }
        for command in rx.try_iter() {
            match command {
                TrayCommand::Stop => {
                    let _ = unsafe { shell.uia.RemoveAllEventHandlers() };
                    return Ok(());
                }
                TrayCommand::Changed => {
                    pending.store(false, Ordering::Release);
                }
                TrayCommand::Pins(new) => pins = new,
                TrayCommand::Panel(open) => {
                    if open && !panel && bridge_items().is_none() {
                        match shell.open_overflow() {
                            Ok(true) => {
                                let snapshot = shell.snapshot(&pins);
                                publish(snapshot.clone());
                                last = snapshot;
                                shell.close_owned_overflow();
                            }
                            Ok(false) => {}
                            Err(e) => action_error = Some(e.to_string()),
                        }
                    }
                    panel = open;
                }
                TrayCommand::Activate(key, right) => {
                    let target = bridge_items().and_then(|items| items.into_iter().find(|(i, _)| i.key == key).map(|(_, t)| t));
                    if let Some(target) = target {
                        if let Err(err) = super::bridge::click(&target, right) {
                            tracing::warn!("bandeja clique: {err:#}");
                            action_error = Some(err.to_string());
                        }
                    } else if let Some(item) = last.items.iter().find(|i| i.key == key) {
                        if let Err(err) = shell.activate(item, &last.items, right) {
                            tracing::warn!("bandeja clique: {err:#}");
                            action_error = Some(err.to_string());
                        }
                    } else {
                        action_error = Some("Ícone não está mais disponível".into());
                    }
                }
            }
        }
    }
}
