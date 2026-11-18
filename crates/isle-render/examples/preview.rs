// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Adapted for isle-win from Mac Island & Dock by caliberda v3.1.0 (ControlPanel::DrawMain):
// Fedora previews of control-center and NotificationPanel states.
// TrayListPanel/pinned icon previews adapted from Mac Island & Dock v3.1.1.

//! Renders each pill mode to PNG, to see the island on Fedora without Windows:
//! `cargo run -p isle-render --example preview -- <dir> [scale]` (1.5 for a 150% screen)

use isle_core::config::IsleConfig;
use isle_core::spring::PillGeometry;
use isle_core::state::{IslandState, PermissionCard, PillMode, TabPage};
use isle_render::renderer::PillRenderer;

fn settled(w: f32, h: f32, r: f32) -> PillGeometry {
    let mut geom = PillGeometry::default();
    geom.morph_to(w, h, r);
    geom.width.value = w;
    geom.height.value = h;
    geom.corner_radius.value = r;
    geom
}

fn main() {
    // Every state is built on the stack at once: more room than the main thread's 1 MB.
    std::thread::Builder::new().stack_size(64 << 20).spawn(shots).unwrap().join().unwrap();
}

fn shots() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    std::fs::create_dir_all(&dir).expect("create preview folder");
    // ISLE_PREVIEW_ACCENT="0,200,0" draws everything with another accent color.
    if let Ok(v) = std::env::var("ISLE_PREVIEW_ACCENT") {
        let c: Vec<u8> = v.split(',').filter_map(|c| c.trim().parse().ok()).collect();
        if let [r, g, b] = c[..] {
            isle_render::set_accent((r, g, b));
        }
    }
    let preview_art = std::env::var("ISLE_PREVIEW_ARTWORK")
        .ok()
        .and_then(|path| tiny_skia::Pixmap::load_png(path).ok())
        .and_then(|p| isle_core::artwork::Artwork::new(p.width(), p.height(), p.data().to_vec()))
        .map(std::sync::Arc::new);
    let mut cfg = IsleConfig::default();
    cfg.liquid_glass = std::env::var_os("ISLE_PREVIEW_GLASS").is_some();
    // Explicit preview choice only; the preview never downloads anything.
    if let Ok(name) = std::env::var("ISLE_PREVIEW_ICON_PACK") {
        cfg.icon_pack = isle_core::icons::IconPack::ALL.into_iter().find(|p|format!("{p:?}").eq_ignore_ascii_case(&name)).expect("known icon pack");
    }
    if let Ok(dir) = std::env::var("ISLE_PREVIEW_ICON_FOLDER") {
        let mut images = isle_core::icons::IconImages {pack:cfg.icon_pack,folder:cfg.icon_folder.clone(),..Default::default()};
        for icon in isle_core::icons::Icon::ALL {
            let info=icon.info();
            let name=cfg.icon_pack.source().and_then(|(_,_,_,i)|info.names[i]).unwrap_or(info.name);
            for file in [format!("{dir}/{name}.svg"),format!("{dir}/{name}.png"),format!("{dir}/{:04X}.svg",info.glyph)] {
                if let Ok(data)=std::fs::read(file) {images.pictures.insert(info.glyph,data);break;}
            }
        }
        isle_render::icon_images::set_images(std::sync::Arc::new(images));
    }
    cfg.pinned_tray_apps = ["Tailscale", "Deskflow", "Google Drive"]
        .map(|name| isle_core::config::PinnedTrayApp {
            exe_path: String::new(),
            guid: None,
            display_name: name.into(),
        })
        .to_vec();
    let mut base = IslandState::default();
    base.clock_text = "22:41".into();
    base.date_text = "ter 10 nov".into();
    base.claude_quota.used_pct = 62.0;
    base.codex_quota.used_pct = 84.0;

    let mut tray = base.clone();
    // Optional real registry snapshots, exported locally into an ignored folder. No private data
    // or third-party app images are embedded in the repository.
    let icons = std::env::var("ISLE_PREVIEW_TRAY_ICONS").ok();
    tray.tray = ["Tailscale","Deskflow","Google Drive","Steam","Discord","Windhawk","Lightshot",
        "Wallpaper Engine","Spotify","Google Chrome","NVIDIA","WhatsApp","Security","Phone","Other",
    ]
        .into_iter().enumerate().map(|(i,name)| isle_core::tray::TrayItem {
            key: name.into(), display_name: name.into(), pinned: i < 3,
            png: icons.as_ref().and_then(|dir| std::fs::read(format!("{dir}/{name}.png")).ok()).unwrap_or_default(),
            ..Default::default()
        }).collect();

    let mut card = base.clone();
    card.mode = PillMode::PermissionPrompt;
    card.pending_permission = Some(PermissionCard {
        request_id: "claude-0".into(),
        agent: "Claude Code · isle-win".into(),
        tool_name: "Bash".into(),
        command: "cargo build --release --target x86_64-pc-windows-msvc --offline && ./scripts/build-win.sh && echo pronto && rm -rf ~/scratch/antigo".into(),
        ..Default::default()
    });
    let mut banner = base.clone();
    banner.mode = PillMode::AlertBanner {
        title: "Claude Code concluiu · isle-win".into(),
        subtitle: "Portei o núcleo e os agentes do zickrian/isle.".into(),
    };
    let mut expanded = base.clone();
    expanded.mode = PillMode::Expanded(TabPage::AiHub);
    // The real limits read on 10/11/2026 at 22:57 (São Paulo).
    let now = 1_794_364_634_000;
    expanded.now_ms = now;
    expanded.utc_offset_s = -3 * 3600;
    expanded.today = (now / 1000 - 3 * 3600).div_euclid(86_400);
    let window = |label: &str, percent: f64, resets_at_ms: i64| isle_core::state::UsageWindow { label: label.into(), percent, resets_at_ms,
    };
    expanded.usage = vec![
        isle_core::state::UsageProvider {
            id: "claude".into(),
            name: "Claude".into(),
            plan: "Pro".into(),
            updated_ms: now - 180_000,
            windows: vec![window("5 horas", 75.0, 1_794_376_200_025), window("Semana", 48.0, 1_794_556_800_025), window("Semana · Fable", 0.0, 1_794_556_800_000),
            ],
            ..Default::default()
        },
        isle_core::state::UsageProvider {
            id: "codex".into(),
            name: "Codex".into(),
            plan: "Plus".into(),
            updated_ms: now - 600_000,
            note: "Do último uso do Codex".into(),
            windows: vec![window("5 horas", 33.0, 1_794_376_762_000), window("Semana", 47.0, 1_794_497_502_000),
            ],
            ..Default::default()
        },
    ];

    let mut control = base.clone();
    control.mode = PillMode::Expanded(TabPage::ControlCenter);
    control.audio_available = true; control.mic_available = true; control.volume_pct = 65;
    control.wifi_connected = Some(true); control.wifi_name = Some("Casa · 5 GHz".into());
    control.bluetooth_on = Some(true); control.night_light_on = Some(false);
    control.audio_outputs = ["Alto-falantes", "Fone de ouvido", "LG ULTRAGEAR", "Realtek Digital Output",
    ].iter().enumerate()
        .map(|(i, n)| isle_core::state::AudioOutput { id: i.to_string(), name: (*n).into(), is_default: i == 1,
    }).collect();
    control.app_volumes = vec![
        isle_core::state::AppVolume { id: "firefox".into(), name: "Firefox".into(), volume_pct: 80, muted: false,
        },
        isle_core::state::AppVolume { id: "spotify".into(), name: "Spotify".into(), volume_pct: 42, muted: true,
        },
    ];
    let mut notifications = base.clone();
    let notification_icon = std::env::var("ISLE_PREVIEW_NOTIFICATION_ICON").ok()
        .and_then(|p| tiny_skia::Pixmap::load_png(p).ok())
        .and_then(|p| isle_core::artwork::Artwork::new(p.width(),p.height(),p.data().to_vec()))
        .map(std::sync::Arc::new);
    notifications.mode = PillMode::Expanded(TabPage::Notifications);
    notifications.now_ms = now;
    notifications.notifications.receive([
        (1, "Thunderbird", "Reunião de amanhã", "O encontro foi confirmado para as 10h.",
            ),
            (
                2,
                "GitHub",
                "Build Windows concluído",
                "isle-win · Todos os checks passaram.",
            ),
        (3, "WhatsApp", "\u{1fa79}", "Ana \u{1f469}\u{1f3fd}\u{200d}\u{1f4bb}: Mensagem de voz (0:13) \u{2764}\u{fe0f}"),
        (4, "Calendário", "Lembrete", "Revisar as notificações no Windows.",
            ),
    ].into_iter().map(|(id,app,title,body)| isle_core::notifications::Notification {
        id, app_id: app.into(), app_name: app.into(), title: title.into(), body: body.into(),
        app_icon: if id==3 {notification_icon.clone()} else {None},
        arrival_ms: now - (5-id)*60_000, unread: false,
        // Telegram's can be answered from the island, like WhatsApp's.
        reply_input: if id == 3 { "box".into() } else { String::new() }, reply_args: "send".into(), ..Default::default()
    },
        )
        .collect(),
        false,
    );
    // The Agenda tab: sample events and holidays; ISLE_PREVIEW_ICS=<file> shows a real agenda instead.
    let mut agenda = base.clone();
    agenda.mode = PillMode::Expanded(TabPage::CalendarWeather);
    agenda.today = isle_core::agenda::days_from_civil(2026, 11, 11);
    agenda.agenda_day = agenda.today;
    let sample = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:a\r\nDTSTART:20261111T130000Z\r\nSUMMARY:Daily do time\r\nEND:VEVENT\r\n\
        BEGIN:VEVENT\r\nUID:b\r\nDTSTART;VALUE=DATE:20261111\r\nSUMMARY:Entrega da fase 6\r\nEND:VEVENT\r\n\
        BEGIN:VEVENT\r\nUID:c\r\nDTSTART;VALUE=DATE:20261104\r\nRRULE:FREQ=MONTHLY;BYMONTHDAY=18\r\nSUMMARY:Fatura do cartão\r\nEND:VEVENT\r\n\
        BEGIN:VEVENT\r\nUID:d\r\nDTSTART:20261113T220000Z\r\nSUMMARY:Jantar com a família\r\nEND:VEVENT\r\nEND:VCALENDAR";
    let raw = std::env::var("ISLE_PREVIEW_ICS")
        .ok()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .unwrap_or_else(|| sample.into());
    agenda.agenda = isle_core::agenda::expand(
        &isle_core::agenda::parse_ics(&raw, -3 * 3600),
        agenda.today - 40,
        agenda.today + 400,
    );
    agenda.agenda.extend(isle_core::agenda::parse_holidays(r#"[{"date":"2026-11-15","localName":"Proclamação da República"},{"date":"2026-11-20","localName":"Dia Nacional de Zumbi e da Consciência Negra"},{"date":"2026-12-25","localName":"Natal"}]"#));
    agenda
        .agenda
        .sort_by_key(|i| (i.day, i.minute.map_or(-1, |m| m as i64)));
    let mut agenda_holiday = agenda.clone();
    agenda_holiday.agenda_day = isle_core::agenda::days_from_civil(2026, 11, 20);
    let mut notification_banner = notifications.clone();
    notification_banner.notification_banner = notifications.notifications.ordered().into_iter().find(|n| n.id==3).cloned();
    notification_banner.mode = PillMode::AlertBanner {
        title: "WhatsApp · \u{1fa79}".into(),
        subtitle: "Ana \u{1f469}\u{1f3fd}\u{200d}\u{1f4bb}: Mensagem de voz (0:13) \u{2764}\u{fe0f}".into(),
    };
    let chat = |provider, input: &str, response: &str, busy, error: Option<&str>| {
        let mut state = base.clone();
        state.mode = PillMode::Expanded(TabPage::AiHub);
        state.chat.open = true;
        state.chat.provider = provider;
        state.chat.input.text = input.into();
        state.chat.busy = busy;
        state.chat.caret = Some(!busy);
        state.chat.error = error.map(str::to_owned);
        if !response.is_empty() || busy || error.is_some() {
            state.chat.items = vec![
                isle_core::chat::Message {
                    user: true,
                    text: "Como funciona a ilha quando não estou usando?".into(),
                },
                isle_core::chat::Message {
                    user: false,
                    text: response.into(),
                },
            ];
        }
        state
    };
    // The weather: a real Open-Meteo answer for Bom Jesus (11/11/2026).
    let forecast = isle_core::weather::Weather::from_open_meteo(
        "Bom Jesus, São Paulo",
        include_str!("open-meteo-sample.json"),
    )
    .unwrap();
    let mut weather = base.clone();
    weather.weather = Some(forecast);
    weather.mode = PillMode::Expanded(TabPage::Weather);
    let mut compact_weather = weather.clone();
    compact_weather.mode = PillMode::CompactBar;
    // The Timer tab: Pomodoro idle and running, the countdown set, the stopwatch running.
    use isle_core::timers::{TimerMode, TimersView};
    let timer = |view: TimersView| {
        let mut s = base.clone();
        s.mode = PillMode::Expanded(TabPage::Timer);
        s.timers = view;
        s
    };
    let idle = TimersView {
        focus_minutes: 25,
        break_minutes: 5,
        pomodoro_remaining_ms: 25 * 60_000,
        pomodoro_duration_ms: 25 * 60_000,
        countdown_remaining_ms: 10 * 60_000,
        countdown_duration_ms: 10 * 60_000,
        ..Default::default()
    };
    let shots = [
        (
            "timer-pomodoro",
            timer(idle.clone()),
            settled(520.0, 360.0, 26.0),
        ),
        (
            "timer-pomodoro-running",
            timer(TimersView {
                pomodoro_phase: isle_core::pomodoro::Phase::Focus,
                pomodoro_running: true,
                pomodoro_remaining_ms: 14 * 60_000 + 32_000,
                ..idle.clone()
            }),
            settled(520.0, 360.0, 26.0),
        ),
        (
            "timer-countdown",
            timer(TimersView {
                mode: TimerMode::Countdown,
                ..idle.clone()
            }),
            settled(520.0, 360.0, 26.0),
        ),
        (
            "timer-reminders",
            timer(TimersView {
                mode: TimerMode::Reminders,
                reminder_minutes: 30,
                reminder_typing: true,
                reminder_text: "Ligar para o dentista".into(),
                reminders: vec![
                    isle_core::timers::ReminderLine {
                        id: 1,
                        text: "Tirar a roupa da máquina".into(),
                        when: "às 17:30".into(),
                    },
                    isle_core::timers::ReminderLine {
                        id: 2,
                        text: "Pagar a fatura do cartão".into(),
                        when: "amanhã às 08:00".into(),
                    },
                ],
                ..idle.clone()
            }),
            settled(520.0, 360.0, 26.0),
        ),
        (
            "timer-reminders-empty",
            timer(TimersView {
                mode: TimerMode::Reminders,
                reminder_minutes: 15,
                ..idle.clone()
            }),
            settled(520.0, 360.0, 26.0),
        ),
        (
            "timer-stopwatch",
            timer(TimersView {
                mode: TimerMode::Stopwatch,
                stopwatch_running: true,
                stopwatch_ms: 83_000,
                ..idle.clone()
            }),
            settled(520.0, 360.0, 26.0),
        ),
        ("weather", weather.clone(), settled(560.0, 366.0, 26.0)),
        (
            "weather-day-3",
            {
                let mut s = weather.clone();
                s.weather_day = 3;
                s
            },
            settled(560.0, 366.0, 26.0),
        ),
        (
            "compact-weather",
            compact_weather,
            settled(284.0, 34.0, 17.0),
        ),
        (
            "ai-empty",
            chat(
                isle_core::state::AiProviderKind::ClaudeCode,
                "",
                "",
                false,
                None,
            ),
            settled(520.0, 360.0, 26.0),
        ),
        (
            "ai-typing",
            chat(
                isle_core::state::AiProviderKind::Codex,
                "Pode explicar isso em português? ação e acentos",
                "",
                false,
                None,
            ),
            settled(520.0, 360.0, 26.0),
        ),
        (
            "ai-streaming",
            chat(
                isle_core::state::AiProviderKind::ClaudeCode,
                "",
                "A ilha espera por eventos do Windows.\n\nDurante uma conversa, o texto chega aos poucos; a interface acorda somente quando há algo novo para mostrar…",
                true,
                None,
            ),
            settled(520.0, 360.0, 26.0),
        ),
        (
            "ai-history",
            chat(
                isle_core::state::AiProviderKind::OpenCodeGo,
                "Obrigado!",
                "A interface fica esperando eventos, usando o Waker para acordar.\n\n1. Você escreve uma mensagem.\n2. O CLI responde em uma thread de trabalho.\n3. Os trechos chegam pelo canal de eventos.\n\nO histórico curto fica nesta sessão. Você pode voltar às mensagens anteriores pelos botões ou pela roda do mouse.\n\nQuando termina, o processo é encerrado. Não há consultas periódicas do chat em repouso.",
                false,
                None,
            ),
            settled(520.0, 360.0, 26.0),
        ),
        (
            "ai-error",
            chat(
                isle_core::state::AiProviderKind::OpenCodeGo,
                "",
                "",
                false,
                Some(
                    "Coloque a chave do OpenCode Go em Configurações → IA. Depois, tente enviar novamente.",
                ),
            ),
            settled(520.0, 360.0, 26.0),
        ),
        (
            "tray-compact",
            tray.clone(),
            settled(isle_render::pages::compact_tray_width(&tray), 34.0, 17.0),
        ),
        (
            "tray-panel",
            {
                let mut s = tray.clone();
                s.mode = PillMode::Expanded(TabPage::Tray);
                s
            },
            settled(520.0, 360.0, 26.0),
        ),
        (
            "tray-page-2",
            {
                let mut s = tray.clone();
                s.mode = PillMode::Expanded(TabPage::Tray);
                s.tray_page = 1;
                s
            },
            settled(520.0, 360.0, 26.0),
        ),
        (
            "tray-unavailable",
            {
                let mut s = base.clone();
                s.mode = PillMode::Expanded(TabPage::Tray);
                s.tray_error = Some("A barra de tarefas não está acessível nesta sessão".into());
                s
            },
            settled(520.0, 360.0, 26.0),
        ),
        (
            "notification-banner",
            notification_banner,
            settled(440.0, 64.0, 20.0),
        ),
        (
            "notifications",
            notifications.clone(),
            settled(520.0, 360.0, 26.0),
        ),
        (
            "notifications-reply",
            {
                let mut s = notifications.clone();
                s.reply = Some(isle_core::notifications::Reply {
                    id: 3,
                    arrival_ms: now - 2 * 60_000,
                    text: "Já vou descer, só um minuto 🙂".into(),
                });
                s
            },
            settled(520.0, 360.0, 26.0),
        ),
        (
            "notifications-page-2",
            {
                let mut s = notifications.clone();
                s.notification_page = 1;
                s
            },
            settled(520.0, 360.0, 26.0),
        ),
        (
            "notifications-grouped",
            {
                let mut s = notifications.clone();
                s.notifications.receive(
                    (5..=6)
                        .map(|id| isle_core::notifications::Notification {
                            id,
                            app_id: "Telegram".into(),
                            app_name: "Telegram".into(),
                            title: format!("Mensagem {id}"),
                            body: "Mais uma mensagem no mesmo aplicativo.".into(),
                            arrival_ms: now + id,
                            unread: false,
                            ..Default::default()
                        })
                        .collect(),
                    false,
                );
                s
            },
            settled(520.0, 360.0, 26.0),
        ),
        (
            "notifications-unavailable",
            {
                let mut s = base.clone();
                s.mode = PillMode::Expanded(TabPage::Notifications);
                s.notification_error = Some("Notificações indisponíveis: banco ocupado".into());
                s
            },
            settled(520.0, 360.0, 26.0),
        ),
        (
            "control-center",
            control.clone(),
            settled(520.0, 360.0, 26.0),
        ),
        ("agenda", agenda, settled(520.0, 360.0, 26.0)),
        (
            "agenda-holiday",
            agenda_holiday,
            settled(520.0, 360.0, 26.0),
        ),
        (
            "control-center-unavailable",
            {
                let mut unavailable = base.clone();
                unavailable.mode = PillMode::Expanded(TabPage::ControlCenter);
                unavailable.control_error = Some("Áudio indisponível".into());
                unavailable
            },
            settled(520.0, 360.0, 26.0),
        ),
        ("compact", base.clone(), settled(isle_render::pages::compact_tray_width(&base), 34.0, 17.0)),
        ("banner", banner, settled(360.0, 64.0, 20.0)),
        ("card", card.clone(), {
            let c = card.pending_permission.as_ref().unwrap().command.clone();
            settled(440.0, isle_render::renderer::card_height(&c), 22.0)
        }),
        ("card-growing", card, {
            let mut g = settled(440.0, 130.0, 22.0);
            (g.width.value, g.height.value) = (300.0, 80.0);
            g
        }),
        ("expanded", expanded, settled(560.0, 450.0, 28.0)),
        (
            "media",
            {
                let mut media = base.clone();
                media.mode = PillMode::Expanded(TabPage::Media);
                media.now_ms = 1_000_000;
                media.media = Some(isle_core::state::MediaInfo {
                    artwork: preview_art.clone(),
                    title: "Bohemian Rhapsody".into(),
                    artist: "Queen".into(),
                    playing: true,
                    position_ms: 61_000,
                    duration_ms: 354_000,
                    sampled_at_ms: 1_000_000,
                });
                media.active_lyric_line =
                    Some("Is this the real life? Is this just fantasy?".into());
                media
            },
            settled(520.0, 260.0, 26.0),
        ),
        (
            "minimized",
            {
                let mut line = IslandState::default();
                line.mode = PillMode::Minimized;
                line
            },
            settled(120.0, 5.0, 2.5),
        ),
    ];
    let shots = shots.into_iter().chain(phase8_shots(&base));
    let scale: f32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);
    let mut renderer = PillRenderer::with_scale(720, 520, scale);
    for (name, mut state, mut geom) in shots {
        if !wanted(name) {continue;}
        if let PillMode::Expanded(tab) = state.mode {
            let (w, mut h, r) = isle_render::layout::expanded_size(tab);
            if let Ok(v)=std::env::var("ISLE_PREVIEW_VIEWPORT_H"){if let Ok(v)=v.parse::<f32>(){h=h.min(v);}}
            state.panel_scroll=std::env::var("ISLE_PREVIEW_SCROLL").ok().and_then(|s|s.parse().ok()).unwrap_or(0.);
            geom = settled(w, h, r);
        }
        let fb = renderer.render(&geom, &state, &cfg);
        let pixmap = desktop_preview(fb,&geom,&state,&cfg,scale);
        let path = format!("{dir}/{name}.png");
        pixmap.save_png(&path).expect("write png");
        println!("{path}");
    }
    phase9_shots(&dir, scale, &base, &cfg);
    // The settings window: every tab wide, and two narrow.
    let mut wide = cfg.clone();
    wide.modules.clipboard = true;
    wide.modules.shelf = true;
    wide.modules.usb = true;
    wide.modules.downloads = true;
    wide.calendar_ics_url = Some("https://calendar.google.com/x.ics".into());
    for tab in isle_core::prefs::SettingsTab::ALL {
        settings_shot(&dir, &format!("settings-{}", tab.key()), (1180, 760), scale, tab, &wide, &base);
    }
    for step in 1..isle_core::prefs::WELCOME.len() {
        settings_shot(&dir, &format!("settings-welcome-{step}"), (1180, 760), scale, isle_core::prefs::SettingsTab::Welcome, &wide, &base);
    }
    for tab in [isle_core::prefs::SettingsTab::Bar, isle_core::prefs::SettingsTab::Control] {
        settings_shot(&dir, &format!("settings-narrow-{}", tab.key()), (600, 500), scale, tab, &wide, &base);
    }
}

fn phase8_shots(base: &IslandState) -> Vec<(&'static str, IslandState, PillGeometry)> {
    use isle_core::phase8::{ClipRow, DownloadView};
    let mut sample = base.clone();
    sample.modules.clipboard = true;
    sample.modules.shelf = true;
    sample.modules.usb = true;
    sample.modules.downloads = true;
    sample.phase8.clipboard = [
        ("Notas da revisão do layout", true),
        ("https://example.com", false),
        ("Imagem · 1920 × 1080", false),
        ("Referências.pdf", false),
    ].into_iter().enumerate().map(|(id, (label, pinned))| ClipRow {
        id: id as u64 + 1, label: label.into(), pinned,
    }).collect();
    sample.phase8.clipboard_has_recent = true;
    sample.phase8.shelf = ["Proposta.pdf", "cimori.png", "anotações.txt"].map(str::to_owned).to_vec();
    sample.phase8.shelf_capacity = 6;
    sample.phase8.shelf_qr = true;
    sample.phase8.usb = vec!["Fotos (E:)".into(), "Backup (F:)".into()];
    sample.phase8.downloads = Some(DownloadView { count: 1, permille: Some(660) });
    sample.phase8.download_meter = true;
    sample.phase8.download_rows = vec![isle_core::phase8::DownloadRowView { name: "Download do navegador".into(), bytes: 568_000_000, total: Some(861_000_000), bytes_per_second: Some(12_300_000) }];
    sample.phase8.download_recent = [("Relatório anual 2026.pdf", 2_400_000, 3), ("cimori-setup.exe", 48_000_000, 42), ("foto.jpg", 830_000, 190), ("planilha.xlsx", 61_000, 1500), ("música.mp3", 7_200_000, 4000)]
        .into_iter().map(|(name, size, min)| isle_core::phase8::RecentDownloadView { name: name.into(), size, modified_ms: sample.now_ms - min * 60_000 }).collect();
    let mut shots = [
        ("phase8-clipboard", TabPage::Clipboard),
        ("phase8-shelf", TabPage::Shelf),
        ("phase8-usb", TabPage::Usb),
        ("phase8-downloads", TabPage::Downloads),
    ].into_iter().map(|(name, tab)| {
        let mut state = sample.clone();
        state.mode = PillMode::Expanded(tab);
        let (w, h, r) = isle_render::layout::expanded_size(tab);
        (name, state, settled(w, h, r))
    }).collect::<Vec<_>>();
    // Two downloads without totals, one paused: their bars slide, the summary has no %.
    let mut several = shots[3].1.clone();
    several.phase8.downloads = Some(DownloadView { count: 2, permille: None });
    several.phase8.download_state = 8;
    several.phase8.download_rows = vec![
        isle_core::phase8::DownloadRowView { name: "video.mp4".into(), bytes: 120_000_000, total: None, bytes_per_second: Some(2_000_000) },
        isle_core::phase8::DownloadRowView { name: "dados.zip".into(), bytes: 40_000_000, total: Some(80_000_000), bytes_per_second: None },
    ];
    shots.push(("phase8-downloads-several", several, shots[3].2.clone()));
    let mut idle = shots[3].1.clone();
    idle.phase8.downloads = None;
    idle.phase8.download_rows.clear();
    shots.push(("phase8-downloads-idle", idle, shots[3].2.clone()));
    let mut dropping = shots[0].1.clone();
    dropping.drop_hint = true;
    shots.push(("phase8-drop-open", dropping, shots[0].2.clone()));
    let mut drop_banner = base.clone();
    drop_banner.mode = PillMode::AlertBanner { title: "Prateleira".into(), subtitle: "Solte para pôr na prateleira".into() };
    shots.push(("phase8-drop-closed", drop_banner, settled(360., 64., 20.)));
    sample.mode = PillMode::CompactBar;
    sample.ui.media_controls = true;
    sample.media = Some(isle_core::state::MediaInfo { title: "A faixa em reprodução".into(), playing: true, ..Default::default() });
    let width = isle_render::pages::compact_tray_width(&sample);
    shots.push(("phase8-compact", sample, settled(width, 44., 20.)));
    let mut minimized = base.clone();
    minimized.mode = PillMode::Minimized;
    minimized.ui.timer_on_line = true;
    minimized.timer_text = Some("04:23".into());
    shots.push(("phase8-line-timer", minimized, settled(120., 19., 2.5)));
    shots
}

/// One state drawn with its own settings (the notch's style) into `dir/name.png`.
fn wanted(name:&str)->bool {
    std::env::var("ISLE_PREVIEW_FILTER").map_or(true,|filter|filter.split(',').any(|part|part==name))
}

/// Reference composition over a synthetic flat desktop. This is never a screen capture:
/// the production tone expression runs in DWM; previews evaluate it against this known input.
fn desktop_preview(fb:&isle_render::renderer::PillFrameBuffer,geom:&PillGeometry,state:&IslandState,cfg:&IsleConfig,scale:f32)->tiny_skia::Pixmap {
    let gray=std::env::var("ISLE_PREVIEW_BACKGROUND").ok().and_then(|v|v.parse::<u8>().ok()).unwrap_or(110);
    let mut mask=tiny_skia::Mask::new(fb.width,fb.height).unwrap();
    if cfg.liquid_glass && state.mode!=PillMode::Minimized {
        let canvas=fb.width as f32/scale;let height=fb.height as f32/scale;
        if let Some(path)=isle_render::glass::glass_path(canvas,geom.width.value.clamp(4.,canvas-8.),geom.height.value.clamp(2.,height-12.),geom.corner_radius.value,cfg.top_gap(),cfg.ui.pill_style==isle_core::config::PillStyle::Notch,cfg.notch_curve,scale) {
            mask.fill_path(&path,tiny_skia::FillRule::Winding,true,tiny_skia::Transform::identity());
        }
    }
    let toned=isle_render::glass::tone([gray as f32/255.;3],matches!(state.mode,PillMode::Expanded(_)))[0]*255.;
    let mut rgba=Vec::with_capacity(fb.bgra_premultiplied.len());
    for (i,px) in fb.bgra_premultiplied.chunks_exact(4).enumerate() {
        let coverage=mask.data()[i] as f32/255.;let desktop=(gray as f32+(toned-gray as f32)*coverage).round() as u16;
        let keep=255-px[3] as u16;let over=|c:u8|(c as u16+desktop*keep/255).min(255) as u8;
        rgba.extend_from_slice(&[over(px[2]),over(px[1]),over(px[0]),255]);
    }
    tiny_skia::Pixmap::from_vec(rgba,tiny_skia::IntSize::from_wh(fb.width,fb.height).unwrap()).unwrap()
}

fn save_shot(dir: &str, scale: f32, name: &str, state: &IslandState, geom: &PillGeometry, cfg: &IsleConfig) {
    if !wanted(name) {return;}
    let mut renderer = PillRenderer::with_scale(720, 520, scale);
    let fb = renderer.render(geom, state, cfg);
    let pixmap = desktop_preview(fb,geom,state,cfg,scale);
    let path = format!("{dir}/{name}.png");
    pixmap.save_png(&path).expect("write png");
    println!("{path}");
}

/// Phase 9: the notch (closed, opening, open, as a line) and its settings.
fn phase9_shots(dir: &str, scale: f32, base: &IslandState, cfg: &IsleConfig) {
    let mut notch = cfg.clone();
    notch.ui.pill_style = isle_core::config::PillStyle::Notch;
    let mut closed = base.clone();
    closed.bar = notch.bar_shown();
    closed.ui = notch.ui;
    let w = isle_render::pages::compact_tray_width(&closed);
    let r = isle_render::renderer::notch_radius(34.0, notch.notch_curve);
    save_shot(dir, scale, "notch-compact", &closed, &settled(w, 34.0, r), &notch);
    let mut items = closed.clone();
    items.bar = vec![isle_core::config::BarItem::Clock, isle_core::config::BarItem::Weather];
    items.weather = isle_core::weather::Weather::from_open_meteo("Rio", include_str!("open-meteo-sample.json")).ok();
    save_shot(dir, scale, "notch-compact-items", &items, &settled(isle_render::pages::compact_tray_width(&items), 34.0, r), &notch);
    let mut open = closed.clone();
    open.mode = PillMode::Expanded(TabPage::ControlCenter);
    let (ew, eh, er) = isle_render::layout::expanded_size(TabPage::ControlCenter);
    save_shot(dir, scale, "notch-expanded", &open, &settled(ew, eh, er), &notch);
    let mut growing = settled(ew, eh, er);
    (growing.width.value, growing.height.value, growing.corner_radius.value) = (w + 80.0, 160.0, 18.0);
    save_shot(dir, scale, "notch-growing", &open, &growing, &notch);
    let mut line = closed.clone();
    line.mode = PillMode::Minimized;
    save_shot(dir, scale, "notch-line", &line, &settled(120.0, 5.0, 2.5), &notch);
    // An agent's banner on the notch, glowing (its shoulders carry the glow too).
    let mut banner = closed.clone();
    banner.mode = PillMode::AlertBanner { title: "Claude Code concluiu · release".into(), subtitle: "Fechei o antigo e abri a versão nova".into() };
    let (bw, bh) = banner.ui.banner_px();
    let mut glowing = settled(bw, bh, isle_render::renderer::notch_radius(bh, notch.notch_curve));
    glowing.glow_intensity.value = 1.0;
    save_shot(dir, scale, "notch-banner-glow", &banner, &glowing, &notch);
    // T5: the power button on the bar, its menu (asking), and the center's new shortcuts.
    let mut bar = base.clone();
    bar.bar = vec![isle_core::config::BarItem::Clock, isle_core::config::BarItem::Next, isle_core::config::BarItem::Power];
    save_shot(dir, scale, "power-compact", &bar, &settled(isle_render::pages::compact_tray_width(&bar), 34.0, 17.0), cfg);
    // The clock in the middle: the date on its left, the weather on its right; then the clock alone.
    let mut centered = base.clone();
    centered.ui.clock_center = true;
    centered.bar = vec![isle_core::config::BarItem::Date, isle_core::config::BarItem::Clock, isle_core::config::BarItem::Weather];
    save_shot(dir, scale, "compact-clock-center", &centered, &settled(isle_render::pages::compact_tray_width(&centered), 34.0, 17.0), cfg);
    centered.bar = vec![isle_core::config::BarItem::Clock];
    save_shot(dir, scale, "compact-clock-alone", &centered, &settled(isle_render::pages::compact_tray_width(&centered), 34.0, 17.0), cfg);
    let mut center_cfg = cfg.clone();
    center_cfg.ui.clock_center = true;
    center_cfg.bar_items = vec![isle_core::config::BarItem::Date, isle_core::config::BarItem::Clock, isle_core::config::BarItem::Weather];
    settings_shot(dir, "settings-bar-clock-center", (1180, 760), scale, isle_core::prefs::SettingsTab::Bar, &center_cfg, base);
    let mut power = base.clone();
    power.mode = PillMode::Expanded(TabPage::Power);
    let (pw, ph, pr) = isle_render::layout::expanded_size(TabPage::Power);
    save_shot(dir, scale, "power-menu", &power, &settled(pw, ph, pr), cfg);
    power.power_confirm = Some(isle_core::power::PowerAction::ShutDown);
    save_shot(dir, scale, "power-confirm", &power, &settled(pw, ph, pr), cfg);
    let mut center = base.clone();
    center.mode = PillMode::Expanded(TabPage::ControlCenter);
    center.control_tiles.extend([isle_core::state::ControlTile::WindowsSettings, isle_core::state::ControlTile::Power]);
    save_shot(dir, scale, "control-shortcuts", &center, &settled(ew, eh, er), cfg);
    // T6: the control center being edited (top, scrolled to the catalog, a control dragged).
    let mut editing = center.clone();
    editing.control_editing = true;
    editing.audio_available = true;
    save_shot(dir, scale, "control-edit", &editing, &settled(ew, eh, er), cfg);
    editing.panel_scroll = 400.0;
    save_shot(dir, scale, "control-edit-scrolled", &editing, &settled(ew, eh, er), cfg);
    editing.panel_scroll = 0.0;
    editing.control_drag = Some((0, 3));
    save_shot(dir, scale, "control-edit-drag", &editing, &settled(ew, eh, er), cfg);
    let mut many = center.clone();
    many.control_tiles = isle_core::state::ControlTile::CATALOG[..18].to_vec();
    many.audio_available = true;
    save_shot(dir, scale, "control-many", &many, &settled(ew, eh, er), cfg);
    // T7: the banner at 150 % and narrower, the units in the settings, a shortcut being recorded.
    let mut big = base.clone();
    big.mode = PillMode::AlertBanner { title: "Claude Code concluiu · isle-win".into(), subtitle: "Os testes passaram".into() };
    big.ui.banner_size = 150;
    big.ui.banner_width_pct = 80;
    let (bw, bh) = big.ui.banner_px();
    save_shot(dir, scale, "banner-150", &big, &settled(bw, bh, 30.0), cfg);
    let mut keys = cfg.clone();
    keys.ui.minimize_keys = isle_core::shortcut::code(isle_core::shortcut::CTRL | isle_core::shortcut::ALT, b'K');
    settings_shot_recording(dir, "settings-shortcuts-recording", (1180, 760), scale, isle_core::prefs::SettingsTab::Shortcuts, &keys, base, Some("ui.notices_hotkey"));
    settings_shot(dir, "settings-notifications-units", (1180, 760), scale, isle_core::prefs::SettingsTab::Notifications, cfg, base);
    settings_shot(dir, "settings-look-notch", (1180, 760), scale, isle_core::prefs::SettingsTab::Look, &notch, base);
    settings_shot(dir, "settings-bar-notch", (1180, 760), scale, isle_core::prefs::SettingsTab::Bar, &notch, base);
}

fn settings_shot(dir: &str, name: &str, size: (u32, u32), scale: f32, tab: isle_core::prefs::SettingsTab, cfg: &IsleConfig, state: &IslandState) {
    settings_shot_recording(dir, name, size, scale, tab, cfg, state, None);
}

#[allow(clippy::too_many_arguments)]
fn settings_shot_recording(dir: &str, name: &str, (w, h): (u32, u32), scale: f32, tab: isle_core::prefs::SettingsTab, cfg: &IsleConfig, state: &IslandState, recording: Option<&'static str>) {
    if !wanted(name) {return;}
    let mut view = isle_render::settings::SettingsView { tab, recording, ..Default::default() };
    view.welcome_step = name.strip_prefix("settings-welcome-").and_then(|s| s.parse().ok()).unwrap_or(0);
    view.info.extend([("welcome.claude", "Hooks instalados".into()), ("welcome.codex", "Encontrado, sem hooks".into())]);
    view.info.extend([("info.version", "Versão 1.0.0".into()), ("info.hooks", "Instalados".into()), ("info.bridge", "Ligada".into())]);
    let place = |d: &str, left| isle_render::window::MonitorPlace { device: d.into(), left, top: 0, width: 2560, scale: 1.0, primary: left == 0 };
    view.info.push(("info.icons","Baixando ícones: 38/61".into()));
    view.info.push(("info.update","Cimori 1.1.0 disponível".into()));
    view.update = Some("1.1.0".into());
    view.info.push(("info.icon_credits","Lucide: ISC · Microsoft e demais autores: MIT · Google: Apache 2.0".into()));
    view.monitors = vec![place(r"\\.\DISPLAY1", 0), place(r"\\.\DISPLAY6", 2560)];
    let mut renderer = PillRenderer::with_scale(w, h, scale);
    let fb = renderer.render_settings(&view, cfg, state);
    let mut rgba = Vec::with_capacity(fb.bgra_premultiplied.len());
    for px in fb.bgra_premultiplied.chunks_exact(4) {
        rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
    }
    let pixmap = tiny_skia::Pixmap::from_vec(rgba, tiny_skia::IntSize::from_wh(fb.width, fb.height).unwrap()).unwrap();
    let path = format!("{dir}/{name}.png");
    pixmap.save_png(&path).expect("write png");
    println!("{path}");
}
