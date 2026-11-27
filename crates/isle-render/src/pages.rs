// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Adapted for isle-win from Mac Island & Dock by caliberda v3.1.0 (ControlPanel::Layout/HitTest):
// shared Rust tile, slider and NotificationPanel hitboxes.
// TrayListPanel/pinned tray layout adapted from Mac Island & Dock v3.1.1.

use isle_core::agenda::{civil_from_days, days_from_civil, weekday};
use isle_core::state::{ControlTile, IslandState, PermissionDecision, PillMode, TabPage};
use isle_core::tray::{TRAY_PER_PAGE, inline_indices, overflow_indices};

/// The media buttons, as the pill draws them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKey {
    Previous,
    PlayPause,
    Next,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAction {
    AiOpen,
    AiBack,
    AiInput,
    AiSend,
    AiStop,
    AiNew,
    AiProvider(isle_core::state::AiProviderKind),
    /// The command the assistant asks to run: Executar (Enter) or Negar (Esc).
    AiRun,
    AiDeny,
    AiScroll(i32),
    /// One of the assistant's ideas under its empty field (`chat::IDEAS`).
    AiIdea(usize),
    ReadNotification(i64, i64),
    /// Avisos: open the reply box under this notification, and send what was typed.
    ReplyNotification(i64, i64),
    SendReply,
    ClearNotifications,
    NotificationsPage(usize),
    Media(MediaKey),
    Control(ControlTile),
    SetVolume(u8),
    ToggleMute,
    ToggleMic,
    AppVolume(usize, u8),
    AppMute(usize),
    OpenMixer,
    SetOutput(usize),
    /// Agenda: pick a day (local days since 1970), or move the month by this much.
    AgendaDay(i64),
    AgendaMonth(i32),
    SwitchTab(TabPage),
    /// The temperature on the compact bar: opens the weather panel. A day in it.
    OpenWeather,
    WeatherDay(usize),
    /// The Timer tab.
    Timer(TimerAction),
    /// The answer, and the key of the card it was on ([`isle_core::state::PermissionCard::key`]).
    Permission(PermissionDecision, u64),
    /// The card itself, off its buttons: the agent's window comes forward, the card stays.
    PermissionFocus(u64),
    ToggleCollapse,
    /// The settings window (the gear on the open island).
    OpenSettings,
    TrayClick(usize, bool),
    ToggleTray,
    TrayPage(usize),
    Phase8(crate::phase8_view::Phase8Action),
    /// The power button (pill or control center), one of its menu's actions, and its confirmation.
    OpenPower,
    Power(isle_core::power::PowerAction),
    PowerConfirm,
    PowerCancel,
    /// The control center's edit mode (mac-island's Editar): on and off, a control's "−", one from
    /// "Adicionar controles", and a control held to be dragged elsewhere (its index).
    EditControls,
    ControlRemove(ControlTile),
    ControlAdd(ControlTile),
    ControlGrab(usize),
}

#[derive(Debug, Clone, Copy)]
pub struct HitBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub action: UiAction,
}

impl HitBox {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

/// How far below the pill's top the media buttons sit.
pub const MEDIA_BUTTONS_TOP: f32 = 150.0;

thread_local! {
    /// Measures the bar's texts for its hitboxes (the renderer measures with its own).
    pub(crate) static BAR_TEXT: crate::text::Text = crate::text::Text::new();
}

pub fn compact_media_keys(state: &IslandState) -> &'static [MediaKey] {
    if !state.ui.media_controls || !state.modules.media || state.media.is_none() {
        &[]
    } else if state.ui.media_play_only {
        &[MediaKey::PlayPause]
    } else {
        &[MediaKey::Previous, MediaKey::PlayPause, MediaKey::Next]
    }
}

pub fn compact_media_contains(state: &IslandState, canvas: f32, width: f32, x: f32) -> bool {
    use isle_core::config::BarItem;
    BAR_TEXT.with(|text| {
        crate::layout::compact_slots(text, (canvas - width) / 2., width, state).into_iter().any(|(item, left, room)| {
            item == BarItem::Media && (left..=left + room).contains(&x)
        })
    })
}

pub fn compact_marquee_needed(state: &IslandState, width: f32) -> bool {
    if !state.ui.media_marquee { return false; }
    let Some(title) = crate::layout::compact_media_text(state) else { return false };
    BAR_TEXT.with(|font| {
        let text_width = font.width(title, 12., crate::text::Weight::Medium);
        crate::layout::compact_slots(font, 0., width, state).iter().any(|(item, _, room)| {
            use isle_core::config::BarItem;
            let keys = compact_media_keys(state).len() as f32 * 23.;
            let room = room - keys;
            let art = if room > 48. { 26. } else { 0. };
            *item == BarItem::Media && text_width > room - art
        })
    })
}

/// The closed pill's width: what its items ask for (`layout::compact_content_width`), at least "Largura
/// mínima da ilha fechada".
pub fn compact_tray_width(state: &IslandState) -> f32 {
    let content = BAR_TEXT.with(|text| crate::layout::compact_content_width(text, state));
    content.max(state.ui.pill_width as f32).min(crate::window::CANVAS_MAX_WIDTH as f32 - 8.)
}

/// What the Timer tab's controls do (start/pause and reset act on the mode shown).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerAction {
    Mode(isle_core::timers::TimerMode),
    Toggle,
    Reset,
    /// Minutes more or less: the Pomodoro's focus and break, the countdown's length.
    Focus(i32),
    Break(i32),
    Duration(i32),
    /// The countdown set to this many minutes.
    Preset(u32),
    /// Reminders: the text field, add, in how many minutes (steps, presets), remove one.
    Field,
    Add,
    In(i32),
    InPreset(u32),
    Delete(u64),
}

/// A reminder's quick delays (minutes).
pub const REMINDER_PRESETS: [u32; 3] = [15, 30, 60];
/// Pending reminders listed on the tab.
pub const REMINDER_ROWS: usize = 4;

pub const TIMER_MODES: [(isle_core::timers::TimerMode, &str); 4] = [
    (isle_core::timers::TimerMode::Pomodoro, "Pomodoro"),
    (isle_core::timers::TimerMode::Countdown, "Timer"),
    (isle_core::timers::TimerMode::Stopwatch, "Cronômetro"),
    (isle_core::timers::TimerMode::Reminders, "Lembretes"),
];
/// The countdown's quick lengths (minutes).
pub const TIMER_PRESETS: [u32; 4] = [5, 10, 15, 30];
/// The Timer tab's ring: centre (from the pill's left and top) and radius.
pub const TIMER_RING: (f32, f32, f32) = (140.0, 204.0, 76.0);

/// The power button's room on the compact bar.
pub const POWER_W: f32 = 26.0;
/// The temperature's room on the compact bar (icon and "24°"), left of the tray icons.
pub const WEATHER_W: f32 = 64.0;
/// The weather panel's size, and where its seven days sit (the renderer draws them there).
pub const WEATHER_PANEL: (f32, f32, f32) = crate::layout::PANEL;
pub const WEATHER_DAYS_TOP: f32 = 164.0;
pub const WEATHER_DAY_H: f32 = 76.0;

pub fn tray_page(state: &IslandState) -> usize {
    state.tray_page.min(overflow_indices(&state.tray).len().saturating_sub(1) / TRAY_PER_PAGE)
}

pub fn compute_hitboxes(
    canvas_width: f32,
    pill_w: f32,
    pill_h: f32,
    top_gap: f32,
    state: &IslandState,
) -> Vec<HitBox> {
    let left = (canvas_width - pill_w) * 0.5;
    let top = top_gap;
    let mut boxes = Vec::new();

    match &state.mode {
        PillMode::PermissionPrompt => {
            let card = state.pending_permission.as_ref().map_or(0, |card| card.key());
            let btn_y = top + pill_h - 36.0;
            let btn_w = 88.0;
            let btn_h = 26.0;
            let spacing = 12.0;
            let total_w = btn_w * 3.0 + spacing * 2.0;
            let start_x = left + (pill_w - total_w) * 0.5;

            boxes.push(HitBox {
                x: start_x,
                y: btn_y,
                w: btn_w,
                h: btn_h,
                action: UiAction::Permission(PermissionDecision::Allow, card),
            });
            boxes.push(HitBox {
                x: start_x + btn_w + spacing,
                y: btn_y,
                w: btn_w,
                h: btn_h,
                action: UiAction::Permission(PermissionDecision::Always, card),
            });
            boxes.push(HitBox {
                x: start_x + (btn_w + spacing) * 2.0,
                y: btn_y,
                w: btn_w,
                h: btn_h,
                action: UiAction::Permission(PermissionDecision::Deny, card),
            });
            // After the buttons, so they win where they are.
            boxes.push(HitBox { x: left, y: top, w: pill_w, h: pill_h, action: UiAction::PermissionFocus(card) });
        }
        PillMode::Expanded(_) => {
            return crate::layout::viewport_hitboxes(canvas_width, pill_w, pill_h, top_gap, state);
        }
        // The line comes back under the mouse (hover), it has nothing to click.
        PillMode::Minimized => {}
        PillMode::CompactBar | PillMode::AlertBanner { .. } => {
            if state.mode == PillMode::CompactBar {
                use isle_core::config::BarItem;
                let slots = BAR_TEXT.with(|t| crate::layout::compact_slots(t, left, pill_w, state));
                let y = top + (pill_h - 24.0) / 2.0;
                for (item, x, w) in slots {
                    let action = match item {
                        BarItem::Tray => {
                            for (n, i) in inline_indices(&state.tray).into_iter().enumerate() {
                                boxes.push(HitBox { x: x + n as f32 * 26.0, y, w: 24.0, h: 24.0, action: UiAction::TrayClick(i, false) });
                            }
                            boxes.push(HitBox { x: x + w - 24.0, y, w: 24.0, h: 24.0, action: UiAction::ToggleTray });
                            continue;
                        }
                        BarItem::Media => {
                            let keys = compact_media_keys(state);
                            for (i, key) in keys.iter().enumerate() {
                                boxes.push(HitBox {
                                    x: x + w - (keys.len() - i) as f32 * 23., y, w: 22., h: 24.,
                                    action: UiAction::Media(*key),
                                });
                            }
                            // The song itself opens the Mídia tab (mac-island: IslandItem::Media → m_mediaPanel).
                            let title = w - keys.len() as f32 * 23.;
                            if state.modules.media && title > 4.0 {
                                boxes.push(HitBox { x, y, w: title - 4.0, h: 24.0, action: UiAction::SwitchTab(isle_core::state::TabPage::Media) });
                            }
                            continue;
                        }
                        // The clock, the date and the next event open the calendar (mac-island: IslandItem::Clock → m_calendar).
                        BarItem::Clock | BarItem::Date | BarItem::Next if state.modules.agenda => {
                            UiAction::SwitchTab(isle_core::state::TabPage::CalendarWeather)
                        }
                        BarItem::Weather => UiAction::OpenWeather,
                        BarItem::Notifications => UiAction::SwitchTab(isle_core::state::TabPage::Notifications),
                        BarItem::Timer => UiAction::SwitchTab(isle_core::state::TabPage::Timer),
                        BarItem::Power => UiAction::OpenPower,
                        _ => continue,
                    };
                    boxes.push(HitBox { x, y, w: w - 4.0, h: 24.0, action });
                }
            }
            boxes.push(HitBox {
                x: left,
                y: top,
                w: pill_w,
                h: pill_h,
                action: UiAction::ToggleCollapse,
            });
        }
    }

    boxes
}

/// Where the Agenda tab's month is: shared by the hitboxes and the renderer.
pub struct AgendaGrid {
    pub x0: f32,
    pub y0: f32,
    pub cell_w: f32,
    pub cell_h: f32,
    /// The day in the first cell (the Sunday on or before the 1st).
    pub first_cell: i64,
    pub year: i64,
    pub month: i64,
}

/// The day shown: the one picked, else today.
pub fn agenda_selected(state: &IslandState) -> i64 {
    if state.agenda_day != 0 { state.agenda_day } else { state.today }
}

pub fn agenda_grid(left: f32, top: f32, pill_w: f32, state: &IslandState) -> AgendaGrid {
    let (year, month, _) = civil_from_days(agenda_selected(state));
    let first = days_from_civil(year, month, 1);
    let (inset, right) = if state.ui.agenda_featured_day {
        ((pill_w * 0.4).min(196.0), 18.0)
    } else {
        (24.0, 24.0)
    };
    AgendaGrid { x0: left + inset, y0: top + crate::layout::BODY + 48.0,
        cell_w: (pill_w - inset - right) / 7.0,
        cell_h: 21.0,
        first_cell: first - weekday(first),
        year,
        month,
    }
}

pub fn slider_percent(x: f32, left: f32, width: f32) -> u8 {
    ((x - left) / width * 100.0).round().clamp(0.0, 100.0) as u8
}

#[cfg(test)]
mod weather_tests {
    use super::*;
    fn forecast() -> isle_core::weather::Weather {
        isle_core::weather::Weather::from_open_meteo("Rio", include_str!("../examples/open-meteo-sample.json"),
        ).unwrap()
    }
    #[test]
    fn the_temperature_opens_the_panel_only_with_a_forecast_and_seven_days_fit_inside() {
        let mut state = IslandState::default();
        assert!(!compute_hitboxes(580.0, 220.0, 34.0, 6.0, &state).iter().any(|h| h.action == UiAction::OpenWeather));
        state.weather = Some(forecast());
        let w = compact_tray_width(&state);
        assert!(compute_hitboxes(580.0, w, 34.0, 6.0, &state).iter().any(|h| h.action == UiAction::OpenWeather));
        state.mode = PillMode::Expanded(TabPage::Weather);
        let (pw, ph, _) = WEATHER_PANEL;
        let left = (580.0 - pw) * 0.5;
        let days: Vec<_> = compute_hitboxes(580.0, pw, ph, 6.0, &state).into_iter().filter(|h| matches!(h.action, UiAction::WeatherDay(_))).collect();
        assert_eq!(days.len(), 7);
        for d in &days { assert!(d.x >= left + 18.0 - 0.01 && d.x + d.w <= left + pw - 18.0 + 0.01 && d.y + d.h < 6.0 + ph); }
    }
}

#[cfg(test)]
mod compact_click_tests {
    use super::*;
    use isle_core::config::BarItem;
    #[test]
    fn the_clock_date_and_song_open_their_own_tabs() {
        let mut state = IslandState::default();
        state.bar = vec![BarItem::Clock, BarItem::Date, BarItem::Media];
        state.clock_text = "19:20".into();
        state.date_text = "ter 17 nov 2026".into();
        state.media = Some(isle_core::state::MediaInfo { title: "Circuito".into(), playing: true, ..Default::default() });
        let w = 520.0;
        let boxes = compute_hitboxes(580.0, w, 34.0, 6.0, &state);
        let slots = BAR_TEXT.with(|t| crate::layout::compact_slots(t, (580.0 - w) / 2., w, &state));
        let action_at = |x: f32| boxes.iter().find(|b| b.contains(x, 6.0 + 17.0)).map(|b| b.action.clone());
        for (item, x, _) in slots {
            let want = match item {
                BarItem::Clock | BarItem::Date => UiAction::SwitchTab(TabPage::CalendarWeather),
                BarItem::Media => UiAction::SwitchTab(TabPage::Media),
                _ => continue,
            };
            assert_eq!(action_at(x + 2.0), Some(want), "{item:?}");
        }
    }
}

#[cfg(test)]
mod control_tests {
    use super::*;
    #[test]
    fn compact_tray_buttons_precede_background_and_fit_the_pill() {
        let mut state = IslandState::default();
        state.tray = (0..3).map(|i| isle_core::tray::TrayItem {
            key: i.to_string(), pinned: true, ..Default::default()
        }).collect();
        let w = compact_tray_width(&state);
        let boxes = compute_hitboxes(580.0, w, 34.0, 6.0, &state);
        for i in 0..3 {
            let hb = boxes.iter().find(|h| h.action == UiAction::TrayClick(i, false)).unwrap();
            assert!(hb.x >= (580.0-w)/2.0 && hb.x+hb.w <= (580.0+w)/2.0);
            assert_eq!(boxes.iter().find(|h| h.contains(hb.x+12.0,hb.y+12.0)).unwrap().action, hb.action);
        }
        assert!(boxes.iter().any(|h| h.action == UiAction::ToggleTray));
        state.mode = PillMode::PermissionPrompt;
        assert!(!compute_hitboxes(580.0,440.0,130.0,6.0,&state).iter().any(|h| matches!(h.action,UiAction::TrayClick(..)|UiAction::ToggleTray)));
    }

    #[test]
    fn overflow_is_paged_and_clamps_after_removal() {
        let mut state = IslandState::default();
        state.mode = PillMode::Expanded(TabPage::Tray);
        state.tray = (0..25).map(|i| isle_core::tray::TrayItem {
            key: i.to_string(), pinned: i < 2, ..Default::default()
        }).collect();
        state.tray_page = 2;
        let boxes = compute_hitboxes(720.0, 600.0, 470.0,6.0,&state);
        let indices: Vec<_> = boxes.iter().filter_map(|h| {
                if let UiAction::TrayClick(i,false) = h.action { Some(i) } else { None
                }
            }).collect();
        assert_eq!(indices, [22,23,24]);
        assert!(!boxes.iter().any(|h| h.action == UiAction::TrayPage(3)));
        state.tray.truncate(4);
        let boxes = compute_hitboxes(720.0, 600.0, 470.0,6.0,&state);
        assert!(boxes.iter().any(|h| h.action == UiAction::TrayClick(2,false)));
        assert!(!boxes.iter().any(|h| matches!(h.action,UiAction::TrayPage(_))));
    }
    #[test]
    fn notification_rows_have_stable_ids_and_pagination_stays_above_the_tray() {
        let mut state = IslandState::default();
        state.mode = PillMode::Expanded(TabPage::Notifications);
        state.notifications.receive((1..=8).map(|id| isle_core::notifications::Notification {
            id, arrival_ms: id * 1000, app_id: "Mail".into(), ..Default::default()
        }).collect(), false,
        );
        let boxes = compute_hitboxes(720.0, 600.0, 470.0, 6.0, &state);
        let rows: Vec<_> = boxes.iter().filter(|b| matches!(b.action, UiAction::ReadNotification(..))).collect();
        assert_eq!(rows.len(),4);
        assert_eq!(rows[0].action, UiAction::ReadNotification(8,8000));
        let pager = boxes.iter().find(|b| b.action == UiAction::NotificationsPage(1)).unwrap();
        assert!(!boxes.iter().any(|b| b.action == UiAction::NotificationsPage(usize::MAX)));
        // Rows above the pager, the pager inside the 360 px pill (top gap 6).
        for row in &rows { assert!(row.y >= 76.0 && row.y + row.h <= pager.y); }
        assert!(pager.y + pager.h <= 6.0 + 470.0 - 4.0);
        // Past the end clamps to the last page: the other four, nothing further.
        state.notification_page = 2;
        let boxes = compute_hitboxes(720.0, 600.0, 470.0,6.0,&state);
        assert_eq!(boxes.iter().filter(|b| matches!(b.action,UiAction::ReadNotification(..))).count(),4);
        assert!(!boxes.iter().any(|b| b.action == UiAction::NotificationsPage(2)));
    }
    #[test]
    fn control_center_has_ten_tiles_and_two_mixer_rows_above_tray() {
        let mut state = IslandState::default();
        state.mode = PillMode::Expanded(TabPage::ControlCenter);
        state.audio_available = true; state.mic_available = true;
        state.app_volumes = (0..4).map(|i| isle_core::state::AppVolume {
            id: i.to_string(), name: "App".into(), volume_pct: 50, muted: false,
        }).collect();
        let boxes = compute_hitboxes(720.0, 600.0, 470.0, 6.0, &state);
        assert_eq!(boxes.iter().filter(|h| matches!(h.action, UiAction::Control(_))).count(), 10);
        assert_eq!(boxes.iter().filter(|h| matches!(h.action, UiAction::AppVolume(_, _))).count(),
            4
        );
        let controls: Vec<_> = boxes.iter().filter(|h| {
                !matches!(h.action, UiAction::SwitchTab(_) | UiAction::ToggleCollapse | UiAction::TrayClick(..)
                )
            }).collect();
        for (i,a) in controls.iter().enumerate() {
            assert!(a.y >= 6.0 && a.y + a.h < 470.0, "{a:?}");
            for b in &controls[i+1..] {
                assert!(a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y, "overlap {a:?}, {b:?}");
            }
        }
    }
    #[test]
    fn every_output_gets_a_chip_and_the_panel_still_fits() {
        let mut state = IslandState::default();
        state.mode = PillMode::Expanded(TabPage::ControlCenter);
        state.audio_available = true; state.mic_available = true;
        state.app_volumes = (0..4).map(|i| isle_core::state::AppVolume {
            id: i.to_string(), name: "App".into(), volume_pct: 50, muted: false,
        }).collect();
        let out = |i: usize| isle_core::state::AudioOutput { id: i.to_string(), name: format!("Saída {i}"), is_default: i == 0,
        };
        for (outputs, chips, app_rows) in [(1, 0, 4), (4, 4, 3), (7, 7, 2), (12, 8, 2)] {
            state.audio_outputs = (0..outputs).map(out).collect();
            let (pw, ph, _) = crate::layout::PANEL;
            let boxes = compute_hitboxes(720.0, pw, ph, 6.0, &state);
            assert_eq!(boxes.iter().filter(|h| matches!(h.action, UiAction::SetOutput(_))).count(), chips, "{outputs} outputs");
            let rows = boxes.iter().filter(|h| matches!(h.action, UiAction::AppVolume(..))).count();
            assert!(rows >= app_rows && rows <= 4, "{outputs} outputs: {rows} app rows");
            let controls: Vec<_> = boxes.iter().filter(|h| !matches!(h.action, UiAction::SwitchTab(_) | UiAction::ToggleCollapse)).collect();
            for (i, a) in controls.iter().enumerate() {
                assert!(a.y + a.h <= 6.0 + crate::layout::PANEL.1, "{a:?} leaves the panel");
                for b in &controls[i + 1..] {
                    assert!(a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y, "overlap {a:?}, {b:?}");
                }
            }
        }
    }
    #[test]
    fn agenda_grid_has_one_cell_per_day_of_the_month_inside_the_panel() {
        use isle_core::agenda::days_from_civil;
        let mut state = IslandState::default();
        state.mode = PillMode::Expanded(TabPage::CalendarWeather);
        state.today = days_from_civil(2026, 11, 11);
        state.agenda_day = days_from_civil(2026, 2, 10);
        let boxes = compute_hitboxes(720.0, 600.0, 470.0, 6.0, &state);
        let days: Vec<i64> = boxes.iter().filter_map(|h| match h.action { UiAction::AgendaDay(d) => Some(d), _ => None,
            }).collect();
        assert_eq!(days.len(), 28);
        assert_eq!(days[0], days_from_civil(2026, 2, 1));
        assert_eq!(boxes.iter().filter(|h| matches!(h.action, UiAction::AgendaMonth(_))).count(), 2);
        let mine: Vec<_> = boxes.iter().filter(|h| !matches!(h.action, UiAction::SwitchTab(_) | UiAction::ToggleCollapse | UiAction::OpenSettings | UiAction::AiOpen)).collect();
        for (i, a) in mine.iter().enumerate() {
            assert!(a.y >= 40.0 && a.y + a.h <= 350.0 && a.x >= 60.0 && a.x + a.w <= 660.0, "{a:?}");
            for b in &mine[i + 1..] {
                assert!(a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y, "overlap {a:?}, {b:?}");
            }
        }
    }
    #[test]
    fn expanded_tabs_have_no_pinned_tray_buttons() {
        let mut state = IslandState::default();
        for tab in [TabPage::ControlCenter, TabPage::Media, TabPage::AiHub] {
            state.mode = PillMode::Expanded(tab);
            assert!(!compute_hitboxes(720.0, 600.0, 470.0, 6.0, &state).iter().any(|h| matches!(h.action, UiAction::TrayClick(..))));
        }
    }
    #[test]
    fn slider_clamps_drag_and_scales_endpoints_to_percent() {
        assert_eq!(slider_percent(49.0, 50.0, 200.0), 0);
        assert_eq!(slider_percent(150.0, 50.0, 200.0), 50);
        assert_eq!(slider_percent(250.0, 50.0, 200.0), 100);
        assert_eq!(slider_percent(999.0, 50.0, 200.0), 100);
    }
}
