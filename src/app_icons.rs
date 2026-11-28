// SPDX-License-Identifier: GPL-3.0-or-later
//! Port of icon source workers/TellIconsRead (mac-island 3.1.1).
use super::*;
use isle_core::icons::{IconImages, IconPack};
use isle_platform_win::icon_packs::{load_folder, pack_path, prepare};
use std::{path::PathBuf, sync::Arc};

#[derive(Default)]
pub(super) struct Work { task: Option<tokio::task::JoinHandle<()>>, generation: u64, pub status: String, disk: Arc<tokio::sync::Mutex<()>> }
impl Work {
    fn cancel(&mut self) {if let Some(t)=self.task.take(){t.abort();}}
    fn spawn(&mut self,rt:&tokio::runtime::Handle,job:impl std::future::Future<Output=()>+Send+'static) {
        self.cancel();let disk=self.disk.clone();
        // A cancelled task can still be inside a synchronous disk call. This barrier survives
        // dropping its JoinHandle, including pack -> Windows -> same pack and delete/reselect.
        self.task=Some(rt.spawn(async move {let _guard=disk.lock().await;job.await;}));
    }
}
impl Drop for Work { fn drop(&mut self) {if let Some(t)=self.task.take(){t.abort();}} }
pub(super) enum Event { Status(u64,String), Loaded(u64,Arc<IconImages>), Folder(Option<PathBuf>) }

impl IsleApp {
    fn icons_root(&self) -> Option<PathBuf> {
        Some(std::env::var_os("APPDATA").map(PathBuf::from)?.join(isle_core::data_dir::NAME).join("icons"))
    }
    // Port of setting an icon source/DownloadIconPack worker (mac-island 3.1.1).
    pub(super) fn start_icons(&mut self) {
        self.icons_work.cancel();
        self.icons_work.generation += 1;
        let generation = self.icons_work.generation;
        isle_render::icon_images::set_images(Arc::new(IconImages::default()));
        let pack = self.config.icon_pack;
        self.icons_work.status = isle_core::i18n::tr(if pack==IconPack::Windows {"icons.windows"}else{"icons.loading"}).into();
        self.refresh_settings_view();self.settings.dirty=true;
        if pack == IconPack::Windows {return;}
        let (Some(Services {rt,tx,waker}),Some(root)) = (self.services.clone(),self.icons_root()) else {return};
        let folder = self.config.icon_folder.clone();
        // A network folder (UNC, \\?\UNC, \\.\): reading it would send the user's Windows credentials
        // to that server, and the folder may come from an imported backup. Local drives only.
        if pack == IconPack::Folder && !local_folder(&folder) {
            tracing::warn!("pasta de ícones na rede ignorada: {folder}");
            return;
        }
        self.icons_work.spawn(&rt,async move {
            let path = if pack == IconPack::Folder {PathBuf::from(&folder)} else {pack_path(&root,pack).unwrap()};
            send(&tx,&waker,AppEvent::Icons(Event::Loaded(generation,Arc::new(load_folder(&path,pack,folder.clone())))));
            let done = prepare(&root,pack,folder.clone(),|n,total| {
                send(&tx,&waker,AppEvent::Icons(Event::Status(generation,isle_core::i18n::trf("icons.downloading", &[("n", &n), ("total", &total)]))));
            }).await;
            let status = match done {
                Ok(images) => { let n=images.pictures.len(); send(&tx,&waker,AppEvent::Icons(Event::Loaded(generation,Arc::new(images)))); isle_core::i18n::trf("icons.ready", &[("n", &n)]) }
                Err(e) => {
                    tracing::warn!("ícones: {e:#}");
                    send(&tx,&waker,AppEvent::Icons(Event::Loaded(generation,Arc::new(load_folder(&path,pack,folder)))));
                    isle_core::i18n::tr("icons.failed").into()
                }
            };
            send(&tx,&waker,AppEvent::Icons(Event::Status(generation,status)));
        });
    }
    pub(super) fn apply_icons(&mut self, event: Event) {
        match event {
            Event::Status(g,text) if g==self.icons_work.generation => self.icons_work.status=text,
            Event::Loaded(g,images) if g==self.icons_work.generation => isle_render::icon_images::set_images(images),
            Event::Folder(Some(path)) => {
                let old=self.config.clone(); self.config.icon_folder=path.to_string_lossy().into(); self.config.icon_pack=IconPack::Folder;
                self.apply_config(&old,true);
            }
            _ => return,
        }
        self.refresh_settings_view(); self.settings.dirty=true;
    }
    pub(super) fn pick_icons(&mut self) {
        let Some(Services {tx,waker,..})=self.services.clone() else {return};
        std::thread::spawn(move || send(&tx,&waker,AppEvent::Icons(Event::Folder(isle_platform_win::files::pick_icon_folder()))));
    }
    // Port of IconPackDelete (mac-island 3.1.1): release it, cancel downloads, delete only its cache.
    pub(super) fn delete_icons(&mut self) {
        let (Some(root),Some(Services {rt,tx,waker}))=(self.icons_root(),self.services.clone()) else {return};
        let Some(path)=pack_path(&root,self.config.icon_pack) else {return};
        self.icons_work.cancel();
        let old=self.config.clone(); self.config.icon_pack=IconPack::Windows; self.apply_config(&old,true);
        let generation=self.icons_work.generation;
        self.icons_work.spawn(&rt,async move {
            let result=(|| -> anyhow::Result<()> {
                if !path.exists(){return Ok(());}
                let base=root.canonicalize()?; let target=path.canonicalize()?;
                anyhow::ensure!(target.parent()==Some(base.as_path()),"Pasta de cache inválida");
                std::fs::remove_dir_all(target)?; Ok(())
            })();
            let status=isle_core::i18n::tr(if result.is_ok(){"icons.deleted"}else{"icons.delete_failed"});
            send(&tx,&waker,AppEvent::Icons(Event::Status(generation,status.into())));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arrow_pin_survives_mouse_leave_and_is_per_island() {
        let mut app=IsleApp::new(IsleConfig::default()); app.config.ui.arrow_pins=true;
        app.islands.push(Presence::new()); app.islands[0].toggle(Some(TabPage::Shelf));
        app.apply_ui_action_on(0,UiAction::ToggleCollapse);
        assert!(app.islands[0].pinned); assert!(!app.islands[1].pinned);
        let now=Instant::now(); app.islands[0].hover(false,app.config.ui);
        app.islands[0].leave(now,true,app.config.ui); app.islands[0].leave(now+Duration::from_secs(5),true,app.config.ui);
        assert_eq!(app.islands[0].mode,PillMode::Expanded(TabPage::Shelf));
        app.apply_ui_action_on(0,UiAction::ToggleCollapse); assert!(!app.islands[0].pinned);
        app.apply_ui_action_on(0,UiAction::ToggleCollapse); app.handle_island_hotkey(1);
        assert!(!app.islands[0].pinned); assert_eq!(app.islands[0].mode,PillMode::Minimized);
    }
    #[test]
    fn obsolete_icon_worker_events_are_ignored() {
        let mut app=IsleApp::new(IsleConfig::default()); app.icons_work.generation=4;
        app.apply_icons(Event::Status(3,"obsolete".into())); assert_ne!(app.icons_work.status,"obsolete");
        app.apply_icons(Event::Status(4,"ready".into())); assert_eq!(app.icons_work.status,"ready");
    }
    #[test]
    fn liquid_glass_does_not_add_an_idle_wake_deadline() {
        let mut app=IsleApp::new(IsleConfig::default());let now=Instant::now();
        let before=app.sleep_for(now,1_800_000_000);
        app.config.liquid_glass=true;
        assert_eq!(app.sleep_for(now,1_800_000_000),before);
    }
    #[test]
    fn cancelled_disk_phase_finishes_before_a_replacement_even_after_windows_selection() {
        use std::sync::atomic::{AtomicBool,Ordering};
        let rt=tokio::runtime::Builder::new_multi_thread().worker_threads(2).build().unwrap();
        let mut work=Work::default();let active=Arc::new(AtomicBool::new(false));let old=active.clone();
        let (started,rx)=std::sync::mpsc::channel();
        work.spawn(rt.handle(),async move {
            old.store(true,Ordering::SeqCst);started.send(()).unwrap();
            // Like std::fs::rename/remove_dir_all: abort cannot interrupt a running synchronous poll.
            std::thread::sleep(Duration::from_millis(150));old.store(false,Ordering::SeqCst);
        });
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
        work.cancel(); // Windows selection drops the old JoinHandle.
        let (finished,rx)=std::sync::mpsc::channel();
        work.spawn(rt.handle(),async move {finished.send(active.load(Ordering::SeqCst)).unwrap();});
        assert!(!rx.recv_timeout(Duration::from_secs(2)).unwrap(),"new pack overlapped the cancelled disk phase");
    }
}

/// Only a folder on a drive letter (`C:\...`, `\\?\C:\...`): never UNC, `\\.\` devices or relative paths.
// ponytail: a mapped network drive (Z:) passes; GetDriveTypeW if that matters.
fn local_folder(folder: &str) -> bool {
    use std::path::{Component, Path, Prefix};
    let folder = folder.trim().replace('/', "\\");
    matches!(Path::new(&folder).components().next(),
        Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
}

#[cfg(test)]
mod local_folder_tests {
    #[test]
    fn only_drive_letter_folders_are_local() {
        for ok in ["C:\\Icones", "d:/icons", " C:\\x ", "\\\\?\\C:\\x"] { assert!(super::local_folder(ok), "{ok}"); }
        for bad in ["\\\\srv\\share", "//srv/share", "\\/srv\\share", " \\\\srv\\s", "\\\\?\\UNC\\srv\\s", "\\\\.\\pipe\\x", "\\??\\C:\\x", "icons", "\\icons", ""] {
            assert!(!super::local_folder(bad), "{bad}");
        }
    }
}
