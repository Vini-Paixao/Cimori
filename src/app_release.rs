// The public release's chores: the update check (port of isle's isle-daemon/src/updates.rs).
use super::*;
use isle_core::updates;

#[derive(Default)]
pub(super) struct Release {
    /// The look at start and once a day, while "Verificar atualizações" is on.
    task: Option<tokio::task::JoinHandle<()>>,
    /// A newer version and its page.
    pub(super) newer: Option<(String, String)>,
    /// The version the island already announced (one banner per version).
    told: Option<String>,
    /// What Sobre says about it.
    pub(super) status: String,
}

/// GitHub's latest release; None offline, on a 404 (nothing published yet) or a bad answer: logged only.
async fn latest() -> Option<(String, String)> {
    match isle_platform_win::weather::fetch_github(updates::LATEST, concat!("Cimori/", env!("CARGO_PKG_VERSION"))).await {
        Ok(json) => updates::parse_latest(&json).or_else(|| {
            tracing::info!("atualizações: resposta sem versão");
            None
        }),
        Err(err) => {
            tracing::info!("atualizações: não deu para consultar o GitHub: {err:#}");
            None
        }
    }
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
                send(&tx, &waker, AppEvent::Update(latest().await));
                tokio::time::sleep(Duration::from_secs(updates::EVERY_SECS)).await;
            }
        }));
    }

    /// "Verificar agora": one look, whatever the option.
    pub(super) fn check_updates_now(&mut self) {
        let Some(Services { rt, tx, waker }) = self.services.clone() else { return };
        self.release.status = "Verificando…".into();
        rt.spawn(async move { send(&tx, &waker, AppEvent::Update(latest().await)) });
    }

    /// What GitHub said: a newer version is announced once on the island and offered in Sobre.
    pub(super) fn apply_update(&mut self, found: Option<(String, String)>) {
        let current = env!("CARGO_PKG_VERSION");
        match found {
            Some((version, page)) if updates::is_newer(&version, current) => {
                self.release.status = format!("Cimori {version} disponível");
                if self.release.told.as_deref() != Some(version.as_str()) {
                    self.alert(&format!("Cimori {version} disponível"), "Baixe em Configurações → Sobre", None);
                    self.release.told = Some(version.clone());
                }
                self.release.newer = Some((version, page));
            }
            Some(_) => {
                self.release.status = format!("Você tem a versão mais nova ({current})");
                self.release.newer = None;
            }
            None => self.release.status = "Não deu para verificar agora".into(),
        }
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
        self.settings.view.status = Some(if copied { "Código da ponte copiado: cole no Windhawk" } else { "Não deu para copiar o código" }.into());
        self.settings.dirty = true;
    }

    /// "Baixar": the release's page (always under the repository's releases).
    pub(super) fn open_update_page(&self) {
        if let Some((_, page)) = &self.release.newer {
            if page.starts_with(updates::RELEASES) && !cfg!(test) {
                isle_platform_win::files::shell_open(page);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_version_is_announced_once_and_offered() {
        let mut app = IsleApp::new(IsleConfig::default());
        let page = "https://github.com/Vini-Paixao/Cimori/releases/tag/v99.0.0".to_string();
        app.apply_update(Some(("99.0.0".into(), page.clone())));
        assert_eq!(app.release.newer, Some(("99.0.0".into(), page.clone())));
        assert_eq!(app.alerts.len(), 1);
        app.apply_update(Some(("99.0.0".into(), page)));
        assert_eq!(app.alerts.len(), 1, "the same version is not announced again");
        app.apply_update(Some((env!("CARGO_PKG_VERSION").into(), String::new())));
        assert!(app.release.newer.is_none());
        app.apply_update(None);
        assert_eq!(app.release.status, "Não deu para verificar agora");
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
