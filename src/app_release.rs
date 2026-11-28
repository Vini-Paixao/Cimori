// The public release's chores: the update check (port of isle's isle-daemon/src/updates.rs).
use isle_core::i18n::{tr, trf};
use super::*;
use isle_core::updates;

const AGENT: &str = concat!("Cimori/", env!("CARGO_PKG_VERSION"));

#[derive(Default)]
pub(super) struct Release {
    /// The look at start and once a day, while "Buscar atualizações sozinha" is on.
    task: Option<tokio::task::JoinHandle<()>>,
    /// A newer version.
    pub(super) newer: Option<updates::Latest>,
    /// The version the island already announced (one banner per version).
    told: Option<String>,
    /// Its installer is being downloaded.
    installing: bool,
    /// What Atualizações says about it.
    pub(super) status: String,
}

/// What GitHub said of the latest release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Checked {
    Found(updates::Latest),
    /// A 404: nothing published yet.
    NothingPublished,
    /// Offline or a bad answer (logged).
    Failed,
}

async fn latest() -> Checked {
    match isle_platform_win::weather::fetch_github_opt(updates::LATEST, AGENT).await {
        Ok(Some(json)) => updates::parse_latest(&json).map_or_else(
            || {
                tracing::info!("atualizações: resposta sem versão");
                Checked::Failed
            },
            Checked::Found,
        ),
        Ok(None) => Checked::NothingPublished,
        Err(err) => {
            tracing::info!("atualizações: não deu para consultar o GitHub: {err:#}");
            Checked::Failed
        }
    }
}

/// The new version's installer, downloaded beside the other temporary files and checked against
/// its release's SHA256SUMS.txt.
async fn fetch_installer(latest: updates::Latest) -> Result<std::path::PathBuf, String> {
    use isle_platform_win::weather::{download, fetch_url_text};
    let (name, url) = latest.installer.ok_or(tr("update.no_installer"))?;
    let sums_url = latest.sums.ok_or(tr("update.no_sums"))?;
    let sums = fetch_url_text(&sums_url).await.map_err(|err| trf("update.sums_failed", &[("err", &err)]))?;
    let expected = updates::expected_sha256(&sums, &name).ok_or(tr("update.not_in_sums"))?;
    let bytes = download(&url, AGENT).await.map_err(|err| trf("update.download_failed", &[("err", &err)]))?;
    if isle_platform_win::files::sha256_hex(&bytes).as_deref() != Some(expected.as_str()) {
        return Err(tr("update.hash_mismatch").into());
    }
    let path = std::env::temp_dir().join(&name);
    std::fs::write(&path, bytes).map_err(|err| trf("update.save_failed", &[("err", &err)]))?;
    Ok(path)
}

impl IsleApp {
    /// Starts or stops the daily look, following the option.
    pub(super) fn sync_update_check(&mut self) {
        if !self.config.check_updates {
            if let Some(task) = self.release.task.take() {
                task.abort();
            }
            return;
        }
        if self.release.task.is_some() {
            return;
        }
        let Some(Services { rt, tx, waker }) = self.services.clone() else { return };
        self.release.task = Some(rt.spawn(async move {
            loop {
                send(&tx, &waker, AppEvent::Update { checked: latest().await, manual: false });
                tokio::time::sleep(Duration::from_secs(updates::EVERY_SECS)).await;
            }
        }));
    }

    /// "Buscar atualizações": one look, whatever the option; the answer shows here, not on the island.
    pub(super) fn check_updates_now(&mut self) {
        let Some(Services { rt, tx, waker }) = self.services.clone() else { return };
        self.release.status = tr("settings.searching").into();
        rt.spawn(async move { send(&tx, &waker, AppEvent::Update { checked: latest().await, manual: true }) });
    }

    /// What GitHub said: a newer version is offered in Atualizações and, found by the daily look,
    /// announced once on the island (its click opens that tab).
    pub(super) fn apply_update(&mut self, checked: Checked, manual: bool) {
        let current = env!("CARGO_PKG_VERSION");
        match checked {
            Checked::Found(latest) if updates::is_newer(&latest.version, current) => {
                let version = latest.version.clone();
                if !self.release.installing {
                    self.release.status = trf("update.available_have", &[("version", &version), ("current", &current)]);
                }
                if !manual && self.release.told.as_deref() != Some(version.as_str()) {
                    self.alerts.push_back((
                        trf("update.available", &[("version", &version)]),
                        tr("update.tap").into(),
                        Some(Opens::Settings(isle_core::prefs::SettingsTab::Updates)),
                    ));
                    self.release.told = Some(version);
                }
                self.release.newer = Some(latest);
            }
            Checked::Found(_) | Checked::NothingPublished => {
                self.release.status = trf("update.latest", &[("current", &current)]);
                self.release.newer = None;
            }
            Checked::Failed => self.release.status = tr("update.check_failed").into(),
        }
        self.settings_changed();
    }

    /// "Baixar e instalar": the installer downloaded and checked, then run (`run_installer`).
    pub(super) fn install_update(&mut self) {
        let Some(latest) = self.release.newer.clone().filter(|l| l.installer.is_some()) else { return };
        if self.release.installing {
            return;
        }
        let Some(Services { rt, tx, waker }) = self.services.clone() else { return };
        self.release.installing = true;
        self.release.status = trf("update.downloading", &[("version", &latest.version)]);
        self.settings_changed();
        rt.spawn(async move { send(&tx, &waker, AppEvent::Installer(fetch_installer(latest).await)) });
    }

    /// The checked installer runs on its own: it closes this Cimori, updates it and opens the new
    /// one; the settings are saved first.
    pub(super) fn run_installer(&mut self, result: Result<std::path::PathBuf, String>) {
        self.release.installing = false;
        match result {
            Ok(path) => {
                tracing::info!("atualizações: instalando {}", path.display());
                self.release.status = tr("update.installing").into();
                self.save_config();
                if !cfg!(test) {
                    let started = std::process::Command::new(&path).args(["/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART"]).spawn();
                    if let Err(err) = started {
                        tracing::warn!("atualizações: o instalador não abriu: {err}");
                        self.release.status = trf("update.installer_failed", &[("err", &err)]);
                    }
                }
            }
            Err(why) => {
                tracing::warn!("atualizações: {why}");
                self.release.status = why;
            }
        }
        self.settings_changed();
    }

    fn settings_changed(&mut self) {
        if self.settings.open {
            self.refresh_settings_view();
            self.settings.dirty = true;
        }
    }

    /// The Run key follows "Iniciar com o Windows"; on, it is rewritten when the exe moved.
    pub(super) fn sync_autostart(&self) {
        #[cfg(windows)]
        if !cfg!(test) {
            use isle_platform_win::files::autostart;
            let done = match (self.config.start_with_windows, autostart::get()) {
                (true, current) => {
                    let Ok(exe) = crate::exe_path() else { return };
                    let command = autostart::command(&exe);
                    if current.as_deref() == Some(command.as_str()) { return; }
                    autostart::set(&command)
                }
                (false, Some(_)) => autostart::remove(),
                (false, None) => return,
            };
            match done {
                Ok(()) => tracing::info!("iniciar com o Windows: {}", if self.config.start_with_windows { "ligado" } else { "desligado" }),
                Err(err) => tracing::warn!("iniciar com o Windows: {err}"),
            }
        }
    }

    /// The welcome's "Copiar código da ponte": the bridge mod's source, to paste into Windhawk.
    pub(super) fn copy_bridge(&mut self) {
        let copied = cfg!(test) || {
            #[cfg(windows)]
            {
                // A window must own the clipboard: the main island's.
                let payload = isle_platform_win::clipboard::ClipboardPayload::Text(include_str!("../windhawk/cimori-tray-bridge.wh.cpp").into());
                self.overlay.native_window().is_some_and(|hwnd| isle_platform_win::clipboard::restore(hwnd, &payload).is_ok())
            }
            #[cfg(not(windows))]
            false
        };
        self.settings.view.status = Some(tr(if copied { "settings.bridge_copied" } else { "settings.bridge_copy_failed" }).into());
        self.settings.dirty = true;
    }

    /// "Novidades da versão": the release's page (always under the repository's releases).
    pub(super) fn open_update_page(&self) {
        if let Some(latest) = &self.release.newer {
            if latest.page.starts_with(updates::RELEASES) && !cfg!(test) {
                isle_platform_win::files::shell_open(&latest.page);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_version_is_announced_once_by_the_daily_look_and_offered() {
        let mut app = IsleApp::new(IsleConfig::default());
        let latest = updates::Latest { version: "99.0.0".into(), page: updates::RELEASES.into(), ..Default::default() };
        // "Buscar atualizações": shown in the tab, no banner.
        app.apply_update(Checked::Found(latest.clone()), true);
        assert_eq!(app.release.newer, Some(latest.clone()));
        assert!(app.alerts.is_empty());
        // The daily look: one banner, leading to Atualizações.
        app.apply_update(Checked::Found(latest.clone()), false);
        assert_eq!(app.alerts.len(), 1);
        assert_eq!(app.alerts[0].2, Some(Opens::Settings(isle_core::prefs::SettingsTab::Updates)));
        app.apply_update(Checked::Found(latest), false);
        assert_eq!(app.alerts.len(), 1, "the same version is not announced again");
        let current = updates::Latest { version: env!("CARGO_PKG_VERSION").into(), ..Default::default() };
        app.apply_update(Checked::Found(current), false);
        assert!(app.release.newer.is_none());
        // Nothing published yet is no failure.
        app.apply_update(Checked::NothingPublished, true);
        assert!(app.release.status.starts_with("Você tem a versão mais nova"));
        app.apply_update(Checked::Failed, true);
        assert!(app.release.status.starts_with("Não deu para verificar"));
    }

    #[test]
    fn a_failed_download_says_why_and_can_be_tried_again() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.release.installing = true;
        app.run_installer(Err("O instalador baixado não confere com o hash da versão; nada foi instalado".into()));
        assert!(!app.release.installing);
        assert!(app.release.status.contains("hash"));
    }
}

#[cfg(test)]
mod welcome_tests {
    use super::*;
    use isle_core::prefs::{ActionId, SettingsTab};
    use isle_render::settings::SettingsAction;

    #[test]
    fn the_first_run_opens_on_the_welcome_and_walks_its_steps() {
        let mut app = IsleApp::new(IsleConfig::default());
        app.show_welcome_at_start();
        app.open_settings();
        assert_eq!(app.settings.view.tab, SettingsTab::Welcome);
        app.settings_action(SettingsAction::Run(ActionId::WelcomeBack));
        assert_eq!(app.settings.view.welcome_step, 0, "no step before the first");
        for _ in 0..10 {
            app.settings_action(SettingsAction::Run(ActionId::WelcomeNext));
        }
        assert_eq!(app.settings.view.welcome_step, isle_core::prefs::WELCOME.len() - 1);
        app.settings_action(SettingsAction::Run(ActionId::WelcomeFinish));
        assert_eq!(app.settings.view.tab, SettingsTab::General);
        app.settings_action(SettingsAction::Run(ActionId::ShowWelcome));
        assert_eq!((app.settings.view.tab, app.settings.view.welcome_step), (SettingsTab::Welcome, 0));
        // Every step's rows exist; the welcome's own rows stay out of the search.
        for (_, ids) in isle_core::prefs::WELCOME {
            assert!(ids.iter().all(|id| isle_core::prefs::by_id(id).is_some()), "{ids:?}");
        }
        assert!(isle_core::prefs::search("windhawk").iter().all(|p| p.tab != SettingsTab::Welcome));
    }
}
