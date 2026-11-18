use super::*;
use isle_core::timers::{TimerMode, clock};
use isle_core::weather::{Sky, degrees};
impl PillRenderer {
    pub(super) fn media_view(
        &mut self,
        canvas: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        s: &IslandState,
    ) {
        let Some(m) = &s.media else {
            self.empty_state(
                x,
                y + CONTENT + 20.,
                w,
                "Nada tocando por enquanto.",
                "Sua música aparece aqui quando começar.",
                Icon::Music,
            );
            return;
        };
        self.media_art(x + 24., y + CONTENT + 14., 100., s);
        self.label(
            if m.playing {
                "TOCANDO AGORA"
            } else {
                "EM PAUSA"
            },
            x + 146.,
            y + CONTENT + 19.,
            10.,
            Weight::Medium,
            muted(),
        );
        self.clipped_label(
            &m.title,
            x + 146.,
            y + CONTENT + 43.,
            w - 171.,
            18.,
            Weight::Semibold,
            white(),
        );
        self.clipped_label(
            &m.artist,
            x + 146.,
            y + CONTENT + 72.,
            w - 171.,
            13.,
            Weight::Regular,
            muted(),
        );
        let pos = m.position_at(s.now_ms);
        let by = y + CONTENT + 128.;
        let max = w - 48.;
        self.round(x + 24., by, max, 4., 2., surface());
        if m.duration_ms > 0 {
            self.round(
                x + 24.,
                by,
                (max * pos as f32 / m.duration_ms as f32)
                    .clamp(0., max)
                    .max(2.),
                4.,
                2.,
                white(),
            );
        }
        self.label(
            &mm_ss(pos),
            x + 24.,
            by + 12.,
            10.,
            Weight::Regular,
            muted(),
        );
        self.label_right(
            &mm_ss(m.duration_ms),
            x + w - 24.,
            by + 12.,
            Weight::Regular,
            muted(),
        );
        for hb in compute_hitboxes(canvas, w, h, y, s) {
            if let UiAction::Media(key) = hb.action {
                let play = key == MediaKey::PlayPause;
                self.round(
                    hb.x,
                    hb.y,
                    hb.w,
                    hb.h,
                    hb.w / 2.,
                    if play { white() } else { Color::TRANSPARENT },
                );
                let ic = match key {
                    MediaKey::Previous => Icon::Prev,
                    MediaKey::Next => Icon::Next,
                    _ => {
                        if m.playing {
                            Icon::Pause
                        } else {
                            Icon::Play
                        }
                    }
                };
                self.ui_icon(
                    ic,
                    hb.x + hb.w / 2.,
                    hb.y + hb.h / 2.,
                    20.,
                    if play { ink() } else { white() },
                );
            }
        }
        if let Some(lyric) = &s.active_lyric_line {
            let size = s.ui.lyric_size as f32;
            let (red, green, blue) = s.ui.lyric_rgb;
            let lyric = self.text.ellipsize(lyric, w - 56., size, Weight::Medium);
            self.centered_label(
                &lyric, x + w / 2., y + CONTENT + 224. - (size - 15.) * 0.5,
                size, Weight::Medium, Color::from_rgba8(red, green, blue, 255),
            );
        }
    }
    pub(super) fn agenda_view(
        &mut self,
        canvas: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        s: &IslandState,
    ) {
        const MONTHS: [&str; 12] = [
            "Janeiro",
            "Fevereiro",
            "Março",
            "Abril",
            "Maio",
            "Junho",
            "Julho",
            "Agosto",
            "Setembro",
            "Outubro",
            "Novembro",
            "Dezembro",
        ];
        const DAYS: [&str; 7] = [
            "Domingo",
            "Segunda-feira",
            "Terça-feira",
            "Quarta-feira",
            "Quinta-feira",
            "Sexta-feira",
            "Sábado",
        ];
        let sel = agenda_selected(s);
        let g = agenda_grid(x, y, w, s);
        if s.ui.agenda_featured_day {
            let (sy, sm, sd) = civil_from_days(sel);
            let holiday = s.agenda.iter().find(|i| i.day == sel && i.holiday);
            let day_width = g.x0 - x - 32.;
            self.clipped_label(
                DAYS[weekday(sel) as usize],
                x + 24.,
                y + CONTENT + 8.,
                day_width,
                11.,
                Weight::Medium,
                if holiday.is_some() { red() } else { accent() },
            );
            self.label(
                &sd.to_string(),
                x + 21.,
                y + CONTENT + 26.,
                64.,
                Weight::Medium,
                white(),
            );
            self.clipped_label(
                &format!("{} {sy}", MONTHS[sm as usize - 1]),
                x + 24.,
                y + CONTENT + 108.,
                day_width,
                12.,
                Weight::Regular,
                muted(),
            );
            if let Some(h) = holiday {
                self.clipped_label(
                    &h.title,
                    x + 24.,
                    y + CONTENT + 135.,
                    day_width.min(150.),
                    11.,
                    Weight::Medium,
                    red(),
                );
            }
        }
        self.label(
            &format!("{} {}", MONTHS[g.month as usize - 1], g.year),
            g.x0,
            y + CONTENT + 5.,
            13.,
            Weight::Medium,
            white(),
        );
        for (i, d) in ["D", "S", "T", "Q", "Q", "S", "S"].iter().enumerate() {
            self.centered_label(
                d,
                g.x0 + (i as f32 + 0.5) * g.cell_w,
                y + CONTENT + 30.,
                11.,
                Weight::Regular,
                muted(),
            );
        }
        for hb in compute_hitboxes(canvas, w, h, y, s) {
            match hb.action {
                UiAction::AgendaMonth(d) => self.ui_icon(
                    if d < 0 { Icon::Left } else { Icon::Right },
                    hb.x + hb.w / 2.,
                    hb.y + hb.h / 2.,
                    14.,
                    muted(),
                ),
                UiAction::AgendaDay(day) => {
                    let cx = hb.x + hb.w / 2.;
                    let selected = day == sel;
                    let today = day == s.today;
                    if selected || today {
                        self.round(
                            cx - 12.,
                            hb.y - 1.,
                            24.,
                            22.,
                            7.,
                            if today { accent() } else { surface() },
                        );
                    }
                    let is_holiday = s.agenda.iter().any(|i| i.day == day && i.holiday);
                    self.centered_label(
                        &civil_from_days(day).2.to_string(),
                        cx,
                        hb.y + 2.,
                        11.,
                        Weight::Medium,
                        if today {
                            ink()
                        } else if is_holiday {
                            red()
                        } else {
                            white()
                        },
                    );
                    if s.ui.agenda_events && s.agenda.iter().any(|i| i.day == day && !i.holiday) && !today {
                        self.round(cx + 9., hb.y + 2., 3., 3., 1.5, accent());
                    }
                }
                _ => {}
            }
        }
        if !s.ui.agenda_events { return; }
        let by = y + CONTENT + 190.;
        self.subtle_line(x + 24., by, w - 48.);
        let cw = (w - 68.) / 2.;
        for (col, future) in [false, true].into_iter().enumerate() {
            let lx = x + 24. + col as f32 * (cw + 20.);
            self.label(
                if future {
                    "Próximos"
                } else if sel == s.today {
                    "Hoje"
                } else {
                    "Neste dia"
                },
                lx,
                by + 13.,
                11.,
                Weight::Medium,
                muted(),
            );
            let events: Vec<_> = s
                .agenda
                .iter()
                .filter(|i| if future { i.day > sel } else { i.day == sel })
                .take(4)
                .collect();
            if events.is_empty() {
                self.clipped_label(
                    "Nenhum compromisso",
                    lx,
                    by + 40.,
                    cw,
                    12.,
                    Weight::Regular,
                    muted(),
                );
            }
            for (i, item) in events.iter().enumerate() {
                let ty = by + 39. + i as f32 * 24.;
                let when = if future {
                    let (_, m, d) = civil_from_days(item.day);
                    format!("{d:02}/{m:02}")
                } else if let Some(m) = item.minute {
                    format!("{:02}:{:02}", m / 60, m % 60)
                } else {
                    "Todo dia".into()
                };
                self.label(&when, lx, ty + 1., 10., Weight::Regular, muted());
                self.round(
                    lx + 48.,
                    ty + 1.,
                    2.,
                    16.,
                    1.,
                    if item.holiday { red() } else { accent() },
                );
                self.clipped_label(
                    &item.title,
                    lx + 59.,
                    ty,
                    cw - 59.,
                    12.,
                    Weight::Medium,
                    white(),
                );
            }
        }
        let _ = h;
    }
    pub(super) fn weather_view(
        &mut self,
        canvas: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        s: &IslandState,
    ) {
        let Some(v) = &s.weather else {
            self.empty_state(
                x,
                y + CONTENT + 30.,
                w,
                "O tempo por aqui",
                s.weather_error.as_deref().unwrap_or("Buscando o clima…"),
                Icon::Sun,
            );
            return;
        };
        let left = x + 24.;
        let inner = w - 48.;
        let b = y + CONTENT;
        self.clipped_label(
            &v.city,
            left,
            b + 2.,
            inner - 128.,
            13.,
            Weight::Medium,
            white(),
        );
        self.label(
            &degrees(v.temperature, s.fahrenheit),
            left,
            b + 24.,
            50.,
            Weight::Regular,
            white(),
        );
        self.sky_icon(Sky::of(v.code, v.day), x + w - 64., b + 46., 61.);
        self.clipped_label(
            &format!(
                "{} · Sensação de {}",
                Sky::of(v.code, v.day).name(),
                degrees(v.feels_like, s.fahrenheit)
            ),
            left,
            b + 83.,
            inner - 100.,
            11.,
            Weight::Regular,
            muted(),
        );
        self.label_right(
            &format!("{}  /  {}", v.sunrise, v.sunset),
            x + w - 24.,
            b + 83.,
            Weight::Regular,
            muted(),
        );
        let facts = [
            ("Umidade", format!("{}%", v.humidity)),
            (
                "Vento",
                if s.fahrenheit {
                    format!("{:0.0} mph", v.wind / 1.609)
                } else {
                    format!("{:0.0} km/h", v.wind)
                },
            ),
            ("Índice UV", v.uv.map_or("—".into(), |n| format!("{n:0.1}"))),
            (
                "Chuva",
                v.rain_chance.map_or("—".into(), |n| format!("{n}%")),
            ),
            ("Pressão", format!("{:0.0} hPa", v.pressure)),
            ("Nuvens", format!("{}%", v.clouds)),
            ("Sensação", degrees(v.feels_like, s.fahrenheit)),
            ("Céu", Sky::of(v.code, v.day).name().into()),
        ];
        self.subtle_line(left, b + 106., inner);
        for (i, (label, value)) in facts.iter().enumerate() {
            let lx = left + (i % 4) as f32 * inner / 4.;
            let ty = b + 115. + (i / 4) as f32 * 27.;
            self.label(label, lx, ty, 10., Weight::Regular, muted());
            self.clipped_label(
                value,
                lx,
                ty + 13.,
                inner / 4. - 9.,
                11.,
                Weight::Medium,
                white(),
            );
        }
        let day = s.weather_day.min(v.days.len().saturating_sub(1));
        for hb in compute_hitboxes(canvas, w, h, y, s) {
            let UiAction::WeatherDay(i) = hb.action else {
                continue;
            };
            let Some(d) = v.days.get(i) else { continue };
            let name = match i {
                0 => "Hoje",
                1 => "Amanhã",
                _ => super::super::weather_view::day_number(&d.date).map_or("", |d| {
                    ["Dom", "Seg", "Ter", "Qua", "Qui", "Sex", "Sáb"][weekday(d) as usize]
                }),
            };
            self.draw_forecast_day(&hb, d, name, i == day, s);
        }
        let Some(date) = v.days.get(day).map(|d| d.date.as_str()) else {
            return;
        };
        let hours = v.hours_of(date, 8);
        if !hours.is_empty() {
            let hi = hours.iter().map(|h| h.temperature).fold(f64::MIN, f64::max);
            let lo = hours.iter().map(|h| h.temperature).fold(f64::MAX, f64::min);
            let span = (hi - lo).max(1.);
            let col = inner / hours.len() as f32;
            let pts: Vec<_> = hours
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    (
                        left + (i as f32 + 0.5) * col,
                        b + 266. + ((hi - v.temperature) / span) as f32 * 23.,
                    )
                })
                .collect();
            self.stroke(&[&pts], accent(), 1.6);
            for (i, (hour, &(px, py))) in hours.iter().zip(&pts).enumerate() {
                self.circle(px, py, 2., accent());
                self.centered_label(
                    &degrees(hour.temperature, s.fahrenheit),
                    px,
                    py - 17.,
                    10.,
                    Weight::Medium,
                    white(),
                );
                self.sky_icon(Sky::of(hour.code, hour.day), px, b + 310., 16.);
                let wind = if s.fahrenheit {
                    format!("{:0.0} mph", hour.wind / 1.609)
                } else {
                    format!("{:0.0} km/h", hour.wind)
                };
                self.centered_label(&wind, px, b + 322., 9., Weight::Regular, muted());
                let time = if i == 0 && day == 0 {
                    "Agora"
                } else {
                    hour.time.get(11..16).unwrap_or("")
                };
                self.centered_label(time, px, b + 336., 10., Weight::Medium, white());
            }
        }
        self.clipped_label(
            s.weather_error
                .as_deref()
                .unwrap_or("Dados do clima: Open-Meteo"),
            left,
            y + h - 18.,
            inner,
            9.,
            Weight::Regular,
            if s.weather_error.is_some() {
                orange()
            } else {
                muted()
            },
        );
    }
    pub(super) fn focus_view(
        &mut self,
        canvas: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        s: &IslandState,
    ) {
        use isle_core::pomodoro::Phase;
        let t = &s.timers;
        let b = y + CONTENT;
        let running = match t.mode {
            TimerMode::Pomodoro => t.pomodoro_running,
            TimerMode::Countdown => t.countdown_running,
            TimerMode::Stopwatch => t.stopwatch_running,
            _ => false,
        };
        if t.mode != TimerMode::Reminders {
            let (time, caption, frac) = match t.mode {
                TimerMode::Pomodoro => (
                    clock(t.pomodoro_remaining_ms, true),
                    if matches!(t.pomodoro_phase, Phase::ShortBreak | Phase::LongBreak) {
                        "Pausa"
                    } else {
                        "Foco"
                    },
                    t.pomodoro_remaining_ms as f32 / t.pomodoro_duration_ms.max(1) as f32,
                ),
                TimerMode::Countdown => (
                    clock(t.countdown_remaining_ms, true),
                    "Timer",
                    t.countdown_remaining_ms as f32 / t.countdown_duration_ms.max(1) as f32,
                ),
                _ => (
                    clock(t.stopwatch_ms, false),
                    "Cronômetro",
                    (t.stopwatch_ms % 60000) as f32 / 60000.,
                ),
            };
            self.ring(x + w / 2., b + 130., 74., frac, accent());
            let size = if time.len() > 6 { 32. } else { 42. };
            self.centered_label(&time, x + w / 2., b + 99., size, Weight::Regular, white());
            self.centered_label(caption, x + w / 2., b + 151., 12., Weight::Medium, muted());
        }
        for hb in compute_hitboxes(canvas, w, h, y, s) {
            let UiAction::Timer(a) = hb.action else {
                continue;
            };
            match a {
                crate::pages::TimerAction::Mode(m) => {
                    let on = m == t.mode;
                    self.round(
                        hb.x,
                        hb.y,
                        hb.w,
                        hb.h,
                        9.,
                        if on { surface() } else { Color::TRANSPARENT },
                    );
                    let text = crate::pages::TIMER_MODES
                        .iter()
                        .find(|v| v.0 == m)
                        .unwrap()
                        .1;
                    self.centered_label(
                        text,
                        hb.x + hb.w / 2.,
                        centered_top(hb.y, hb.h, 11.),
                        11.,
                        Weight::Medium,
                        if on { white() } else { muted() },
                    );
                }
                crate::pages::TimerAction::Toggle => {
                    self.round(
                        hb.x,
                        hb.y,
                        hb.w,
                        hb.h,
                        11.,
                        if running { surface() } else { accent() },
                    );
                    self.ui_icon(
                        if running { Icon::Pause } else { Icon::Play },
                        hb.x + 21.,
                        hb.y + hb.h / 2.,
                        15.,
                        if running { white() } else { ink() },
                    );
                    self.centered_label(
                        if running { "Pausar" } else { "Iniciar" },
                        hb.x + hb.w / 2. + 10.,
                        centered_top(hb.y, hb.h, 13.),
                        13.,
                        Weight::Semibold,
                        if running { white() } else { ink() },
                    );
                }
                crate::pages::TimerAction::Reset => self.action_icon(&hb, Icon::Reset, false),
                crate::pages::TimerAction::Focus(d)
                | crate::pages::TimerAction::Break(d)
                | crate::pages::TimerAction::Duration(d)
                | crate::pages::TimerAction::In(d) => {
                    self.round(hb.x, hb.y, hb.w, hb.h, 8., surface());
                    if d > 0 {
                        self.ui_icon(Icon::Plus, hb.x + hb.w / 2., hb.y + hb.h / 2., 14., white());
                    } else {
                        self.round(hb.x + 9., hb.y + hb.h / 2. - 0.7, 12., 1.4, 0.7, white());
                    }
                }
                crate::pages::TimerAction::Preset(m) | crate::pages::TimerAction::InPreset(m) => {
                    self.round(hb.x, hb.y, hb.w, hb.h, 9., surface());
                    self.centered_label(
                        &format!("{m} min"),
                        hb.x + hb.w / 2.,
                        centered_top(hb.y, hb.h, 11.),
                        11.,
                        Weight::Medium,
                        muted(),
                    );
                }
                crate::pages::TimerAction::Field => {
                    self.round(hb.x, hb.y, hb.w, hb.h, 11., surface());
                    let shown = if t.reminder_text.is_empty() {
                        "Do que você quer lembrar?".to_owned()
                    } else {
                        format!(
                            "{}{}",
                            t.reminder_text,
                            if t.reminder_typing { "|" } else { "" }
                        )
                    };
                    self.clipped_label(
                        &shown,
                        hb.x + 12.,
                        hb.y + 10.,
                        hb.w - 24.,
                        12.,
                        Weight::Regular,
                        if t.reminder_text.is_empty() {
                            muted()
                        } else {
                            white()
                        },
                    );
                }
                crate::pages::TimerAction::Add => {
                    self.action_icon(&hb, Icon::Plus, !t.reminder_text.trim().is_empty())
                }
                crate::pages::TimerAction::Delete(id) => {
                    if let Some(r) = t.reminders.iter().find(|r| r.id == id) {
                        self.subtle_line(x + 20., hb.y + hb.h + 3., w - 40.);
                        self.clipped_label(
                            &r.text,
                            x + 24.,
                            hb.y + 4.,
                            w - 185.,
                            12.,
                            Weight::Medium,
                            white(),
                        );
                        self.clipped_label(
                            &r.when,
                            x + w - 150.,
                            hb.y + 5.,
                            88.,
                            11.,
                            Weight::Regular,
                            muted(),
                        );
                        self.action_icon(&hb, Icon::Close, false);
                    }
                }
            }
        }
        match t.mode {
            TimerMode::Pomodoro if t.pomodoro_phase == Phase::Idle => {
                for (i, (name, n)) in [("Foco", t.focus_minutes), ("Pausa", t.break_minutes)]
                    .into_iter()
                    .enumerate()
                {
                    let lx = x + w / 2. - 235. + i as f32 * 240.;
                    self.label(name, lx, b + 232., 12., Weight::Regular, muted());
                    self.label(
                        &format!("{n} min"),
                        lx + 65.,
                        b + 232.,
                        12.,
                        Weight::Medium,
                        white(),
                    );
                }
            }
            TimerMode::Countdown if !t.countdown_started => self.centered_label(
                &format!("{} minutos", t.countdown_duration_ms / 60000),
                x + w / 2.,
                b + 231.,
                12.,
                Weight::Medium,
                muted(),
            ),
            TimerMode::Reminders => {
                self.label(
                    &format!("Daqui a {} min", t.reminder_minutes),
                    x + 20.,
                    b + 103.,
                    12.,
                    Weight::Regular,
                    muted(),
                );
                if t.reminders.is_empty() {
                    self.label(
                        "Nenhum lembrete pendente",
                        x + 24.,
                        b + 161.,
                        13.,
                        Weight::Regular,
                        muted(),
                    );
                } else if t.reminders.len() > crate::pages::REMINDER_ROWS {
                    self.label(
                        &format!("e mais {}", t.reminders.len() - crate::pages::REMINDER_ROWS),
                        x + 24.,
                        y + h - 27.,
                        10.,
                        Weight::Regular,
                        muted(),
                    );
                }
            }
            _ => {}
        }
    }
}
