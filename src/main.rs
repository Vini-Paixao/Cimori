// No console window behind the island; the command-line modes borrow the caller's console below.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;

use anyhow::Result;
use app::IsleApp;
use isle_ai::agents;
use isle_core::config::IsleConfig;
use std::env;
use std::fs;
use std::path::PathBuf;

fn default_settings_path() -> PathBuf {
    if let Ok(custom) = env::var("ISLE_SETTINGS_PATH") {
        PathBuf::from(custom)
    } else if let Ok(appdata) = env::var("APPDATA") {
        PathBuf::from(appdata).join(isle_core::data_dir::NAME).join("settings.json")
    } else {
        PathBuf::from("dist").join("settings.json")
    }
}

/// This program as the AIs' hooks run it. Replaced while running (an update), Linux adds " (deleted)".
fn exe_path() -> Result<String> {
    let exe = env::current_exe()?;
    let exe = exe
        .to_str()
        .and_then(|path| path.strip_suffix(" (deleted)"))
        .map_or(exe.clone(), PathBuf::from);
    let exe = fs::canonicalize(&exe).unwrap_or(exe);
    // canonicalize gives Windows' \\?\ verbatim form; plain paths are what the AIs run best.
    let exe = exe.to_string_lossy();
    Ok(exe.strip_prefix(r"\\?\").unwrap_or(&exe).to_owned())
}

pub(crate) fn install_hooks(host: Option<&str>) -> Result<()> {
    let exe = exe_path()?;
    let claude_path = agents::claude::settings_path();
    agents::claude::install(&claude_path, &agents::claude::hook(&exe, host))?;
    println!("Claude Code: hooks em {}", claude_path.display());
    let codex_path = agents::codex::hooks_path();
    if codex_path.parent().is_some_and(|dir| dir.is_dir()) {
        agents::codex::install(&codex_path, &agents::codex::hook(&exe, host))?;
        println!("Codex: hooks em {}", codex_path.display());
    }
    println!("Ilha: {}", host.unwrap_or(agents::hook::DEFAULT_HOST));
    Ok(())
}

/// Hooks installed before the token (refused by the island now) or running isle-win.exe (now Cimori.exe):
/// put in again once, aimed where they were.
fn reinstall_outdated_hooks() {
    let Ok(exe) = exe_path() else { return };
    let claude = agents::claude::settings_path();
    let codex = agents::codex::hooks_path();
    let done = [
        agents::claude::outdated(&claude).map(|host| ("Claude Code", agents::claude::install(&claude, &agents::claude::hook(&exe, host.as_deref())))),
        agents::codex::outdated(&codex).map(|host| ("Codex", agents::codex::install(&codex, &agents::codex::hook(&exe, host.as_deref())))),
    ];
    for (name, result) in done.into_iter().flatten() {
        match result {
            Ok(()) => tracing::info!("{name}: hooks antigos reinstalados para a Cimori com o token"),
            Err(err) => tracing::warn!("{name}: não foi possível reinstalar os hooks antigos: {err:#}"),
        }
    }
}

pub(crate) fn uninstall_hooks() -> Result<()> {
    for (name, path, removed) in [
        ("Claude Code", agents::claude::settings_path(), agents::claude::remove as fn(&std::path::Path) -> Result<bool>,
        ),
        ("Codex", agents::codex::hooks_path(), agents::codex::remove),
    ] {
        if removed(&path)? {
            println!("{name}: hooks removidos de {}", path.display());
        }
    }
    Ok(())
}

/// A windowed program gets no console; when started from a terminal (--usage, --install-hooks),
/// print into that terminal. Pipes (the AI hooks) are already valid handles and stay as they are.
#[cfg(windows)]
fn use_parent_console() {
    use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_OUTPUT_HANDLE,
    };
    let has_stdout = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) }.is_ok_and(|h| !h.is_invalid() && !h.0.is_null());
    if !has_stdout {
        let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    // isle-win's folders become Cimori's before anything reads them; logged once logging starts.
    let migrated: Vec<_> = ["APPDATA", "LOCALAPPDATA"].into_iter()
        .filter_map(|var| env::var_os(var).filter(|v| !v.is_empty()))
        .map(|base| isle_core::data_dir::migrate(&PathBuf::from(base))).collect();
    #[cfg(windows)]
    if args.len() > 1 {
        use_parent_console();
    }
    // Shows the settings of the Cimori already running; with none, Cimori starts with them open.
    let open_settings = args.iter().any(|a| a == "--settings");
    if open_settings && isle_render::window::ask_running_to_open_settings() {
        return Ok(());
    }
    if args.iter().any(|a| a == "--version") {
        println!(
            "Cimori {} | visual 2026-11-11-r2",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    let flag = |name: &str| {
        args.windows(2)
            .find(|w| w[0] == name)
            .map(|w| w[1].as_str())
    };

    // Before logging starts: the hook's stdout is its answer to the AI and must hold nothing else.
    if let Some(ai) = flag("--island-hook") {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        rt.block_on(agents::hook::run(ai, flag("--host").unwrap_or(agents::hook::DEFAULT_HOST),
        ));
        return Ok(());
    }

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    // The island has no console: it also logs to Cimori.log beside its settings, started anew
    // each run (the command-line modes leave a running island's log alone).
    let log_file = (!args.iter().any(|a| a.starts_with("--") && a != "--settings"))
        .then(|| {
            let path = default_settings_path().with_file_name("Cimori.log");
            fs::create_dir_all(path.parent()?).ok()?;
            fs::File::create(path).ok()
        })
        .flatten();
    match log_file {
        Some(file) => {
            use tracing_subscriber::fmt::writer::MakeWriterExt;
            tracing_subscriber::fmt().with_env_filter(filter).with_ansi(false)
                .with_writer(std::io::stdout.and(std::sync::Mutex::new(file))).init();
        }
        None => tracing_subscriber::fmt().with_env_filter(filter).init(),
    }

    for result in migrated {
        match result {
            Ok(Some(old)) => tracing::info!("Pasta antiga movida para a da Cimori: {}", old.display()),
            Ok(None) => {}
            Err(err) => tracing::warn!("Não foi possível mover a pasta antiga do isle-win: {err}"),
        }
    }

    let windows_host = flag("--windows-host");
    if args.iter().any(|a| a == "--install-hooks") {
        return install_hooks(windows_host);
    }
    if args.iter().any(|a| a == "--uninstall-hooks") {
        return uninstall_hooks();
    }

    // The AIs' limits as the island reads them on this machine, once (to check a sign-in).
    if args.iter().any(|a| a == "--usage") {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        rt.block_on(isle_ai::usage::run(|providers| {
            println!("{}", serde_json::to_string_pretty(&providers).unwrap_or_default());
            std::process::exit(0);
        }));
        return Ok(());
    }

    // Tray diagnostics do not bind the hook port or replace an installed island's log/settings.
    if args.iter().any(|a| a == "--tray-inspect") {
        let raw = fs::read_to_string("Mac Island & Dock 2026-11-10.txt").unwrap_or_default();
        let cfg = IsleConfig::from_windhawk_backup(&raw);
        let snapshot = isle_platform_win::tray::inspect(&cfg.pinned_tray_apps)?;
        for item in &snapshot.items {
            println!("{} | pinned={} confirmed={} png={} | {}",item.display_name.replace('\n'," / "),item.pinned,item.confirmed,item.png.len(),item.key);
        }
        println!("tray: {} icons; status={:?}",snapshot.items.len(),snapshot.error);
        if let Some(seconds) = flag("--watch-seconds").and_then(|s| s.parse::<u64>().ok()) {
            let (tx,rx) = std::sync::mpsc::channel();
            let _service = isle_platform_win::tray::spawn(cfg.pinned_tray_apps,move |snapshot| { let _ = tx.send(snapshot); });
            let until = std::time::Instant::now()+std::time::Duration::from_secs(seconds.min(60));
            while let Ok(snapshot) = rx.recv_timeout(until.saturating_duration_since(std::time::Instant::now())) {
                println!("tray event: {} icons; status={:?}",snapshot.items.len(),snapshot.error);
            }
        }
        return Ok(());
    }

    let backup_path = PathBuf::from("Mac Island & Dock 2026-11-10.txt");
    let settings_path = default_settings_path();
    // After the isle-win folder moved over: an existing user has settings and sees no welcome.
    let first_run = !settings_path.exists();
    let cfg = IsleConfig::load_or_import(&settings_path, Some(&backup_path))?;
    // After the settings exist (the token is saved in them), so a first run still imports the backup.
    reinstall_outdated_hooks();
    let mut app = IsleApp::new(cfg);
    let cleared_path = settings_path.with_file_name("avisos-limpos.txt");
    app.state.notifications.cleared_until = fs::read_to_string(&cleared_path).ok()
        .and_then(|text| text.trim().parse().ok()).unwrap_or(0);
    app.cleared_path = Some(cleared_path);
    app.settings_path = Some(settings_path.clone());
    app.set_reminder_files(settings_path.with_file_name("lembretes.json"), settings_path.with_file_name("agenda-avisados.json"),
    );
    // The services' threads; this one is the UI's (Win32 windows belong to the thread that made them).
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    if first_run {
        app.show_welcome_at_start();
    } else if open_settings {
        app.show_settings_at_start();
    }
    app.run(&rt)
}
