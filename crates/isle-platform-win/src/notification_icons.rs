// SPDX-License-Identifier: GPL-3.0
// Adapted from Mac Island & Dock v3.1.1 by caliberda: LoadAppIconBitmap/RegisteredAppPicture.
// Shell objects stay on a dedicated worker; notification receipt never waits for icon extraction.
use crate::notifications::NotificationEvent;
use isle_core::artwork::Artwork;
use std::{
    collections::HashMap,
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};

/// Sender-specific AUMID first, then its package application (e.g. Phone Link sub-senders).
pub fn app_candidates(app: &str) -> Vec<String> {
    if app.is_empty() || app.len() > 4096 || app.contains('\0') || app.contains(['\\', '/']) {
        return vec![];
    }
    let mut ids = vec![app.to_owned()];
    if let Some((package, entry)) = app.split_once('!') {
        if !package.is_empty() && !entry.eq_ignore_ascii_case("App") {
            ids.push(format!("{package}!App"));
        }
    }
    ids
}
/// Pixels to ask Windows for the app icon: its biggest slot (32 logical px) at the screen scale,
/// so the renderer draws it (almost) pixel for pixel instead of shrinking a 128 px bitmap.
pub fn icon_px_for_dpi(dpi: u32) -> u32 {
    ((32 * dpi.clamp(96, 384) + 48) / 96).clamp(32, 128)
}
pub fn local_path_shape(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() > 3
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && b[2] == b'\\'
        && !path.contains('\0')
}

#[cfg(windows)]
/// Resolves a sender picture on the calling thread, which must have initialized COM.
pub fn load(app: &str) -> anyhow::Result<Artwork> {
    native::load(app, icon_px_for_dpi(unsafe { windows::Win32::UI::HiDpi::GetDpiForSystem() }))
}

type Send = Arc<dyn Fn(NotificationEvent) + std::marker::Send + Sync>;
struct Entry {
    image: Option<Arc<Artwork>>,
    retry: Option<Instant>,
}
pub(crate) struct Resolver {
    queue: mpsc::SyncSender<String>,
    cache: Arc<Mutex<HashMap<String, Entry>>>,
    send: Send,
}
impl Resolver {
    pub(crate) fn new(send: Send) -> Self {
        let (tx, rx) = mpsc::sync_channel::<String>(128);
        let cache = Arc::new(Mutex::new(HashMap::<String, Entry>::new()));
        let worker_cache = cache.clone();
        let callback = send.clone();
        let worker = std::thread::Builder::new()
            .name("cimori-notification-icons".into())
            .spawn(move || {
                #[cfg(windows)]
                let _com = match native::Apartment::new() {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!("Ícones dos avisos indisponíveis: {e}");
                        return;
                    }
                };
                while let Ok(id) = rx.recv() {
                    #[cfg(windows)]
                    let image = load(&id).ok().map(Arc::new);
                    #[cfg(not(windows))]
                    let image: Option<Arc<Artwork>> = None;
                    {
                        let mut map = worker_cache
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        if map.len() >= 128 && !map.contains_key(&id.to_lowercase()) {
                            map.clear();
                        }
                        map.insert(
                            id.to_lowercase(),
                            Entry {
                                image: image.clone(),
                                retry: if image.is_some() {
                                    None
                                } else {
                                    Some(Instant::now() + Duration::from_secs(300))
                                },
                            },
                        );
                    }
                    if let Some(image) = image {
                        callback(NotificationEvent::AppIcon { app_id: id, image });
                    }
                }
            });
        if let Err(error) = worker {
            tracing::warn!("Notification icon worker unavailable: {error}");
        }
        Self {
            queue: tx,
            cache,
            send,
        }
    }
    /// Call only after publishing the batch: a cached result is delivered after the items exist.
    pub(crate) fn request(&self, id: &str) {
        if id.is_empty() || id.len() > 4096 || id.contains('\0') {
            return;
        }
        let key = id.to_lowercase();
        let mut cache = self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(entry) = cache.get(&key) {
            if let Some(image) = &entry.image {
                let image = image.clone();
                drop(cache);
                (self.send)(NotificationEvent::AppIcon {
                    app_id: id.into(),
                    image,
                });
                return;
            }
            if entry.retry.is_none_or(|at| Instant::now() < at) {
                return;
            }
        }
        if cache.len() >= 128 {
            cache.clear();
        }
        cache.insert(
            key.clone(),
            Entry {
                image: None,
                retry: None,
            },
        );
        drop(cache);
        if self.queue.try_send(id.into()).is_err() {
            self.cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&key);
        }
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use windows::{
        core::{w, Interface, PCWSTR, PWSTR},
        Win32::{
            Foundation::*,
            Graphics::{Gdi::*, Imaging::*},
            Storage::FileSystem::*,
            System::{
                Com::*, Diagnostics::ToolHelp::*, Registry::*, Threading::*,
                WindowsProgramming::DRIVE_REMOTE,
            },
            UI::Shell::*,
        },
    };
    pub(super) struct Apartment;
    impl Apartment {
        pub(super) fn new() -> anyhow::Result<Self> {
            unsafe {
                CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            }
            Ok(Self)
        }
    }
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() }
        }
    }
    struct Bitmap(HBITMAP);
    impl Drop for Bitmap {
        fn drop(&mut self) {
            unsafe {
                let _ = DeleteObject(self.0);
            }
        }
    }
    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    struct RegistryKey(HKEY);
    impl Drop for RegistryKey {
        fn drop(&mut self) {
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    fn local(path: &str) -> bool {
        if !local_path_shape(path) {
            return false;
        }
        let root = wide(&path[..3]);
        unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) != DRIVE_REMOTE }
    }
    unsafe fn pixels(
        source: &IWICBitmapSource,
        size: u32,
        factory: &IWICImagingFactory,
    ) -> anyhow::Result<Artwork> {
        unsafe {
            let (mut w, mut h) = (0, 0);
            source.GetSize(&mut w, &mut h)?;
            anyhow::ensure!(
                isle_core::artwork::normalized_size(w, h).is_some(),
                "invalid sender icon size"
            );
            let scale = (size as f64 / w.max(h) as f64).min(1.);
            let (dw, dh) = (
                (w as f64 * scale).round().max(1.) as u32,
                (h as f64 * scale).round().max(1.) as u32,
            );
            let scaler = factory.CreateBitmapScaler()?;
            scaler.Initialize(source, dw, dh, WICBitmapInterpolationModeFant)?;
            let converter = factory.CreateFormatConverter()?;
            converter.Initialize(
                &scaler,
                &GUID_WICPixelFormat32bppRGBA,
                WICBitmapDitherTypeNone,
                None,
                0.,
                WICBitmapPaletteTypeCustom,
            )?;
            let mut rgba = vec![0; dw as usize * dh as usize * 4];
            converter.CopyPixels(std::ptr::null(), dw * 4, &mut rgba)?;
            Artwork::new(dw, dh, rgba).ok_or_else(|| anyhow::anyhow!("invalid sender icon pixels"))
        }
    }
    unsafe fn from_shell(
        shell: IShellItemImageFactory,
        wic: &IWICImagingFactory,
        px: u32,
    ) -> anyhow::Result<Artwork> {
        unsafe {
            // Exactly `px` (no BIGGERSIZEOK): the shell picks the closest resource of the icon and
            // scales it once, instead of us shrinking a 128 px bitmap again in the renderer.
            let bitmap = Bitmap(shell.GetImage(
                SIZE { cx: px as i32, cy: px as i32 },
                SIIGBF_ICONONLY,
            )?);
            let source = wic.CreateBitmapFromHBITMAP(
                bitmap.0,
                HPALETTE::default(),
                WICBitmapUsePremultipliedAlpha,
            )?;
            pixels(&source.cast()?, px, wic)
        }
    }
    fn running_program(app: &str) -> Option<String> {
        unsafe {
            if app.contains(['\\', '/', '!']) {
                return None;
            }
            let snapshot = Handle(CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?);
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            if Process32FirstW(snapshot.0, &mut entry).is_err() {
                return None;
            }
            loop {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
                if name.eq_ignore_ascii_case(app)
                    || name.eq_ignore_ascii_case(&format!("{app}.exe"))
                {
                    if let Ok(raw) = OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION,
                        false,
                        entry.th32ProcessID,
                    ) {
                        let p = Handle(raw);
                        let mut path = [0u16; 32768];
                        let mut len = path.len() as u32;
                        if QueryFullProcessImageNameW(
                            p.0,
                            PROCESS_NAME_WIN32,
                            PWSTR(path.as_mut_ptr()),
                            &mut len,
                        )
                        .is_ok()
                        {
                            let path = String::from_utf16_lossy(&path[..len as usize]);
                            if local(&path) {
                                return Some(path);
                            }
                        }
                    }
                }
                if Process32NextW(snapshot.0, &mut entry).is_err() {
                    break;
                }
            }
            None
        }
    }
    fn registered_picture(app: &str) -> Option<String> {
        unsafe {
            let key = wide(&format!("Software\\Classes\\AppUserModelId\\{app}"));
            for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
                let mut handle = HKEY::default();
                if RegOpenKeyExW(hive, PCWSTR(key.as_ptr()), 0, KEY_QUERY_VALUE, &mut handle)
                    != ERROR_SUCCESS
                {
                    continue;
                }
                let registered = RegistryKey(handle);
                let mut data = [0u16; 4096];
                let mut bytes = (data.len() * 2) as u32;
                let status = RegGetValueW(
                    registered.0,
                    None,
                    w!("IconUri"),
                    RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
                    None,
                    Some(data.as_mut_ptr().cast()),
                    Some(&mut bytes),
                );
                if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
                    let end = data.iter().position(|c| *c == 0).unwrap_or(data.len());
                    let mut path = String::from_utf16_lossy(&data[..end]);
                    // Same fallback as Mac Island for registered Windows senders without IconUri.
                    if path.is_empty() {
                        path="ms-resource://Windows.UI.ShellCommon/Files/Images/DefaultSystemNotification.png".into();
                    }
                    if path.starts_with("ms-resource://") {
                        let resource = wide(&format!("@{{Windows?{path}}}"));
                        let mut resolved = [0u16; 4096];
                        if SHLoadIndirectString(PCWSTR(resource.as_ptr()), &mut resolved, None)
                            .is_ok()
                        {
                            path = String::from_utf16_lossy(
                                &resolved[..resolved
                                    .iter()
                                    .position(|c| *c == 0)
                                    .unwrap_or(resolved.len())],
                            );
                        }
                    }
                    if local(&path) {
                        return Some(path);
                    }
                }
            }
            None
        }
    }
    pub(super) fn load(app: &str, px: u32) -> anyhow::Result<Artwork> {
        unsafe {
            anyhow::ensure!(
                !app.is_empty() && app.len() <= 4096 && !app.contains('\0'),
                "invalid sender identity"
            );
            let wic: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            for id in app_candidates(app) {
                let id = wide(&id);
                if let Ok(shell) = SHCreateItemInKnownFolder::<_, IShellItemImageFactory>(
                    &FOLDERID_AppsFolder,
                    KF_FLAG_DEFAULT,
                    PCWSTR(id.as_ptr()),
                ) {
                    if let Ok(image) = from_shell(shell, &wic, px) {
                        return Ok(image);
                    }
                }
            }
            let program = if local(app) {
                Some(app.to_owned())
            } else {
                running_program(app)
            };
            if let Some(path) = program {
                let p = wide(&path);
                if let Ok(shell) = SHCreateItemFromParsingName::<_, _, IShellItemImageFactory>(
                    PCWSTR(p.as_ptr()),
                    None,
                ) {
                    if let Ok(image) = from_shell(shell, &wic, px) {
                        return Ok(image);
                    }
                }
            }
            if let Some(path) = registered_picture(app) {
                use std::io::Read;
                let mut bytes = Vec::new();
                std::fs::File::open(path)?
                    .take(isle_core::artwork::MAX_ENCODED + 1)
                    .read_to_end(&mut bytes)?;
                let image = crate::artwork::decode(&bytes)?;
                let source = wic.CreateBitmapFromMemory(
                    image.width,
                    image.height,
                    &GUID_WICPixelFormat32bppRGBA,
                    image.width * 4,
                    &image.rgba,
                )?;
                return pixels(&source.cast()?, px, &wic);
            }
            anyhow::bail!("sender icon unavailable")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn icon_size_follows_the_screen_scale() {
        assert_eq!(icon_px_for_dpi(96), 32);
        assert_eq!(icon_px_for_dpi(120), 40);
        assert_eq!(icon_px_for_dpi(144), 48);
        assert_eq!(icon_px_for_dpi(192), 64);
        assert_eq!(icon_px_for_dpi(0), 32);
        assert_eq!(icon_px_for_dpi(u32::MAX), 128);
    }
    #[test]
    fn candidates_keep_sender_identity_and_use_package_fallback() {
        assert_eq!(
            app_candidates("Package!PhoneSender"),
            ["Package!PhoneSender", "Package!App"]
        );
        assert_eq!(app_candidates("Package!App"), ["Package!App"]);
        assert!(app_candidates("bad\0id").is_empty());
        assert!(local_path_shape("C:\\Apps\\Chat.exe"));
        assert!(!local_path_shape("\\\\server\\app.exe"));
        assert!(!local_path_shape("https://site/icon.png"));
    }
}
