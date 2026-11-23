//! Windows adapters for the Isle clipboard history and file shelf, and Cimori's devices panels.
use super::{send, AppEvent, IsleApp, IsleConfig, PillMode, TabPage};
use isle_core::clipboard::{content_hash, Clip, ClipboardHistory};
use isle_core::phase8::{ClipRow, DownloadView, ShareView};
use isle_core::shelf::{Shelf, ShelfItem};
use isle_platform_win::{downloads, files, shelf_share, system_controls};
use isle_render::phase8_view::Phase8Action;
use std::{collections::HashMap, path::PathBuf, time::Duration};

#[cfg(windows)]
use isle_platform_win::clipboard::{self, ClipboardCapture, ClipboardListener, ClipboardPayload};

const MAX_CLIP_IMAGES_BYTES: usize = 64 * 1024 * 1024;
const DROP_HINT: &str = "Solte para pôr na prateleira";

#[derive(Default)]
pub(super) struct Phase8 {
    history: ClipboardHistory,
    clipboard_search_window: Option<usize>,
    images: HashMap<String, std::sync::Arc<Vec<u8>>>,
    thumbnails: HashMap<String, std::sync::Arc<isle_core::artwork::Artwork>>,
    shelf_thumbnails: HashMap<String, std::sync::Arc<isle_core::artwork::Artwork>>,
    #[cfg(windows)]
    store: Option<clipboard::StoreWorker>,
    #[cfg(windows)]
    store_ready: bool,
    shelf: Shelf,
    usb: Vec<system_controls::UsbDriveInfo>,
    usb_loaded: bool,
    usb_running: bool,
    usb_again: bool,
    usb_generation: u64,
    downloads: Option<downloads::DownloadWatcher>,
    download_generation: u64,
    recent: Vec<downloads::RecentDownload>,
    /// Whether an island showed the Downloads tab when last drawn (its opening refreshes the recent files).
    downloads_tab: bool,
    share: Option<shelf_share::ShelfShare>,
    share_starting: bool,
    share_generation: u64,
    #[cfg(windows)]
    clipboard: Option<ClipboardListener>,
    #[cfg(windows)]
    clipboard_generation: u64,
    #[cfg(windows)]
    clipboard_sequence: Option<u32>,
    #[cfg(windows)]
    clipboard_reading: bool,
    #[cfg(windows)]
    clipboard_again: bool,
    /// The last copy notice and when, so an app copying the same thing twice shows it once.
    #[cfg(windows)]
    clipboard_shown: Option<(String, std::time::Instant)>,
    #[cfg(windows)]
    session_notifications: bool,
    #[cfg(windows)]
    registered_hotkeys: Vec<i32>,
}

pub(super) enum Event {
    #[cfg(windows)]
    Clipboard(u64, Option<ClipboardCapture>),
    #[cfg(windows)]
    Stored(clipboard::Stored),
    #[cfg(windows)]
    Thumbnail(String, Option<isle_core::artwork::Artwork>, bool),
    ShelfPicked(Option<PathBuf>),
    /// Files let go on the island (directories already left out).
    ShelfDropped(Vec<PathBuf>),
    Usb(u64, Vec<system_controls::UsbDriveInfo>),
    Download(u64, Option<downloads::DownloadProgress>),
    DownloadDetails(u64, downloads::DownloadSnapshot),
    RecentDownloads(u64, Vec<downloads::RecentDownload>),
    ShareStarted(u64, Result<shelf_share::ShelfShare, String>),
    ShareChanged(u64),
    Message(TabPage, String),
}

impl IsleApp {
    pub(super) fn apply_phase8_config(&mut self, old: Option<&IsleConfig>) {
        let m = self.config.modules;
        if !m.clipboard && self.phase8.clipboard_search_window.is_some() {
            self.close_clipboard_search();
            self.overlay.release_keyboard();
        }
        self.state.phase8.shelf_capacity = self.config.shelf_capacity as usize;
        self.state.phase8.clipboard_paste = self.config.clipboard_paste;
        #[cfg(windows)]
        if self.phase8.store.is_none() && (self.config.clipboard_persist || self.config.shelf_persist) {
            if let (Some(services), Some(folder)) = (self.services.clone(), std::env::var_os("APPDATA")) {
                self.phase8.store = Some(clipboard::StoreWorker::start(PathBuf::from(folder).join(isle_core::data_dir::NAME), true, true, move |stored| {
                    send(&services.tx, &services.waker, AppEvent::Phase8(Event::Stored(stored)));
                }));
            }
        }
        self.state.phase8.shelf_qr = m.shelf && self.config.shelf_qr;
        self.state.phase8.shelf_receive = self.config.shelf_receive;
        self.state.phase8.download_meter = m.downloads && self.config.download_meter;
        self.state.phase8.download_animation = self.config.download_animation;
        if old.is_some_and(|old| old.download_taskbar != self.config.download_taskbar) {
            self.phase8.downloads = None;
            self.phase8.download_generation += 1;
        }
        if old.is_none_or(|old| {
            old.modules.clipboard != m.clipboard
                || old.clipboard_notice != self.config.clipboard_notice
                || old.modules.shelf != m.shelf
                || old.ui.minimize_hotkey != self.config.ui.minimize_hotkey
                || old.ui.notices_hotkey != self.config.ui.notices_hotkey
                || old.win_n != self.config.win_n
                || old.modules.notifications != m.notifications
        }) {
            self.bind_phase8_windows();
        }
        if old.is_some_and(|old| old.clipboard_limit != self.config.clipboard_limit) {
            self.refresh_clipboard_rows();
        }
        if m.usb && old.is_none_or(|old| !old.modules.usb) {
            self.refresh_usb();
        } else if !m.usb && old.is_some_and(|old| old.modules.usb) {
            self.phase8.usb.clear();
            self.phase8.usb_generation += 1;
            self.phase8.usb_loaded = false;
            self.phase8.usb_running = false;
            self.phase8.usb_again = false;
            self.state.phase8.usb.clear();
            self.alerts.retain(|(_, _, tab)| *tab != Some(super::Opens::Tab(TabPage::Usb)));
        }
        if m.downloads && self.phase8.downloads.is_none() {
            if let Some(services) = self.services.clone() {
                let generation = self.phase8.download_generation;
                self.phase8.downloads = Some(downloads::subscribe_detailed(self.config.download_taskbar, move |value| {
                    send(
                        &services.tx,
                        &services.waker,
                        AppEvent::Phase8(Event::DownloadDetails(generation, value)),
                    );
                }));
            }
        } else if !m.downloads && old.is_some_and(|old| old.modules.downloads) {
            self.phase8.downloads = None;
            self.phase8.download_generation += 1;
            self.state.phase8.downloads = None;
            self.state.phase8.download_rows.clear();
        }
        if !(m.downloads && self.config.download_recent) {
            self.phase8.recent.clear();
            self.state.phase8.download_recent.clear();
        } else if old.is_some_and(|old| !old.download_recent) {
            self.refresh_recent_downloads();
        }
        if !self.state.phase8.shelf_qr
            || old.is_some_and(|old| old.shelf_idle_minutes != self.config.shelf_idle_minutes)
            || old.is_some_and(|old| old.shelf_receive != self.config.shelf_receive)
        {
            self.stop_shelf_share();
        }
    }

    #[cfg(windows)]
    pub(super) fn unbind_phase8_windows(&mut self) {
        use windows::Win32::System::RemoteDesktop::WTSUnRegisterSessionNotification;
        use windows::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey;
        self.phase8.clipboard = None;
        self.phase8.clipboard_generation += 1;
        self.phase8.clipboard_sequence = None;
        self.phase8.clipboard_reading = false;
        self.phase8.clipboard_again = false;
        if let Some(hwnd) = self.overlay.native_window() {
            for id in self.phase8.registered_hotkeys.drain(..) {
                unsafe {
                    let _ = UnregisterHotKey(hwnd, id);
                }
            }
            if self.phase8.session_notifications {
                unsafe {
                    let _ = WTSUnRegisterSessionNotification(hwnd);
                }
                self.phase8.session_notifications = false;
            }
        }
    }

    #[cfg(not(windows))]
    pub(super) fn unbind_phase8_windows(&mut self) {}

    #[cfg(windows)]
    pub(super) fn bind_phase8_windows(&mut self) {
        use isle_core::config::Hotkey;
        use windows::Win32::System::RemoteDesktop::{
            WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
        };
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            RegisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
        };

        self.unbind_phase8_windows();
        let Some(hwnd) = self.overlay.native_window() else {
            return;
        };
        if self.config.modules.shelf {
            self.phase8.session_notifications =
                unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) }.is_ok();
        }
        if self.config.modules.clipboard || self.config.clipboard_notice {
            match ClipboardListener::register(hwnd) {
                Ok(listener) => self.phase8.clipboard = Some(listener),
                Err(_) => self.phase8_message(
                    TabPage::Clipboard,
                    "Não foi possível monitorar a área de transferência.".into(),
                ),
            }
        }
        use isle_platform_win::notifications::{self, ShortcutAction};
        let minimize = self.config.ui.minimize_hotkey;
        let notices = if self.config.win_n && self.config.modules.notifications {
            self.config.ui.notices_hotkey
        } else {
            Hotkey::None
        };
        if let Some(services) = self.services.clone() {
            let action = |key| {
                if key == minimize {
                    Some(ShortcutAction::ToggleMinimize)
                } else if key == notices {
                    Some(ShortcutAction::OpenNotifications)
                } else {
                    None
                }
            };
            notifications::configure_shortcuts(
                action(Hotkey::WinN),
                action(Hotkey::WinJ),
                move |event| {
                    send(
                        &services.tx,
                        &services.waker,
                        AppEvent::Notifications(event),
                    );
                },
            );
        }
        let shortcuts = [(1, minimize), (2, notices)];
        for (id, key) in shortcuts {
            let (modifiers, letter) = match key {
                Hotkey::None | Hotkey::WinJ | Hotkey::WinN => continue,
                Hotkey::CtrlAltJ => (MOD_CONTROL | MOD_ALT, b'J'),
                Hotkey::CtrlAltN => (MOD_CONTROL | MOD_ALT, b'N'),
            };
            match unsafe { RegisterHotKey(hwnd, id, modifiers | MOD_NOREPEAT, u32::from(letter)) } {
                Ok(()) => self.phase8.registered_hotkeys.push(id),
                Err(error) => tracing::warn!("atalho {key:?}: {error}"),
            }
        }
    }

    #[cfg(not(windows))]
    pub(super) fn bind_phase8_windows(&mut self) {}

    pub(super) fn handle_island_hotkey(&mut self, id: i32) {
        if id == 1 { self.islands[0].pinned = false; }
        match id {
            1 if self.islands[0].mode == PillMode::Minimized => {
                self.islands[0].manually_hidden = false;
                self.islands[0].mode = PillMode::CompactBar;
                self.islands[0].morph(super::COMPACT);
            }
            1 if matches!(
                self.islands[0].mode,
                PillMode::Expanded(_) | PillMode::CompactBar
            ) =>
            {
                self.islands[0].manually_hidden = true;
                self.islands[0].mode = PillMode::Minimized;
                self.islands[0].morph(super::LINE);
            }
            2 if self.config.modules.notifications => self.toggle_notifications(),
            _ => {}
        }
    }

    #[cfg(windows)]
    pub(super) fn queue_clipboard_capture(&mut self) {
        use windows::Win32::Foundation::HWND;

        if self.phase8.clipboard.is_none() {
            return;
        }
        if self.phase8.clipboard_reading {
            self.phase8.clipboard_again = true;
            return;
        }
        let (Some(hwnd), Some(services)) = (self.overlay.native_window(), self.services.clone())
        else {
            return;
        };
        let handle = hwnd.0 as isize;
        let generation = self.phase8.clipboard_generation;
        let ignored = self.config.clipboard_ignored_apps.clone();
        self.phase8.clipboard_reading = true;
        services.rt.spawn_blocking(move || {
            let capture = (0..4)
                .find_map(|attempt| {
                    if attempt > 0 {
                        std::thread::sleep(Duration::from_millis(15));
                    }
                    match clipboard::capture_ignoring(HWND(handle as *mut _), &ignored) {
                        Ok(value) => Some(value),
                        Err(_) => None,
                    }
                })
                .flatten();
            send(
                &services.tx,
                &services.waker,
                AppEvent::Phase8(Event::Clipboard(generation, capture)),
            );
        });
    }

    #[cfg(not(windows))]
    pub(super) fn queue_clipboard_capture(&mut self) {}

    #[cfg(windows)]
    /// Mac Island's OnClipboardChanged: a file's name (or how many), the text's start on one line,
    /// or "Imagem copiada" (not the Snipping Tool's, which has its own banner). Nothing is kept.
    fn show_copy_notice(&mut self, capture: &ClipboardCapture) {
        let Some(what) = copy_notice(&capture.payload, capture.screenshot) else {
            return;
        };
        let now = std::time::Instant::now();
        if self.phase8.clipboard_shown.as_ref().is_some_and(|(shown, at)| {
            *shown == what && now.duration_since(*at) < Duration::from_secs(1)
        }) {
            return;
        }
        self.phase8.clipboard_shown = Some((what.clone(), now));
        let main = &mut self.islands[0];
        let can_show = matches!(main.mode, PillMode::CompactBar | PillMode::Minimized)
            || matches!(&main.mode, PillMode::AlertBanner { title, .. } if title == "Copiado");
        if self.state.dnd_active || !can_show {
            return;
        }
        main.mode = PillMode::AlertBanner {
            title: "Copiado".into(),
            subtitle: what,
        };
        main.morph((360.0, 64.0, 20.0));
        self.banner_until = Some(now + Duration::from_secs(2));
    }

    fn capture_clip(&mut self, capture: ClipboardCapture) {
        if self.phase8.clipboard_sequence == Some(capture.sequence) {
            return;
        }
        self.phase8.clipboard_sequence = Some(capture.sequence);
        let clip = match capture.payload {
            ClipboardPayload::Text(text) => Clip::from_text(&text).ok(),
            ClipboardPayload::Files(uris) if self.config.clipboard_files => {
                Clip::from_uris(uris).ok()
            }
            ClipboardPayload::Image { png, width, height } if self.config.clipboard_images => {
                let hash = content_hash(&png);
                if !self.phase8.images.contains_key(&hash) {
                    let bytes = self.phase8.images.values().map(|png| png.len()).sum::<usize>();
                    if bytes + png.len() > MAX_CLIP_IMAGES_BYTES {
                        self.phase8_message(
                            TabPage::Clipboard,
                            "Limite de imagens do histórico atingido.".into(),
                        );
                        return;
                    }
                    self.phase8.images.insert(hash.clone(), std::sync::Arc::new(png));
                    if let Some(services) = self.services.clone() {
                        let bytes = self.phase8.images[&hash].clone();
                        let key = hash.clone();
                        services.rt.clone().spawn_blocking(move || {
                            let thumb = clipboard::thumbnail(&bytes);
                            send(&services.tx, &services.waker, AppEvent::Phase8(Event::Thumbnail(key, thumb, false)));
                        });
                    }
                }
                Some(Clip::Image {
                    hash,
                    width,
                    height,
                })
            }
            _ => None,
        };
        if let Some(clip) = clip {
            let removed = self
                .phase8
                .history
                .add(clip, "", self.state.now_ms)
                .unreferenced;
            for hash in removed {
                self.phase8.images.remove(&hash);
            }
            self.refresh_clipboard_rows();
            self.persist_phase8();
        }
    }

    fn refresh_clipboard_rows(&mut self) {
        self.phase8.thumbnails.retain(|hash, _| self.phase8.images.contains_key(hash));
        let limit = self.config.clipboard_limit as usize;
        let keep: std::collections::HashSet<_> = self
            .phase8
            .history
            .items()
            .iter()
            .filter(|item| !item.pinned)
            .take(limit)
            .map(|item| item.id)
            .collect();
        for hash in self
            .phase8
            .history
            .retain(|item| item.pinned || keep.contains(&item.id))
        {
            self.phase8.images.remove(&hash);
        }
        let view = &mut self.state.phase8;
        view.clipboard_has_recent = self.phase8.history.items().iter().any(|item| !item.pinned);
        let query = view.clipboard_query.trim().to_lowercase();
        view.clipboard = self
            .phase8
            .history
            .items()
            .iter()
            .filter(|item| {
                (!view.clipboard_pins_only || item.pinned)
                    && (query.is_empty()
                        || item.clip.contains(&query)
                        || (matches!(item.clip, Clip::Image { .. }) && "imagem".contains(&query)))
            })
            .map(|item| ClipRow {
                id: item.id,
                pinned: item.pinned,
                label: match &item.clip {
                    Clip::Image { width, height, .. } => format!("Imagem · {width} × {height}"),
                    clip => clip
                        .preview()
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                },
            })
            .collect();
        view.clipboard_page = view
            .clipboard_page
            .min(view.clipboard.len().saturating_sub(1) / isle_core::phase8::ROWS_PER_PAGE);
        view.clipboard_thumbnails = if self.config.clipboard_thumbnails { self.phase8.history.items().iter().filter_map(|item| {
            item.clip.image_hash().and_then(|hash| self.phase8.thumbnails.get(hash)).map(|thumb| (item.id, thumb.clone()))
        }).collect() } else { Default::default() };
    }

    pub(super) fn add_shelf_files(&mut self, paths: Vec<PathBuf>) {
        if !self.config.modules.shelf || paths.is_empty() {
            return;
        }
        let Some(services) = self.services.clone() else {
            return;
        };
        // Classify files before they reach the UI thread; removable and network paths can block.
        services.rt.spawn_blocking(move || {
            let files = paths.into_iter().filter(|path| path.is_file()).collect();
            send(&services.tx, &services.waker, AppEvent::Phase8(Event::ShelfDropped(files)));
        });
    }

    /// Port of Island::DragOverAt/DragGone (mac-island 3.1.1): files over island `window` (`over`) or gone.
    /// A closed pill says to let go; an open one shows it over its panel and stays open meanwhile.
    pub(super) fn files_over(&mut self, window: usize, over: bool) {
        self.files_over = if over { Some(window) } else { self.files_over.filter(|w| *w != window) };
        let Some(island) = self.islands.get_mut(window) else { return; };
        let hinted = matches!(&island.mode, PillMode::AlertBanner { subtitle, .. } if subtitle == DROP_HINT);
        if over && matches!(island.mode, PillMode::CompactBar | PillMode::Minimized) {
            island.mode = PillMode::AlertBanner { title: "Prateleira".into(), subtitle: DROP_HINT.into() };
            island.morph((360.0, 64.0, 20.0));
        } else if !over && hinted {
            island.mode = PillMode::CompactBar;
            island.morph(super::COMPACT);
        }
    }

    /// Island::DropAt's "Na prateleira: N" on the main island, when it's free.
    fn announce_shelf(&mut self) {
        let main = &mut self.islands[0];
        if self.state.dnd_active || !matches!(main.mode, PillMode::CompactBar | PillMode::Minimized) { return; }
        main.mode = PillMode::AlertBanner { title: "Prateleira".into(), subtitle: format!("Na prateleira: {}", self.phase8.shelf.items().len()) };
        main.morph((360.0, 64.0, 20.0));
        self.banner_until = Some(std::time::Instant::now() + Duration::from_secs(2));
    }

    #[cfg(windows)]
    fn park_shelf_file(&mut self, path: PathBuf) -> bool {
        self.park_shelf_file_received(path, false)
    }

    /// True when the file went on the shelf.
    #[cfg(windows)]
    fn park_shelf_file_received(&mut self, path: PathBuf, received: bool) -> bool {
        if !self.config.modules.shelf && !received {
            return false;
        }
        if !received { self.stop_shelf_share(); }
        let (Some(uri), Some(name)) = (clipboard::file_uri(&path), path.file_name()) else {
            return false;
        };
        let shelf = &mut self.phase8.shelf;
        // A completed upload must reach the shelf: when it is full, the oldest item makes room.
        while received && shelf.items().len() >= self.config.shelf_capacity as usize
            && !shelf.items().iter().any(|item| item.uri == uri)
        {
            let Some(oldest) = shelf.items().last().map(|item| item.uri.clone()) else { break };
            shelf.remove(&oldest);
        }
        if !received && shelf.items().len() >= self.config.shelf_capacity as usize
            && !shelf.items().iter().any(|item| item.uri == uri)
        {
            self.phase8_message(
                TabPage::Shelf,
                "A prateleira atingiu o limite de arquivos.".into(),
            );
            return false;
        }
        if shelf.add(ShelfItem {
            uri,
            name: name.to_string_lossy().into_owned(),
            added_at_ms: self.state.now_ms,
        }) {
            self.refresh_shelf();
            self.persist_phase8();
            if let Some(services) = self.services.clone() {
                let key = clipboard::file_uri(&path).unwrap_or_default();
                services.rt.clone().spawn_blocking(move || {
                    let thumb = clipboard::file_thumbnail(&path);
                    send(&services.tx, &services.waker, AppEvent::Phase8(Event::Thumbnail(key, thumb, true)));
                });
            }
            self.state.phase8.message = None;
            return true;
        }
        false
    }

    #[cfg(not(windows))]
    fn park_shelf_file(&mut self, _: PathBuf) -> bool { false }

    fn refresh_shelf(&mut self) {
        self.phase8.shelf_thumbnails.retain(|uri, _| self.phase8.shelf.items().iter().any(|item| &item.uri == uri));
        self.state.phase8.shelf = self
            .phase8
            .shelf
            .items()
            .iter()
            .map(|item| item.name.clone())
            .collect();
        self.state.phase8.shelf_thumbnails = self.phase8.shelf.items().iter().enumerate()
            .filter_map(|(i, item)| self.phase8.shelf_thumbnails.get(&item.uri).map(|thumb| (i, thumb.clone()))).collect();
    }

    fn persist_phase8(&self) {
        #[cfg(windows)]
        if self.phase8.store_ready {
            if let Some(store) = &self.phase8.store {
                store.save(clipboard::Stored {
                    history: self.phase8.history.clone(), shelf: self.phase8.shelf.clone(), images: self.phase8.images.clone(),
                    keep_clipboard: self.config.clipboard_persist, keep_shelf: self.config.shelf_persist, ..Default::default()
                });
            }
        }
    }

    /// Integration point for the UI's mouse-down/drag-threshold handler (Claude-owned app.rs).
    #[cfg(windows)]
    pub(super) fn drag_shelf_file(&mut self, index: usize) {
        let Some((hwnd, item)) = self.overlay.native_window().zip(self.phase8.shelf.items().get(index)) else { return; };
        let uri = item.uri.clone();
        let Some(path) = clipboard::file_path(&uri) else { return; };
        match clipboard::drag_out(hwnd, &path, self.config.shelf_drag_move) {
            Ok(true) => { self.phase8.shelf.remove(&uri); self.refresh_shelf(); self.persist_phase8(); }
            Ok(false) => {}
            Err(err) => self.phase8_message(TabPage::Shelf, format!("Não foi possível arrastar: {err}")),
        }
    }

    fn start_shelf_share(&mut self) {
        #[cfg(windows)]
        {
            if !self.state.phase8.shelf_qr
                || self.phase8.share_starting
                || self.phase8.share.is_some()
            {
                return;
            }
            let Some(services) = self.services.clone() else {
                return;
            };
            let paths = self
                .phase8
                .shelf
                .items()
                .iter()
                .filter_map(|item| clipboard::file_path(&item.uri))
                .collect::<Vec<_>>();
            if paths.is_empty() && !self.config.shelf_receive {
                return;
            }
            self.phase8.share_starting = true;
            self.phase8_message(TabPage::Shelf, "Preparando o compartilhamento…".into());
            let generation = self.phase8.share_generation;
            let idle = Duration::from_secs(u64::from(self.config.shelf_idle_minutes) * 60);
            let receive = self.config.shelf_receive;
            let capacity = self.config.shelf_capacity as usize;
            services.rt.clone().spawn_blocking(move || {
                let signals = services.clone();
                let result = (|| {
                    let address = shelf_share::local_address()?;
                    let receive = if receive { Some(std::env::var_os("APPDATA").map(PathBuf::from)
                        .ok_or_else(|| anyhow::anyhow!("A pasta de dados do usuário não está disponível."))?.join(isle_core::data_dir::NAME).join("received")) } else { None };
                    shelf_share::ShelfShare::start_receiving(address, paths, idle, shelf_share::ShareOptions {
                        receive, capacity, ..Default::default()
                    }, move || {
                        send(
                            &signals.tx,
                            &signals.waker,
                            AppEvent::Phase8(Event::ShareChanged(generation)),
                        );
                    })
                })()
                .map_err(|err| format!("Compartilhamento indisponível. {err}"));
                send(
                    &services.tx,
                    &services.waker,
                    AppEvent::Phase8(Event::ShareStarted(generation, result)),
                );
            });
        }
    }

    pub(super) fn shelf_share_in_progress(&self) -> bool {
        self.phase8.share_starting || self.phase8.share.is_some()
    }

    pub(super) fn stop_shelf_share(&mut self) {
        if let Some(share) = self.phase8.share.take() {
            let received = share.stop_and_drain();
            #[cfg(windows)]
            for path in received { self.park_shelf_file_received(path, true); }
            #[cfg(not(windows))]
            let _ = received;
        }
        self.phase8.share_generation += 1;
        self.phase8.share = None;
        self.phase8.share_starting = false;
        self.state.phase8.share = None;
    }

    fn refresh_share(&mut self) {
        #[cfg(windows)]
        if let Some(share) = self.phase8.share.as_ref() {
            let received = share.drain_received();
            for path in received { self.park_shelf_file_received(path, true); }
        }
        let Some(share) = self.phase8.share.as_ref() else {
            return;
        };
        if !share.is_active() {
            self.stop_shelf_share();
            self.phase8_message(TabPage::Shelf, "Compartilhamento encerrado.".into());
            return;
        }
        let pending_code = share.pending().map(|peer| peer.code);
        if let Some(view) = &mut self.state.phase8.share {
            view.pending_code = pending_code;
        } else {
            let qr = share.qr();
            let size = qr.width();
            let modules: Vec<_> = (0..size)
                .flat_map(|y| (0..size).map(move |x| qr.dark(x, y)))
                .collect();
            self.state.phase8.share = Some(ShareView {
                size,
                modules: modules.into(),
                pending_code,
            });
        }
    }

    pub(super) fn refresh_usb(&mut self) {
        if !self.config.modules.usb {
            return;
        }
        if self.phase8.usb_running {
            self.phase8.usb_again = true;
            return;
        }
        let Some(services) = self.services.clone() else {
            return;
        };
        self.phase8.usb_running = true;
        let generation = self.phase8.usb_generation;
        services.rt.spawn_blocking(move || {
            let drives = system_controls::list_usb_drives();
            send(
                &services.tx,
                &services.waker,
                AppEvent::Phase8(Event::Usb(generation, drives)),
            );
        });
    }

    /// The newest finished files of Downloads, read off the UI thread. Only when the tab opens or a
    /// download ends: nothing runs while the island is closed.
    pub(super) fn refresh_recent_downloads(&mut self) {
        if !self.config.modules.downloads || !self.config.download_recent { return; }
        let Some(services) = self.services.clone() else { return; };
        let generation = self.phase8.download_generation;
        services.rt.spawn_blocking(move || {
            let recent = downloads::recent_downloads(5);
            send(&services.tx, &services.waker, AppEvent::Phase8(Event::RecentDownloads(generation, recent)));
        });
    }

    /// Called as the islands are drawn: the Downloads tab just opened reads the recent files again.
    pub(super) fn watch_downloads_tab(&mut self) {
        let open = self.islands.iter().any(|i| i.mode == PillMode::Expanded(TabPage::Downloads));
        if open && !self.phase8.downloads_tab { self.refresh_recent_downloads(); }
        self.phase8.downloads_tab = open;
    }

    fn phase8_message(&mut self, tab: TabPage, message: String) {
        self.state.phase8.message = Some((tab, message));
    }

    #[cfg(windows)]
    fn restore_clipboard(&mut self, payload: ClipboardPayload, tab: TabPage) {
        use windows::Win32::Foundation::HWND;
        let (Some(hwnd), Some(services)) = (self.overlay.native_window(), self.services.clone())
        else {
            return;
        };
        let handle = hwnd.0 as isize;
        let paste = (tab == TabPage::Clipboard && self.config.clipboard_paste).then(|| clipboard::paste_target(hwnd)).flatten();
        services.rt.spawn_blocking(move || {
            let msg = if clipboard::restore(HWND(handle as *mut _), &payload).is_ok() {
                if let Some(target) = paste { let _ = clipboard::paste_into(target); }
                "Copiado para a área de transferência."
            } else {
                "Não foi possível copiar o item."
            };
            send(
                &services.tx,
                &services.waker,
                AppEvent::Phase8(Event::Message(tab, msg.into())),
            );
        });
    }

    fn copy_clip(&mut self, id: u64) {
        #[cfg(windows)]
        {
            let Some(item) = self.phase8.history.get(id) else {
                return;
            };
            let payload = match &item.clip {
                Clip::Text { text } | Clip::Link { text } => ClipboardPayload::Text(text.clone()),
                Clip::Files { uris } => ClipboardPayload::Files(uris.clone()),
                Clip::Image {
                    hash,
                    width,
                    height,
                } => {
                    let Some(png) = self.phase8.images.get(hash) else {
                        return;
                    };
                    ClipboardPayload::Image {
                        png: png.to_vec(),
                        width: *width,
                        height: *height,
                    }
                }
            };
            self.restore_clipboard(payload, TabPage::Clipboard);
        }
        #[cfg(not(windows))]
        let _ = id;
    }

    fn shelf_file_action(&mut self, index: usize, copy: bool) {
        #[cfg(windows)]
        {
            let Some(item) = self.phase8.shelf.items().get(index) else {
                return;
            };
            if copy {
                self.restore_clipboard(
                    ClipboardPayload::Files(vec![item.uri.clone()]),
                    TabPage::Shelf,
                );
            } else if let Some(path) = clipboard::file_path(&item.uri) {
                files::shell_open(&path.to_string_lossy());
            }
        }
        #[cfg(not(windows))]
        let _ = (index, copy);
    }

    pub(super) fn clipboard_search_window(&self) -> Option<usize> {
        self.phase8.clipboard_search_window
    }

    fn clipboard_search_on(&self, island: usize) -> bool {
        self.clipboard_search_window() == Some(island)
    }

    pub(super) fn close_clipboard_search(&mut self) {
        self.phase8.clipboard_search_window = None;
        self.state.phase8.clipboard_searching = false;
    }

    pub(super) fn sync_clipboard_search(&mut self) {
        if self.phase8.clipboard_search_window.is_some_and(|i| {
            self.islands
                .get(i)
                .is_none_or(|island| island.mode != PillMode::Expanded(TabPage::Clipboard))
        }) {
            self.close_clipboard_search();
            self.overlay.release_keyboard();
        }
    }

    pub(super) fn clipboard_search_input(&mut self, island: usize, ch: char) -> bool {
        if !self.clipboard_search_on(island) {
            return false;
        }
        let query = &mut self.state.phase8.clipboard_query;
        match ch {
            '\u{8}' => {
                query.pop();
            }
            '\u{1b}' => {
                query.clear();
                self.close_clipboard_search();
                self.overlay.release_keyboard();
            }
            '\r' => {
                self.close_clipboard_search();
                self.overlay.release_keyboard();
            }
            c if !c.is_control() && query.chars().count() < 80 => query.push(c),
            _ => return true,
        }
        self.state.phase8.clipboard_page = 0;
        self.refresh_clipboard_rows();
        true
    }

    pub(super) fn phase8_action(&mut self, island: usize, action: Phase8Action) {
        if matches!(action, Phase8Action::ShelfRow(_)) { return; }
        if action != Phase8Action::SearchClips && self.clipboard_search_on(island) {
            self.close_clipboard_search();
            self.overlay.release_keyboard();
        }
        use Phase8Action::*;
        self.state.phase8.message = None;
        match action {
            CopyClip(id) => self.copy_clip(id),
            PinClip(id) => {
                if let Some(item) = self.phase8.history.get(id) {
                    if self.phase8.history.set_pinned(id, !item.pinned).is_err() {
                        self.phase8_message(
                            TabPage::Clipboard,
                            "Limite de itens fixados atingido.".into(),
                        );
                    }
                    self.refresh_clipboard_rows();
                    self.persist_phase8();
                }
            }
            DeleteClip(id) => {
                for hash in self.phase8.history.remove(id).unwrap_or_default() {
                    self.phase8.images.remove(&hash);
                }
                self.refresh_clipboard_rows();
                self.persist_phase8();
            }
            ClearClips => {
                for hash in self.phase8.history.clear() {
                    self.phase8.images.remove(&hash);
                }
                self.refresh_clipboard_rows();
                self.persist_phase8();
            }
            ClipboardPage(page) => self.state.phase8.clipboard_page = page,
            SearchClips => {
                self.close_ai_input();
                self.close_reply();
                self.stop_typing_reminder();
                self.phase8.clipboard_search_window = Some(island);
                self.state.phase8.clipboard_searching = true;
                if !cfg!(test) {
                    let _ = self.overlay.take_keyboard(island);
                }
            }
            FilterPinnedClips => {
                self.state.phase8.clipboard_pins_only = !self.state.phase8.clipboard_pins_only;
                self.state.phase8.clipboard_page = 0;
                self.refresh_clipboard_rows();
            }
            ShelfPick => {
                if let Some(services) = self.services.clone() {
                    services.rt.spawn_blocking(move || {
                        let picked = files::pick_shelf_file();
                        send(
                            &services.tx,
                            &services.waker,
                            AppEvent::Phase8(Event::ShelfPicked(picked)),
                        );
                    });
                }
            }
            ShelfRow(_) => {}
            ShelfOpen(i) => self.shelf_file_action(i, false),
            ShelfCopy(i) => self.shelf_file_action(i, true),
            ShelfRemove(i) => {
                if let Some(item) = self.phase8.shelf.items().get(i) {
                    self.phase8.shelf.remove(&item.uri.clone());
                    self.stop_shelf_share();
                    self.refresh_shelf();
                    self.persist_phase8();
                }
            }
            ShareStart => self.start_shelf_share(),
            ShareStop => self.stop_shelf_share(),
            ShareApprove | ShareDeny => {
                if let Some((share, code)) = self.phase8.share.as_ref().zip(
                    self.state
                        .phase8
                        .share
                        .as_ref()
                        .and_then(|view| view.pending_code.as_deref()),
                ) {
                    if action == ShareApprove {
                        share.approve(code);
                    } else {
                        share.deny(code);
                    }
                }
                self.refresh_share();
            }
            UsbOpen(i) | UsbEject(i) => {
                let Some((drive, services)) = self.phase8.usb.get(i).zip(self.services.clone())
                else {
                    return;
                };
                let letter = drive.letter.clone();
                services.rt.spawn_blocking(move || {
                    let result = if matches!(action, UsbEject(_)) {
                        system_controls::eject_usb_drive(&letter)
                    } else {
                        system_controls::open_usb_drive(&letter)
                    };
                    if let Err(err) = result {
                        send(
                            &services.tx,
                            &services.waker,
                            AppEvent::Phase8(Event::Message(TabPage::Usb, err.to_string())),
                        );
                    }
                });
            }
            UsbPage(page) => self.state.phase8.usb_page = page,
            OpenDownloads => {
                if !cfg!(test) {
                    let _ = system_controls::execute_quick_action(
                        system_controls::QuickAction::PinnedFolder("downloads"),
                    );
                }
            }
            RecentOpen(i) | RecentShow(i) => {
                let Some(file) = self.phase8.recent.get(i).filter(|_| !cfg!(test)) else { return; };
                if action == RecentOpen(i) { files::shell_open(&file.path.to_string_lossy()); } else { files::shell_select(&file.path); }
            }
        }
    }

    pub(super) fn apply_phase8_event(&mut self, event: Event) {
        match event {
            #[cfg(windows)]
            Event::Stored(mut stored) => {
                if self.config.clipboard_persist {
                    for item in self.phase8.history.items().iter().rev() {
                        let id = stored.history.add(item.clip.clone(), &item.app_id, item.at_ms).id;
                        if item.pinned { let _ = stored.history.set_pinned(id, true); }
                    }
                    self.phase8.history = stored.history;
                    stored.images.extend(std::mem::take(&mut self.phase8.images));
                    self.phase8.images = stored.images;
                    self.phase8.thumbnails.extend(stored.thumbnails);
                }
                if self.config.shelf_persist {
                    for item in self.phase8.shelf.items().iter().rev() { stored.shelf.add(item.clone()); }
                    self.phase8.shelf = stored.shelf;
                    self.phase8.shelf_thumbnails.extend(stored.shelf_thumbnails);
                }
                self.phase8.store_ready = true;
                self.refresh_clipboard_rows();
                self.refresh_shelf();
                self.persist_phase8();
            }
            #[cfg(windows)]
            Event::Thumbnail(key, Some(thumb), shelf) => {
                if shelf && self.phase8.shelf.items().iter().any(|i| i.uri == key) {
                    self.phase8.shelf_thumbnails.insert(key, std::sync::Arc::new(thumb)); self.refresh_shelf();
                } else if !shelf && self.phase8.images.contains_key(&key) {
                    self.phase8.thumbnails.insert(key, std::sync::Arc::new(thumb)); self.refresh_clipboard_rows();
                }
            }
            #[cfg(windows)]
            Event::Clipboard(generation, value)
                if generation == self.phase8.clipboard_generation
                    && (self.config.modules.clipboard || self.config.clipboard_notice) =>
            {
                self.phase8.clipboard_reading = false;
                if let Some(value) = value {
                    if self.config.clipboard_notice {
                        self.show_copy_notice(&value);
                    }
                    if self.config.modules.clipboard {
                        self.capture_clip(value);
                    }
                }
                if std::mem::take(&mut self.phase8.clipboard_again) {
                    self.queue_clipboard_capture();
                }
            }
            Event::ShelfPicked(Some(path)) => { self.park_shelf_file(path); }
            Event::ShelfDropped(paths) => {
                let mut added = false;
                for path in paths { added |= self.park_shelf_file(path); }
                if added { self.announce_shelf(); }
            }
            Event::Usb(generation, drives)
                if self.config.modules.usb && generation == self.phase8.usb_generation =>
            {
                if self.phase8.usb_loaded && self.config.usb_alerts {
                    for drive in drives
                        .iter()
                        .filter(|drive| {
                            !self
                                .phase8
                                .usb
                                .iter()
                                .any(|known| known.letter == drive.letter)
                        })
                        .cloned()
                        .collect::<Vec<_>>()
                    {
                        self.alert(
                            "USB conectado",
                            &format!("{} ({})", drive.label, drive.letter),
                            Some(TabPage::Usb),
                        );
                    }
                }
                self.phase8.usb_loaded = true;
                self.phase8.usb_running = false;
                self.state.phase8.usb = drives
                    .iter()
                    .map(|drive| format!("{} ({})", drive.label, drive.letter))
                    .collect();
                self.state.phase8.usb_page = self
                    .state
                    .phase8
                    .usb_page
                    .min(drives.len().saturating_sub(1) / isle_core::phase8::ROWS_PER_PAGE);
                self.phase8.usb = drives;
                if std::mem::take(&mut self.phase8.usb_again) {
                    self.refresh_usb();
                }
            }
            Event::DownloadDetails(generation, snapshot)
                if self.config.modules.downloads && generation == self.phase8.download_generation =>
            {
                self.state.phase8.download_state = snapshot.state;
                self.state.phase8.download_rows = if self.config.download_details { snapshot.rows.into_iter().map(|row| isle_core::phase8::DownloadRowView {
                    name: row.name, bytes: row.bytes, total: row.total, bytes_per_second: row.bytes_per_second,
                }).collect() } else { Vec::new() };
                self.apply_phase8_event(Event::Download(generation, snapshot.progress));
            }
            Event::RecentDownloads(generation, recent)
                if self.config.modules.downloads && self.config.download_recent && generation == self.phase8.download_generation =>
            {
                self.state.phase8.download_recent = recent.iter().map(|file| isle_core::phase8::RecentDownloadView {
                    name: file.name.clone(), size: file.size,
                    modified_ms: file.modified.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64),
                }).collect();
                self.phase8.recent = recent;
            }
            Event::Download(generation, progress)
                if self.config.modules.downloads
                    && generation == self.phase8.download_generation =>
            {
                // One finished (or all): its file is among the recent ones now.
                if self.state.phase8.downloads.is_some_and(|before| progress.is_none_or(|now| now.active_count < before.count)) {
                    self.refresh_recent_downloads();
                }
                self.state.phase8.download_fill.update(progress.and_then(|p| p.permille), progress.is_some(), self.state.now_ms);
                self.state.phase8.downloads = progress.map(|value| DownloadView {
                    count: value.active_count,
                    permille: value.permille,
                });
            }
            // A share closed before it was installed may still hold a landed upload.
            #[cfg(windows)]
            Event::ShareStarted(generation, Ok(share)) if generation != self.phase8.share_generation => {
                for path in share.stop_and_drain() { self.park_shelf_file_received(path, true); }
            }
            Event::ShareStarted(generation, result)
                if generation == self.phase8.share_generation =>
            {
                self.phase8.share_starting = false;
                match result {
                    Ok(share) => {
                        self.phase8.share = Some(share);
                        self.state.phase8.message = None;
                        self.refresh_share();
                    }
                    Err(error) => self.phase8_message(TabPage::Shelf, error),
                }
            }
            Event::ShareChanged(generation) if generation == self.phase8.share_generation => {
                self.refresh_share()
            }
            Event::Message(tab, message) => self.phase8_message(tab, message),
            _ => {}
        }
    }
}

/// How much of a copied text the notice shows (Mac Island's kClipboardChars).
const NOTICE_CHARS: usize = 32;

/// The notice's line for a copy, or None when there's nothing to say.
#[cfg(windows)]
fn copy_notice(payload: &ClipboardPayload, screenshot: bool) -> Option<String> {
    match payload {
        ClipboardPayload::Files(uris) if uris.len() == 1 => clipboard::file_path(&uris[0])
            .and_then(|path| path.file_name().map(|name| name.to_string_lossy().into_owned())),
        ClipboardPayload::Files(uris) => Some(format!("{} arquivos", uris.len())),
        ClipboardPayload::Text(text) => {
            // One line, without runs of spaces.
            let mut line = String::new();
            for c in text.chars().map(|c| if c.is_whitespace() { ' ' } else { c }) {
                if c != ' ' || line.chars().last().is_some_and(|last| last != ' ') {
                    line.push(c);
                }
                if line.chars().count() > NOTICE_CHARS {
                    break;
                }
            }
            let line = line.trim_end();
            if line.chars().count() > NOTICE_CHARS {
                Some(line.chars().take(NOTICE_CHARS).collect::<String>() + "…")
            } else {
                (!line.is_empty()).then(|| line.to_owned())
            }
        }
        ClipboardPayload::Image { .. } => (!screenshot).then(|| "Imagem".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use isle_core::phase8::ROWS_PER_PAGE;

    #[cfg(windows)]
    #[test]
    fn copy_notice_shows_one_short_line_and_skips_screenshots() {
        let text = |t: &str| copy_notice(&ClipboardPayload::Text(t.into()), false);
        assert_eq!(text("  Olá\n\n   mundo\t!").as_deref(), Some("Olá mundo !"));
        assert_eq!(text(&"a".repeat(40)), Some("a".repeat(32) + "…"));
        assert_eq!(text(" \n "), None);
        let files = ClipboardPayload::Files(vec!["file:///C:/x/a.pdf".into(), "file:///C:/x/b.pdf".into()]);
        assert_eq!(copy_notice(&files, false).as_deref(), Some("2 arquivos"));
        let image = ClipboardPayload::Image { png: Vec::new(), width: 1, height: 1 };
        assert_eq!(copy_notice(&image, false).as_deref(), Some("Imagem"));
        assert_eq!(copy_notice(&image, true), None);
    }

    #[cfg(windows)]
    #[test]
    fn a_received_upload_reaches_a_full_shelf_by_evicting_the_oldest() {
        let mut app = IsleApp::new(IsleConfig::default());
        for n in 0..isle_core::shelf::CAPACITY {
            app.park_shelf_file_received(PathBuf::from(format!("C:/x/old{n}.txt")), true);
        }
        app.park_shelf_file_received(PathBuf::from("C:/x/phone.txt"), true);
        let items = app.phase8.shelf.items();
        assert_eq!(items.len(), isle_core::shelf::CAPACITY);
        assert_eq!(items[0].name, "phone.txt");
        assert!(!items.iter().any(|item| item.name == "old0.txt"));
    }

    #[cfg(windows)]
    #[test]
    fn a_received_upload_keeps_the_shelf_within_its_configured_limit() {
        let mut app = IsleApp::new(IsleConfig { shelf_capacity: 3, ..IsleConfig::default() });
        for n in 0..5 {
            app.park_shelf_file_received(PathBuf::from(format!("C:/x/up{n}.txt")), true);
        }
        let names: Vec<_> = app.phase8.shelf.items().iter().map(|item| item.name.as_str()).collect();
        assert_eq!(names, ["up4.txt", "up3.txt", "up2.txt"]);
    }

    #[cfg(windows)]
    #[test]
    fn copy_notice_works_without_the_history_and_keeps_nothing() {
        let mut app = IsleApp::new(IsleConfig { clipboard_notice: true, ..IsleConfig::default() });
        let capture = ClipboardCapture { sequence: 1, payload: ClipboardPayload::Text("segredo".into()), screenshot: false };
        app.show_copy_notice(&capture);
        assert!(matches!(&app.islands[0].mode, PillMode::AlertBanner { title, subtitle } if title == "Copiado" && subtitle == "segredo"));
        assert!(app.phase8.history.items().is_empty());
    }

    #[test]
    fn files_over_the_pill_ask_to_let_go_and_keep_an_open_island_open() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.files_over(0, true);
        assert!(matches!(&app.islands[0].mode, PillMode::AlertBanner { subtitle, .. } if subtitle == DROP_HINT));
        let later = std::time::Instant::now() + Duration::from_secs(60);
        app.on_deadlines(std::time::Instant::now());
        app.on_deadlines(later);
        assert!(matches!(app.islands[0].mode, PillMode::AlertBanner { .. }), "the hint stays while the files do");
        app.files_over(0, false);
        assert_eq!(app.islands[0].mode, PillMode::CompactBar);
        app.islands[0].mode = PillMode::Expanded(TabPage::Clipboard);
        app.files_over(0, true);
        app.on_deadlines(std::time::Instant::now());
        app.on_deadlines(later);
        assert_eq!(app.islands[0].mode, PillMode::Expanded(TabPage::Clipboard), "not closed by the leave timer");
        app.files_over(0, false);
        app.on_deadlines(later);
        app.on_deadlines(later + Duration::from_secs(60));
        assert_eq!(app.islands[0].mode, PillMode::CompactBar, "the normal rule again");
    }

    #[test]
    fn reducing_history_limit_keeps_pins_and_clamps_the_page() {
        let mut app = IsleApp::new(IsleConfig::default());
        let pinned = app
            .phase8
            .history
            .add(Clip::from_text("fixado").unwrap(), "", 0)
            .id;
        app.phase8.history.set_pinned(pinned, true).unwrap();
        for i in 0..(ROWS_PER_PAGE * 2) {
            let text = format!("cópia {i}");
            app.phase8
                .history
                .add(Clip::from_text(&text).unwrap(), "", i as i64);
        }
        app.state.phase8.clipboard_page = 2;
        let old = app.config.clone();
        app.config.clipboard_limit = 2;
        app.apply_config(&old, false);
        assert_eq!(app.state.phase8.clipboard_page, 0);
        assert_eq!(app.state.phase8.clipboard.len(), 3);
        assert!(app
            .state
            .phase8
            .clipboard
            .iter()
            .any(|row| row.id == pinned && row.pinned));
    }

    #[test]
    fn clipboard_search_filters_the_history_and_can_be_cleared_with_escape() {
        let mut cfg = IsleConfig::default();
        cfg.modules.clipboard = true;
        let mut app = IsleApp::new(cfg);
        app.phase8
            .history
            .add(Clip::from_text("Pesquisa azul").unwrap(), "", 0);
        let id = app
            .phase8
            .history
            .add(Clip::from_text("Pesquisa verde").unwrap(), "", 1)
            .id;
        app.phase8.history.set_pinned(id, true).unwrap();
        app.phase8_action(0, Phase8Action::SearchClips);
        for ch in "VERDE".chars() {
            assert!(app.clipboard_search_input(0, ch));
        }
        assert_eq!(app.state.phase8.clipboard.len(), 1);
        assert_eq!(app.state.phase8.clipboard[0].id, id);
        assert!(app.clipboard_search_input(0, '\u{1b}'));
        assert_eq!(app.state.phase8.clipboard.len(), 2);
        assert!(!app.state.phase8.clipboard_searching);
        app.phase8_action(0, Phase8Action::FilterPinnedClips);
        assert_eq!(app.state.phase8.clipboard.len(), 1);
        assert_eq!(app.state.phase8.clipboard[0].id, id);
    }

    #[test]
    fn disabled_downloads_ignore_queued_progress() {
        let mut app = IsleApp::new(IsleConfig::default());
        let progress = Some(downloads::DownloadProgress {
            active_count: 1,
            permille: Some(640),
        });
        app.apply_phase8_event(Event::Download(0, progress));
        assert!(app.state.phase8.downloads.is_none());
        let old = app.config.clone();
        app.config.modules.downloads = true;
        app.apply_config(&old, false);
        app.apply_phase8_event(Event::Download(0, progress));
        assert_eq!(app.state.phase8.downloads.unwrap().permille, Some(640));
        assert!(app.state.phase8.download_meter);
    }

    #[test]
    fn usb_refresh_announces_new_letters_once_after_initial_snapshot() {
        let mut config = IsleConfig::default();
        config.modules.usb = true;
        let mut app = IsleApp::new(config);
        let drive = |letter: &str, label: &str| system_controls::UsbDriveInfo {
            letter: letter.into(),
            label: label.into(),
        };
        app.apply_phase8_event(Event::Usb(0, vec![drive("E:", "Arquivo")]));
        assert!(app.alerts.is_empty());
        app.apply_phase8_event(Event::Usb(
            0,
            vec![drive("E:", "Renomeado"), drive("F:", "Fotos")],
        ));
        assert_eq!(app.alerts.len(), 1);
        assert_eq!(app.alerts[0].1, "Fotos (F:)");
    }
}

