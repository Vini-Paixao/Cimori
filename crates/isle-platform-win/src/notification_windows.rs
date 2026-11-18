// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Ported from Mac Island & Dock v3.1.0: Sqlite/ReadNewToasts/ToastThreadProc,
// AppDisplayName, StartToastWatch and ShortcutKeyProc; read-only Rust worker.

use super::{arrival_ms, parse_toast, KeyDisposition, NotificationEvent, ReadCursor, ShortcutAction, ShortcutState};
use anyhow::{anyhow, bail, Context, Result};
use isle_core::notifications::Notification;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CString};
use std::sync::{Arc, Mutex};
use std::os::windows::ffi::OsStrExt;
use std::time::Duration;
use windows::core::{w, PCSTR, PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

type SendEvent = Arc<dyn Fn(NotificationEvent) + Send + Sync>;
type Raw = *mut c_void;
type Release = unsafe extern "system" fn(Raw) -> i32;

struct Library(HMODULE);
impl Library {
    unsafe fn load(name: PCWSTR) -> Result<Self> {
        Ok(Self(LoadLibraryExW(name, None, LOAD_LIBRARY_SEARCH_SYSTEM32)?))
    }
    unsafe fn get<T: Copy>(&self, name: &'static [u8]) -> Result<T> {
        let symbol = GetProcAddress(self.0, PCSTR(name.as_ptr())).context("API do Windows ausente")?;
        assert_eq!(std::mem::size_of::<T>(), std::mem::size_of_val(&symbol));
        Ok(std::mem::transmute_copy(&symbol))
    }
}
impl Drop for Library {
    fn drop(&mut self) { unsafe { let _ = FreeLibrary(self.0); } }
}

struct SqlHandle { raw: Raw, release: Release }
impl Drop for SqlHandle {
    fn drop(&mut self) { if !self.raw.is_null() { unsafe { (self.release)(self.raw); } } }
}

struct Sqlite {
    _library: Library,
    open: unsafe extern "system" fn(*const c_char, *mut Raw, i32, *const c_char) -> i32,
    prepare: unsafe extern "system" fn(Raw, *const c_char, i32, *mut Raw, *mut *const c_char) -> i32,
    bind: unsafe extern "system" fn(Raw, i32, i64) -> i32,
    step: unsafe extern "system" fn(Raw) -> i32,
    integer: unsafe extern "system" fn(Raw, i32) -> i64,
    text: unsafe extern "system" fn(Raw, i32) -> *const u8,
    blob: unsafe extern "system" fn(Raw, i32) -> *const u8,
    bytes: unsafe extern "system" fn(Raw, i32) -> i32,
    finalize: Release,
    close: Release,
    timeout: unsafe extern "system" fn(Raw, i32) -> i32,
}

impl Sqlite {
    unsafe fn load() -> Result<Self> {
        let library = Library::load(w!("winsqlite3.dll"))?;
        Ok(Self {
            open: library.get(b"sqlite3_open_v2\0")?, prepare: library.get(b"sqlite3_prepare_v2\0")?,
            bind: library.get(b"sqlite3_bind_int64\0")?, step: library.get(b"sqlite3_step\0")?,
            integer: library.get(b"sqlite3_column_int64\0")?, text: library.get(b"sqlite3_column_text\0")?,
            blob: library.get(b"sqlite3_column_blob\0")?, bytes: library.get(b"sqlite3_column_bytes\0")?,
            finalize: library.get(b"sqlite3_finalize\0")?, close: library.get(b"sqlite3_close\0")?,
            timeout: library.get(b"sqlite3_busy_timeout\0")?, _library: library,
        })
    }

    unsafe fn prepare(&self, db: Raw, query: &std::ffi::CStr) -> Result<SqlHandle> {
        let mut statement = SqlHandle { raw: std::ptr::null_mut(), release: self.finalize };
        let code = (self.prepare)(db, query.as_ptr(), -1, &mut statement.raw, std::ptr::null_mut());
        if code != 0 { bail!("Esquema de notificações indisponível (SQLite {code})"); }
        Ok(statement)
    }

    unsafe fn read(&self, path: &CString, cursor: &mut ReadCursor, names: &mut HashMap<String,String>) -> Result<(Vec<Notification>,bool,Vec<i64>,Vec<String>)> {
        let mut db = SqlHandle { raw: std::ptr::null_mut(), release: self.close };
        // SQLITE_OPEN_READONLY: no CREATE, DELETE, UPDATE or journal-mode changes.
        let code = (self.open)(path.as_ptr(), &mut db.raw, 1, std::ptr::null());
        if code != 0 { bail!("Banco de notificações indisponível (SQLite {code})"); }
        if (self.timeout)(db.raw,500) != 0 { bail!("Não foi possível configurar a espera do SQLite"); }
        let max = self.prepare(db.raw,c"SELECT COALESCE(MAX(Id),0) FROM Notification")?;
        let code = (self.step)(max.raw);
        if code != 100 { bail!("Não foi possível ler o último aviso (SQLite {code})"); }
        let max_id = (self.integer)(max.raw,0);
        drop(max);
        let (after,initial) = cursor.query_after(max_id);        let rows = self.prepare(db.raw,c"SELECT n.Id,h.PrimaryId,n.Payload,n.ArrivalTime FROM Notification n JOIN NotificationHandler h ON h.RecordId=n.HandlerId WHERE n.Type='toast' AND n.Id>?1 AND n.Id<=?2 ORDER BY n.Id DESC LIMIT 100")?;
        if (self.bind)(rows.raw,1,after) != 0 || (self.bind)(rows.raw,2,max_id) != 0 { bail!("Parâmetros de leitura recusados"); }
        let mut items = Vec::new();
        loop {
            let code = (self.step)(rows.raw);
            if code == 101 { break; }
            if code != 100 { bail!("Leitura de notificações interrompida (SQLite {code})"); }
            let app = (self.text)(rows.raw,1);
            let app_len = (self.bytes)(rows.raw,1);
            if app.is_null() || !(1..=65_536).contains(&app_len) { continue; }
            let Ok(app_id) = std::str::from_utf8(std::slice::from_raw_parts(app,app_len as usize)) else { continue; };
            let app_id = app_id.to_owned();
            let payload = (self.blob)(rows.raw,2);
            let size = (self.bytes)(rows.raw,2);
            if payload.is_null() || !(1..=1_048_576).contains(&size) { continue; }
            let payload = std::slice::from_raw_parts(payload,size as usize);
            let Some((title,body)) = parse_toast(payload) else { continue; };
            let (launch,launch_type) = super::toast_launch(payload);
            let (reply_input,reply_args) = super::toast_reply(payload);
            if names.len() > 128 { names.clear(); }
            let app_name = names.entry(app_id.clone()).or_insert_with(|| app_name(&app_id)).clone();
            items.push(Notification { app_icon:None,id: (self.integer)(rows.raw,0), app_id, app_name, title, body,
                arrival_ms: arrival_ms((self.integer)(rows.raw,3)), unread: false, launch, launch_type, reply_input, reply_args });
        }
        drop(rows);
        let ids = self.prepare(db.raw,c"SELECT Id FROM Notification WHERE Type='toast'")?;
        let mut present = Vec::new();
        loop {
            let code = (self.step)(ids.raw);
            if code == 101 { break; }
            if code != 100 { bail!("Leitura de notificações interrompida (SQLite {code})"); }
            present.push((self.integer)(ids.raw,0));
        }
        drop(ids);
        let badges = self.prepare(db.raw,c"SELECT h.PrimaryId,n.Payload FROM Notification n JOIN NotificationHandler h ON h.RecordId=n.HandlerId WHERE n.Type='badge'")?;
        let mut badged = Vec::new();
        loop {
            let code = (self.step)(badges.raw);
            if code == 101 { break; }
            if code != 100 { bail!("Leitura de notificações interrompida (SQLite {code})"); }
            let (app, app_len) = ((self.text)(badges.raw,0), (self.bytes)(badges.raw,0));
            let (payload, size) = ((self.blob)(badges.raw,1), (self.bytes)(badges.raw,1));
            if app.is_null() || payload.is_null() || !(1..=65_536).contains(&app_len) || !(1..=4096).contains(&size) { continue; }
            if !super::badge_shown(std::slice::from_raw_parts(payload,size as usize)) { continue; }
            if let Ok(app) = std::str::from_utf8(std::slice::from_raw_parts(app,app_len as usize)) { badged.push(app.to_owned()); }
        }
        // A partial/locked query must not advance the cursor and lose notifications.
        cursor.commit(max_id);
        items.reverse();
        Ok((items,initial,present,badged))
    }
}

unsafe fn app_name(app: &str) -> String {
    let wide: Vec<_> = app.encode_utf16().chain(Some(0)).collect();
    if let Ok(item) = SHCreateItemInKnownFolder::<_,IShellItem>(&FOLDERID_AppsFolder,KF_FLAG_DEFAULT,PCWSTR(wide.as_ptr())) {
        if let Ok(display) = item.GetDisplayName(SIGDN_NORMALDISPLAY) {
            let name = display.to_string().unwrap_or_default();
            CoTaskMemFree(Some(display.0.cast()));
            if !name.is_empty() { return name; }
        }
    }
    // shortcut: senders without a Start menu entry use their readable AUMID; add registry names if needed.
    super::fallback_app_name(app)
}

/// The mod's PackageFolder: a packaged app's install folder, from the family in its AUMID.
unsafe fn package_folder(app_id: &str) -> Option<String> {
    use windows::Win32::Storage::Packaging::Appx::*;
    let family = windows::core::HSTRING::from(app_id.split_once('!')?.0);
    let (mut count, mut length) = (0u32, 0u32);
    if GetPackagesByPackageFamily(&family,&mut count,None,&mut length,PWSTR::null()) != ERROR_INSUFFICIENT_BUFFER || count == 0 { return None; }
    let mut names = vec![PWSTR::null(); count as usize];
    let mut buffer = vec![0u16; length as usize];
    if GetPackagesByPackageFamily(&family,&mut count,Some(names.as_mut_ptr()),&mut length,PWSTR(buffer.as_mut_ptr())) != ERROR_SUCCESS { return None; }
    let full = PCWSTR(names[0].0);
    let mut size = 0u32;
    if GetPackagePathByFullName(full,&mut size,PWSTR::null()) != ERROR_INSUFFICIENT_BUFFER { return None; }
    let mut path = vec![0u16; size as usize];
    if GetPackagePathByFullName(full,&mut size,PWSTR(path.as_mut_ptr())) != ERROR_SUCCESS { return None; }
    Some(String::from_utf16_lossy(&path[..path.iter().position(|&c| c == 0).unwrap_or(path.len())]))
}

/// Port of the protocol branch of ActivateToast (mac-island 3.1.1): the URI through the shell.
pub(super) fn open_uri(uri: &str) -> Result<()> {
    let _ = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
    let uri = windows::core::HSTRING::from(uri);
    let result = unsafe { ShellExecuteW(None, w!("open"), &uri, PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
    if result.0 as isize <= 32 { bail!("o Windows não abriu o link"); }
    Ok(())
}

// Port of UnpackagedToastActivator / ReadToastShortcuts / FindToastActivator (mac-island 3.1.1).
// PKEY_AppUserModel_ID is {9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3},5; ToastActivatorCLSID is pid 26.
const AUMID_FORMAT: windows::core::GUID = windows::core::GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3);
const SHORTCUTS_MAX_ENTRIES: i32 = 5000;
const SHORTCUTS_MAX_DEPTH: u32 = 4;
const SHORTCUTS_REFRESH: Duration = Duration::from_secs(5 * 60);

static SHORTCUT_ACTIVATORS: Mutex<Option<(std::time::Instant, Vec<(String,String)>)>> = Mutex::new(None);

/// A value of an app's registration as a notification sender (Software\Classes\AppUserModelId\<ID>).
fn registered_value(app_id: &str, name: PCWSTR) -> Option<String> {
    use windows::Win32::System::Registry::*;
    if app_id.is_empty() || app_id.len() > 256 || app_id.contains('\\') { return None; }
    let key = windows::core::HSTRING::from(format!("Software\\Classes\\AppUserModelId\\{app_id}"));
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let mut value = [0u16; 128];
        let mut size = std::mem::size_of_val(&value) as u32;
        let status = unsafe { RegGetValueW(root, &key, name, RRF_RT_REG_SZ, None, Some(value.as_mut_ptr().cast()), Some(&mut size)) };
        if status == ERROR_SUCCESS {
            let text = String::from_utf16_lossy(&value[..value.iter().position(|&c| c == 0).unwrap_or(value.len())]);
            if !text.is_empty() { return Some(text); }
        }
    }
    None
}

fn read_shortcuts(folder: &std::path::Path, depth: u32, budget: &mut i32, found: &mut Vec<(String,String)>) {
    use windows::Win32::UI::Shell::PropertiesSystem::*;
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    for entry in entries.flatten() {
        *budget -= 1;
        if *budget < 0 { return; }
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if depth < SHORTCUTS_MAX_DEPTH && !kind.is_symlink() { read_shortcuts(&path, depth + 1, budget, found); }
            continue;
        }
        if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")) { continue; }
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let Ok(store) = (unsafe { SHGetPropertyStoreFromParsingName::<_,_,IPropertyStore>(PCWSTR(wide.as_ptr()), None, GPS_DEFAULT) }) else { continue };
        let (id_key, clsid_key) = (PROPERTYKEY { fmtid: AUMID_FORMAT, pid: 5 }, PROPERTYKEY { fmtid: AUMID_FORMAT, pid: 26 });
        // PROPVARIANT's Display converts to text (a CLSID comes out as "{...}"); it frees itself.
        let (Ok(id), Ok(clsid)) = (unsafe { store.GetValue(&id_key) }, unsafe { store.GetValue(&clsid_key) }) else { continue };
        let (id, clsid) = (id.to_string(), clsid.to_string());
        if !id.is_empty() && windows::core::GUID::try_from(clsid.trim_matches(['{','}'])).is_ok() { found.push((id, clsid)); }
    }
}

/// An unpackaged app's notification activator (a CLSID): its registration's CustomActivator, else
/// the ToastActivatorCLSID of its Start menu shortcut; the shortcuts are read again after a few minutes.
fn unpackaged_activator(app_id: &str) -> Option<String> {
    if let Some(value) = registered_value(app_id, w!("CustomActivator")) {
        if windows::core::GUID::try_from(value.trim_matches(['{','}'])).is_ok() { return Some(value); }
    }
    let find = |list: &[(String,String)]| list.iter().find(|(id,_)| id.eq_ignore_ascii_case(app_id)).map(|(_,clsid)| clsid.clone());
    let mut cache = SHORTCUT_ACTIVATORS.lock().ok()?;
    if let Some((_, list)) = cache.as_ref() { if let Some(found) = find(list) { return Some(found); } }
    if cache.as_ref().is_some_and(|(at, _)| at.elapsed() < SHORTCUTS_REFRESH) { return None; }
    let mut found = Vec::new();
    let mut budget = SHORTCUTS_MAX_ENTRIES;
    unsafe {
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        for folder in [FOLDERID_Programs, FOLDERID_CommonPrograms] {
            if let Ok(path) = SHGetKnownFolderPath(&folder, KF_FLAG_DEFAULT, None) {
                let text = path.to_string().unwrap_or_default();
                CoTaskMemFree(Some(path.0.cast()));
                read_shortcuts(std::path::Path::new(&text), 0, &mut budget, &mut found);
            }
        }
        if com.is_ok() { CoUninitialize(); }
    }
    let result = find(&found);
    *cache = Some((std::time::Instant::now(), found));
    result
}

/// The mod's ActivateToast/ToastActivationThreadProc: the app's INotificationActivationCallback,
/// out of process, gets the toast's arguments as if its notification had been clicked.
/// With `input`: a reply, the text typed for that input id (NOTIFICATION_USER_INPUT_DATA).
pub(super) fn activate(app_id: &str, launch: &str, input: Option<(&str,&str)>) -> Result<()> {
    use windows::Win32::UI::Notifications::{INotificationActivationCallback, NOTIFICATION_USER_INPUT_DATA};
    let packaged = unsafe { package_folder(app_id) }
        .and_then(|folder| std::fs::read_to_string(std::path::Path::new(&folder).join("AppxManifest.xml")).ok())
        .and_then(|manifest| super::toast_activator(&manifest).map(str::to_owned));
    // Not packaged: its registration's CustomActivator, or its Start menu shortcut's ToastActivatorCLSID.
    let clsid = packaged.or_else(|| unpackaged_activator(app_id)).context("app sem ativador de avisos")?;
    let clsid = windows::core::GUID::try_from(clsid.trim_matches(['{','}'])).map_err(|_| anyhow!("CLSID inválido"))?;
    unsafe {
        CoInitializeEx(None,COINIT_MULTITHREADED).ok()?;
        // The app (or the page it opens) may come to the front.
        let _ = AllowSetForegroundWindow(ASFW_ANY);
        let result = (|| -> Result<()> {
            let callback: INotificationActivationCallback = CoCreateInstance(&clsid,None,CLSCTX_LOCAL_SERVER)?;
            let input = input.map(|(key,value)| (windows::core::HSTRING::from(key),windows::core::HSTRING::from(value)));
            let data: Vec<_> = input.iter().map(|(key,value)| NOTIFICATION_USER_INPUT_DATA { Key: PCWSTR(key.as_ptr()), Value: PCWSTR(value.as_ptr()) }).collect();
            callback.Activate(&windows::core::HSTRING::from(app_id),&windows::core::HSTRING::from(launch),&data)?;
            Ok(())
        })();
        CoUninitialize();
        result
    }
}

struct EventHandle(HANDLE);
impl Drop for EventHandle {
    fn drop(&mut self) { unsafe { let _ = CloseHandle(self.0); } }
}

type Unsubscribe = unsafe extern "system" fn(Raw) -> i32;
struct WnfSignal { event: EventHandle, token: Raw, unsubscribe: Unsubscribe, _library: Library }
impl Drop for WnfSignal {
    fn drop(&mut self) { unsafe { (self.unsubscribe)(self.token); } }
}
// WNF_USER_CALLBACK(StateName, ChangeStamp, TypeId, CallbackContext, Buffer, Length): the context is 4th.
unsafe extern "system" fn changed(_: u64, _: u32, _: Raw, context: Raw, _: *const c_void, _: u32) -> i32 {
    let _ = SetEvent(HANDLE(context));
    0
}
impl WnfSignal {
    unsafe fn new() -> Result<Self> {
        let library = Library::load(w!("ntdll.dll"))?;
        type Subscribe = unsafe extern "system" fn(*mut Raw,u64,u32,Raw,Raw,*const c_void,u32,u32) -> i32;
        type Query = unsafe extern "system" fn(*const u64,*const c_void,*const c_void,*mut u32,Raw,*mut u32) -> i32;
        let subscribe: Subscribe = library.get(b"RtlSubscribeWnfStateChangeNotification\0")?;
        let query: Query = library.get(b"NtQueryWnfStateData\0")?;
        let unsubscribe: Unsubscribe = library.get(b"RtlUnsubscribeWnfStateChangeNotification\0")?;
        let event = EventHandle(CreateEventW(None,false,false,None)?);
        let state = 0x0D83063EA3BC1035u64;
        let mut stamp = 0;
        let mut buffer = [0u8;64];
        let mut size = buffer.len() as u32;
        query(&state,std::ptr::null(),std::ptr::null(),&mut stamp,buffer.as_mut_ptr().cast(),&mut size);
        let mut token = std::ptr::null_mut();
        if subscribe(&mut token,state,stamp,changed as *const () as Raw,event.0.0,std::ptr::null(),0,0) < 0 {
            bail!("WNF de notificações indisponível");
        }
        Ok(Self { event,token,unsubscribe,_library:library })
    }
}

enum Signal { Wnf(WnfSignal), Directory(HANDLE) }
impl Drop for Signal {
    fn drop(&mut self) {
        if let Self::Directory(handle) = self { unsafe { let _ = FindCloseChangeNotification(*handle); } }
    }
}
impl Signal {
    unsafe fn new(directory: &std::path::Path) -> Result<Self> {
        if let Ok(wnf) = WnfSignal::new() { return Ok(Self::Wnf(wnf)); }
        // Supported event-driven fallback on builds where the mod's undocumented WNF is absent.
        let wide: Vec<_> = directory.as_os_str().encode_wide().chain(Some(0)).collect();
        Ok(Self::Directory(FindFirstChangeNotificationW(PCWSTR(wide.as_ptr()),false,
            FILE_NOTIFY_CHANGE_LAST_WRITE | FILE_NOTIFY_CHANGE_FILE_NAME)?))
    }
    unsafe fn wait(&self) -> Result<()> {
        let handle = match self { Self::Wnf(s) => s.event.0, Self::Directory(h) => *h };
        if WaitForSingleObject(handle,INFINITE) != WAIT_OBJECT_0 { bail!("Monitor de notificações interrompido"); }
        if let Self::Directory(h) = self { FindNextChangeNotification(*h)?; }
        Ok(())
    }
}

fn publish_batch(send: &SendEvent, icons: Option<&crate::notification_icons::Resolver>,
    items: Vec<Notification>, initial: bool, present: Vec<i64>, badged: Vec<String>) {
    let apps: std::collections::HashSet<String> = items.iter().map(|n| n.app_id.clone()).collect();
    send(NotificationEvent::Batch { items, initial, present, badged });
    if let Some(icons) = icons { for app in apps { icons.request(&app); } }
}

unsafe fn watch(send: &SendEvent, cursor: &mut ReadCursor, icons: Option<&crate::notification_icons::Resolver>) -> Result<()> {
    let directory = std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA ausente")?)
        .join("Microsoft/Windows/Notifications");
    let path = directory.join("wpndatabase.db");
    let path = CString::new(path.to_str().context("Caminho de notificações inválido")?)?;
    let sqlite = Sqlite::load()?;
    // Subscribe before the initial snapshot: arrivals during startup must still wake this worker.
    let signal = Signal::new(&directory)?;
    let mut names = HashMap::new();
    let (items,initial,present,badged) = sqlite.read(&path,cursor,&mut names)?;
    let mut last = (present.clone(),badged.clone());
    send(NotificationEvent::Status(None));
    publish_batch(send, icons, items, initial, present, badged);
    loop {
        signal.wait()?;
        // Like the mod: WNF can precede the DB write (notably WhatsApp). No timer when idle.
        for delay in [150,350,700,1500] {
            std::thread::sleep(Duration::from_millis(delay));
            let (items,initial,present,badged) = sqlite.read(&path,cursor,&mut names)?;
            let any = !items.is_empty();
            // Removals (read in the app, dismissed in Windows) and badge changes wake WNF too.
            if any || initial || (&present,&badged) != (&last.0,&last.1) {
                tracing::info!("avisos do Windows: novos {:?}, {} no banco, badge em {badged:?}",
                    items.iter().map(|n| n.id).collect::<Vec<_>>(), present.len());
                last = (present.clone(),badged.clone());
                publish_batch(send, icons, items, initial, present, badged);
            }
            if any { break; }
        }
    }
}

pub(super) fn spawn(send: SendEvent, win_n: bool, icons: bool) {
    if win_n {
        configure_shortcuts(Some(ShortcutAction::OpenNotifications), None, send.clone());
    }
    std::thread::spawn(move || unsafe {
        if let Err(err) = CoInitializeEx(None,COINIT_MULTITHREADED).ok() {
            send(NotificationEvent::Status(Some(format!("Notificações: {err}")))); return;
        }
        let icons = icons.then(|| crate::notification_icons::Resolver::new(send.clone()));
        let mut cursor = ReadCursor::default();
        loop {
            if let Err(err) = watch(&send,&mut cursor,icons.as_ref()) {
                send(NotificationEvent::Status(Some(format!("Notificações indisponíveis: {err}"))));
                // Fault recovery only; healthy workers wait indefinitely on WNF/directory events.
                std::thread::sleep(Duration::from_secs(30));
            }
        }
    });
}

struct ShortcutConfig {
    n: Option<ShortcutAction>,
    j: Option<ShortcutAction>,
    send: Option<SendEvent>,
    running: bool,
}

static SHORTCUTS: Mutex<ShortcutConfig> = Mutex::new(ShortcutConfig {
    n: None,
    j: None,
    send: None,
    running: false,
});

thread_local! {
    static KEY_PRESSES: std::cell::RefCell<ShortcutState> = Default::default();
}

pub(super) fn configure_shortcuts(
    win_n: Option<ShortcutAction>,
    win_j: Option<ShortcutAction>,
    send: SendEvent,
) {
    let active = win_n.is_some() || win_j.is_some();
    let (start, previous_send) = {
        let mut config = SHORTCUTS.lock().unwrap_or_else(|err| err.into_inner());
        config.n = win_n;
        config.j = win_j;
        let previous_send = std::mem::replace(&mut config.send, active.then_some(send));
        let start = active && !config.running;
        config.running |= start;
        (start, previous_send)
    };
    drop(previous_send);
    if start {
        if let Err(err) = std::thread::Builder::new()
            .name("cimori-shortcuts".into())
            .spawn(|| {
                let result = unsafe { shortcuts() };
                SHORTCUTS.lock().unwrap_or_else(|err| err.into_inner()).running = false;
                if let Err(err) = result {
                    tracing::warn!("Atalhos Win+N / Win+J indisponíveis: {err:#}");
                }
            })
        {
            SHORTCUTS.lock().unwrap_or_else(|err| err.into_inner()).running = false;
            tracing::warn!("Atalhos Win+N / Win+J indisponíveis: {err}");
        }
    }
}

const SHORTCUT_ACTION: u32 = WM_APP + 1;

unsafe extern "system" fn keyboard(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code != HC_ACTION as i32 {
        return CallNextHookEx(None, code, message, data);
    }
    let key = &*(data.0 as *const KBDLLHOOKSTRUCT);
    if key.flags.contains(LLKHF_INJECTED) || !matches!(key.vkCode, 0x4e | 0x4a) {
        return CallNextHookEx(None, code, message, data);
    }
    let down = message.0 == WM_KEYDOWN as usize || message.0 == WM_SYSKEYDOWN as usize;
    let action = if down {
        let config = SHORTCUTS.lock().unwrap_or_else(|err| err.into_inner());
        if key.vkCode == 0x4e { config.n } else { config.j }
    } else {
        None
    };
    // The modifier events have already been delivered when this letter arrives.
    let held = |vk: VIRTUAL_KEY| GetAsyncKeyState(vk.0 as i32) < 0;
    let win_only = action.is_some() && (held(VK_LWIN) || held(VK_RWIN))
        && !held(VK_CONTROL) && !held(VK_MENU) && !held(VK_SHIFT);
    let disposition = KEY_PRESSES.with(|state| {
        state.borrow_mut().route(key.vkCode, down, false, win_only, action)
    });
    match disposition {
        KeyDisposition::Pass => {}
        KeyDisposition::Swallow => return LRESULT(1),
        KeyDisposition::Trigger(action) => {
            // Prevent the Start menu on Win release, as the mod's PressUnusedKey does.
            let mut dummy = [INPUT::default(); 2];
            for input in &mut dummy {
                input.r#type = INPUT_KEYBOARD;
                input.Anonymous.ki.wVk = VIRTUAL_KEY(0xff);
            }
            dummy[1].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
            let action = match action {
                ShortcutAction::OpenNotifications => WPARAM(1),
                ShortcutAction::ToggleMinimize => WPARAM(2),
            };
            if SendInput(&dummy, std::mem::size_of::<INPUT>() as i32) == 2
                && PostThreadMessageW(GetCurrentThreadId(), SHORTCUT_ACTION, action, LPARAM(0)).is_ok()
            {
                return LRESULT(1);
            }
            // UIPI can prevent SendInput for an elevated foreground app.
            // A failed interception leaves the original Windows shortcut in place.
            KEY_PRESSES.with(|state| state.borrow_mut().pass_press(key.vkCode));
        }
    }
    CallNextHookEx(None, code, message, data)
}

unsafe fn shortcuts() -> Result<()> {
    let mut message = MSG::default();
    let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
    let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), GetModuleHandleW(None)?, 0)?;
    let result = loop {
        let status = GetMessageW(&mut message, None, 0, 0).0;
        if status == -1 {
            break Err(anyhow!("Loop dos atalhos interrompido"));
        }
        if status == 0 {
            break Ok(());
        }
        if message.message == SHORTCUT_ACTION {
            let send = SHORTCUTS.lock().unwrap_or_else(|err| err.into_inner()).send.clone();
            if let Some(send) = send {
                let event = match message.wParam.0 {
                    1 => NotificationEvent::Open,
                    2 => NotificationEvent::Minimize,
                    _ => continue,
                };
                send(event);
            }
        }
    };
    let _ = UnhookWindowsHookEx(hook);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wnf_callback_signals_the_event_passed_as_its_context() {
        unsafe {
            let event = EventHandle(CreateEventW(None,false,false,None).unwrap());
            // WNF_USER_CALLBACK(StateName, ChangeStamp, TypeId, CallbackContext, Buffer, Length).
            changed(0, 0, std::ptr::null_mut(), event.0.0, std::ptr::null(), 0);
            assert_eq!(WaitForSingleObject(event.0, 0), WAIT_OBJECT_0);
        }
    }

    #[test]
    fn the_start_menu_scan_runs_and_finds_only_valid_activators() {
        let mut found = Vec::new();
        let mut budget = SHORTCUTS_MAX_ENTRIES;
        let start = std::path::PathBuf::from(std::env::var_os("APPDATA").unwrap()).join("Microsoft/Windows/Start Menu/Programs");
        read_shortcuts(&start, 0, &mut budget, &mut found);
        eprintln!("{} shortcuts with an activator", found.len());
        assert!(found.iter().all(|(id, clsid)| !id.is_empty() && windows::core::GUID::try_from(clsid.trim_matches(['{','}'])).is_ok()));
    }
}

/// The text on the clipboard (CF_UNICODETEXT), for Ctrl+V in a reply; empty without one.
pub(super) fn clipboard_text() -> String {
    use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    unsafe {
        if OpenClipboard(None).is_err() { return String::new(); }
        let mut text = String::new();
        if let Ok(data) = GetClipboardData(13) {
            let global = HGLOBAL(data.0);
            let ptr = GlobalLock(global) as *const u16;
            if !ptr.is_null() {
                text = PCWSTR(ptr).to_string().unwrap_or_default().chars().take(2000).collect();
                let _ = GlobalUnlock(global);
            }
        }
        let _ = CloseClipboard();
        text
    }
}
