//! Cimori geometry. Drawing and native hit testing consume the same rectangles.
use crate::pages::{
    HitBox, MediaKey, REMINDER_PRESETS, REMINDER_ROWS, TIMER_MODES, TIMER_PRESETS, TimerAction,
    UiAction,
};
use isle_core::state::{IslandState, PillMode, TabPage};
use isle_core::timers::TimerMode;
use isle_core::tray::{TRAY_PER_PAGE, overflow_indices};

pub const BODY: f32 = 94.0;
pub const TABS: [TabPage; 12] = TabPage::NAV;
pub fn fit_size(tab:TabPage,available_w:f32,available_h:f32)->(f32,f32,f32){let(w,h,r)=expanded_size(tab);(w.min(available_w.max(320.)),h.min(available_h.max(220.)),r)}
pub fn natural_height(s:&IslandState,viewport:f32)->f32{if s.mode==PillMode::Expanded(TabPage::AiHub)&&s.chat.open{viewport}else if let PillMode::Expanded(t)=s.mode{content_height(t,s).max(viewport)}else{viewport}}
/// How tall a tab's content is: its size, or the control center's list while it is edited.
pub fn content_height(tab: TabPage, s: &IslandState) -> f32 {
    use isle_core::state::ControlTile;
    let (_, h, _) = expanded_size(tab);
    if tab == TabPage::ControlCenter && s.control_editing {
        let missing = missing_controls(s).len();
        (control_add_top(s.control_tiles.len()) + 30.0 + missing.div_ceil(3) as f32 * 38.0 + 12.0).max(h)
    } else if tab == TabPage::AiHub && !s.chat.open {
        usage_height(s).max(h)
    } else if tab == TabPage::ControlCenter {
        // More controls than the ten it was made for: the sound below them scrolls into view.
        h + (control_tiles_end(s.control_tiles.len()) - control_tiles_end(ControlTile::ALL.len())).max(0.0)
    } else {
        h
    }
}

/// Where the control center's tiles end (from the panel's top), for `n` of them: the first two big,
/// the rest four to a row.
pub fn control_tiles_end(n: usize) -> f32 {
    let rows = n.saturating_sub(2).div_ceil(4);
    match (n, rows) {
        (0, _) => BODY - 11.0,
        (_, 0) => BODY + 58.0,
        _ => BODY + 68.0 + (rows - 1) as f32 * 54.0 + 48.0,
    }
}

/// Edit mode: where "Adicionar controles" starts, under the tiles.
pub fn control_add_top(n: usize) -> f32 {
    control_tiles_end(n) + 18.0
}

/// The catalog's controls not in the center, in the catalog's order (mac-island's "Adicionar controles").
pub fn missing_controls(s: &IslandState) -> Vec<isle_core::state::ControlTile> {
    isle_core::state::ControlTile::CATALOG.into_iter().filter(|t| !s.control_tiles.contains(t)).collect()
}

/// The header's "Editar" / "Concluído" button on the control center.
pub fn edit_button(left: f32, top: f32) -> HitBox {
    // The header's left, free since the clock moved right with isle's status.
    HitBox { x: left + 20.0, y: top + 9.0, w: 86.0, h: 28.0, action: UiAction::EditControls }
}
pub fn footer_height(s:&IslandState)->f32{match s.mode{PillMode::Expanded(TabPage::AiHub)=>if s.chat.open{104.}else{0.},PillMode::Expanded(TabPage::Timer)=>if s.timers.mode==TimerMode::Reminders{24.}else{64.},PillMode::Expanded(TabPage::Media)=>140.,PillMode::Expanded(TabPage::Notifications|TabPage::Tray)=>52.,PillMode::Expanded(TabPage::Clipboard|TabPage::Shelf|TabPage::Usb|TabPage::Downloads)=>68.,_=>0.}}
pub fn scroll_top(s:&IslandState)->f32{if s.mode==PillMode::Expanded(TabPage::Timer)&&s.timers.mode==TimerMode::Reminders{BODY+140.}else{BODY}}
pub fn viewport_hitboxes(canvas:f32,w:f32,h:f32,top:f32,s:&IslandState)->Vec<HitBox>{
    let natural=natural_height(s,h);let footer=footer_height(s);let prefix=scroll_top(s);let offset=s.panel_scroll.clamp(0.,(natural-h).max(0.));
    expanded_hitboxes(canvas,w,natural,top,s).into_iter().filter_map(|mut b|{
        if natural>h+0.1 {
            if b.y>=top+natural-footer {b.y-=natural-h;}
            else if b.y>=top+prefix {b.y-=offset;let end=(b.y+b.h).min(top+h-footer);b.y=b.y.max(top+prefix);b.h=end-b.y;}
        }
        if b.h<=0.||b.y<top||b.y+b.h>top+h+0.1{None}else{Some(b)}
    }).collect()
}
/// Every tab's panel: one size, so going from tab to tab does not resize the island.
pub const PANEL: (f32, f32, f32) = (620.0, 470.0, 28.0);

pub fn expanded_size(tab: TabPage) -> (f32, f32, f32) {
    match tab {
        // The power menu (from its button, not a tab): as wide, shorter.
        TabPage::Power => (PANEL.0, 250.0, PANEL.2),
        _ => PANEL,
    }
}
/// The tabs whose module is on, in the navigation's order.
pub fn visible_tabs(m: &isle_core::config::Modules) -> Vec<TabPage> {
    TABS.into_iter()
        .filter(|t| match t {
            TabPage::AiHub => m.ai,
            TabPage::ControlCenter => m.control,
            TabPage::CalendarWeather => m.agenda,
            TabPage::Weather => m.weather,
            TabPage::Media => m.media,
            TabPage::Notifications => m.notifications,
            TabPage::Timer => m.timer,
            TabPage::Tray => m.tray,
            TabPage::Shelf => m.shelf,
            TabPage::Clipboard => m.clipboard,
            TabPage::Usb => m.usb,
            TabPage::Downloads => m.downloads,
            TabPage::Power => false,
        })
        .collect()
}

/// Port of ExpandedView._buildTabs (isle): icons in a row from the left, 2 px apart, the open one in a pill
/// with its name (.isle-tab: 4 px by 10, the name 13 px semibold 6 px from the icon). Narrower when they
/// would not fit, then without the name.
pub fn navigation(left: f32, top: f32, w: f32, m: &isle_core::config::Modules, open: Option<TabPage>) -> Vec<HitBox> {
    let tabs = visible_tabs(m);
    let name = open.filter(|t| tabs.contains(t)).map_or(0., |t| crate::pages::BAR_TEXT.with(|text| text.width(t.label(), 13., crate::text::Weight::Semibold)) + 6.);
    let width = |pad: f32, name: f32| tabs.len() as f32 * (16. + pad * 2. + 2.) + name;
    let (pad, name) = [(10., name), (7., name), (7., 0.)].into_iter().find(|(p, n)| width(*p, *n) <= w - 40.).unwrap_or((7., 0.));
    let mut x = left + 20.;
    tabs.into_iter().map(|t| {
        let tw = 16. + pad * 2. + if Some(t) == open { name } else { 0. };
        let hb = HitBox { x, y: top + 47., w: tw, h: 26., action: UiAction::SwitchTab(t) };
        x += tw + 2.;
        hb
    }).collect()
}
/// The tabs' side padding (10, or 7 when narrowed): the narrowest one is an icon and its padding.
pub fn tab_pad(nav: &[HitBox]) -> f32 {
    ((nav.iter().map(|b| b.w).fold(f32::MAX, f32::min) - 16.) / 2.).min(10.)
}
/// The tab lit now: the page open, none while the assistant (the ✦) is.
pub fn open_tab(s: &IslandState) -> Option<TabPage> {
    match s.mode { PillMode::Expanded(TabPage::AiHub) if s.chat.open => None, PillMode::Expanded(t) => Some(t), _ => None }
}
/// Port of ExpandedView._buildHeader's status (isle): the ✦ (assistant), the date, the time in bold, then the
/// gear (Cimori's collapse arrow after it). The ✦ hit box, and where the date and the time start.
pub fn header_status(left: f32, top: f32, w: f32, s: &IslandState) -> (Option<HitBox>, f32, f32) {
    crate::pages::BAR_TEXT.with(|text| {
        let time_x = left + w - 92. - text.width(&s.clock_text, 13., crate::text::Weight::Semibold);
        let date_x = time_x - 8. - text.width(&s.date_text, 13., crate::text::Weight::Regular);
        let star = s.modules.ai.then_some(HitBox { x: date_x - 38., y: top + 9., w: 28., h: 28., action: UiAction::AiOpen });
        (star, date_x, time_x)
    })
}
/// Port of UsagePage._content (isle), every provider stacked: the head (20), 14 apart, then each limit
/// (its line and bar, 27) 11 apart; the providers 24 apart.
pub const USAGE_HEAD: f32 = 20.;
pub const USAGE_LIMIT: f32 = 38.;
pub const USAGE_GAP: f32 = 24.;
pub fn usage_block(windows: usize) -> f32 { USAGE_HEAD + 14. + windows.max(1) as f32 * USAGE_LIMIT - 11. }
pub fn usage_height(s: &IslandState) -> f32 {
    BODY + 14. + s.usage.iter().map(|p| usage_block(p.windows.len()) + USAGE_GAP).sum::<f32>() + 20.
}
pub fn expanded_hitboxes(canvas: f32, w: f32, h: f32, top: f32, s: &IslandState) -> Vec<HitBox> {
    let PillMode::Expanded(tab) = s.mode else {
        return vec![];
    };
    let left = (canvas - w) * 0.5;
    let mut boxes = navigation(left, top, w, &s.modules, open_tab(s));
    boxes.extend(header_status(left, top, w, s).0);
    let mut add = |x, y, bw, bh, a| {
        boxes.push(HitBox {
            x: left + x,
            y: top + y,
            w: bw,
            h: bh,
            action: a,
        })
    };
    add(w - 50.0, 9.0, 30.0, 28.0, UiAction::ToggleCollapse);
    add(w - 84.0, 9.0, 30.0, 28.0, UiAction::OpenSettings);
    match tab {
        TabPage::AiHub if s.chat.open => boxes.extend(crate::chat::hitboxes(left, top, w, h, &s.chat)),
        TabPage::AiHub => {}
        TabPage::ControlCenter => {
            let e = edit_button(0.0, 0.0);
            add(e.x, e.y, e.w, e.h, UiAction::EditControls);
            let cw = (w - 48.0) / 2.0;
            // The first two buttons are the big ones, the rest four to a row.
            for (i, t) in s.control_tiles.iter().copied().enumerate() {
                let (x, y, bw, bh) = if i < 2 {
                    (20.0 + i as f32 * (cw + 8.0), BODY, cw, 58.0)
                } else {
                    let j = i - 2;
                    let col = (w - 40.0) / 4.0;
                    (
                        20.0 + (j % 4) as f32 * col,
                        BODY + 68.0 + (j / 4) as f32 * 54.0,
                        col - 6.0,
                        48.0,
                    )
                };
                if s.control_editing {
                    // Port of ControlPanel's edit mode (mac-island 3.1.1): a "−" on each control (found
                    // first), the control itself held to drag it elsewhere.
                    add(x + 3.0, y + 3.0, 20.0, 20.0, UiAction::ControlRemove(t));
                    add(x, y, bw, bh, UiAction::ControlGrab(i));
                } else {
                    add(x, y, bw, bh, UiAction::Control(t));
                }
            }
            let n = s.control_tiles.len();
            let tiles_end = control_tiles_end(n);
            if s.control_editing {
                let top = control_add_top(n) + 30.0;
                let col = (w - 40.0) / 3.0;
                for (j, t) in missing_controls(s).into_iter().enumerate() {
                    add(20.0 + (j % 3) as f32 * col, top + (j / 3) as f32 * 38.0, col - 6.0, 32.0, UiAction::ControlAdd(t));
                }
                return boxes;
            }
            let vy = tiles_end + 11.0;
            let show_mic = s.mic_available && s.ui.mic_button;
            if s.audio_available {
                let mut right = 20.0;
                if s.ui.mic_button { right += 42.0; }
                if s.ui.volume_percent { right += 43.0; }
                add(20.0, vy, 34.0, 34.0, UiAction::ToggleMute);
                add(67.0, vy, w - 67.0 - right, 34.0, UiAction::SetVolume(0));
            }
            if show_mic {
                add(w - 54.0, vy, 34.0, 34.0, UiAction::ToggleMic);
            }
            let outputs = if s.control_outputs && s.audio_outputs.len() > 1 {
                s.audio_outputs.len().min(8)
            } else {
                0
            };
            let chip = (w - 40.0) / 4.0;
            for i in 0..outputs {
                add(
                    20.0 + (i % 4) as f32 * chip,
                    vy + 41.0 + (i / 4) as f32 * 27.0,
                    chip - 6.0,
                    24.0,
                    UiAction::SetOutput(i),
                );
            }
            let by = vy + 40.0 + outputs.div_ceil(4) as f32 * 27.0;
            if s.control_mixer {
                add(w - 54.0, by, 34.0, 24.0, UiAction::OpenMixer);
            }
            let capacity = if s.control_mixer { ((h - 15.0 - by - 30.0) / 27.0).floor().max(0.0) as usize } else { 0 };
            for i in 0..s.app_volumes.len().min(capacity) {
                let y = by + 28.0 + i as f32 * 27.0;
                add(156.0, y, 26.0, 24.0, UiAction::AppMute(i));
                let right = if s.ui.volume_percent { 74.0 } else { 20.0 };
                add(192.0, y, w - 192.0 - right, 24.0, UiAction::AppVolume(i, 0));
            }
        }
        TabPage::Weather => {
            let dw = (w - 40.0) / 7.0;
            let day_height = if s.ui.weather_cards { 72.0 } else { 66.0 };
            for i in 0..s.weather.as_ref().map_or(0, |v| v.days.len().min(7)) {
                add(
                    20.0 + i as f32 * dw,
                    BODY + 174.0,
                    dw - 5.0,
                    day_height,
                    UiAction::WeatherDay(i),
                );
            }
        }
        TabPage::Media if s.media.is_some() => {
            for (i, k) in [MediaKey::Previous, MediaKey::PlayPause, MediaKey::Next]
                .into_iter()
                .enumerate()
            {
                let size = if i == 1 { 50.0 } else { 36.0 };
                add(
                    w / 2.0 + (i as f32 - 1.0) * 64.0 - size / 2.0,
                    BODY + 152.0 + (50.0 - size) / 2.0,
                    size,
                    size,
                    UiAction::Media(k),
                );
            }
        }
        TabPage::CalendarWeather => {
            let g = crate::pages::agenda_grid(left, top, w, s);
            for (i, d) in [(5.0, -1), (6.0, 1)] {
                add(
                    g.x0 - left + i * g.cell_w + 2.0,
                    BODY,
                    g.cell_w - 4.0,
                    24.0,
                    UiAction::AgendaMonth(d),
                );
            }
            let first = isle_core::agenda::days_from_civil(g.year, g.month, 1);
            for day in first..first + isle_core::agenda::month_len(g.year, g.month) {
                let cell = day - g.first_cell;
                add(
                    g.x0 - left + (cell % 7) as f32 * g.cell_w + 2.0,
                    g.y0 - top + (cell / 7) as f32 * g.cell_h,
                    g.cell_w - 4.0,
                    g.cell_h - 2.0,
                    UiAction::AgendaDay(day),
                );
            }
        }
        TabPage::Notifications => {
            let list = s.notifications.ordered();
            let page = s.notification_page.min(s.notifications.last_page());
            if !list.is_empty() {
                add(w - 54.0, BODY, 34.0, 28.0, UiAction::ClearNotifications);
            }
            let per = s.notifications.per_page();
            for (i, n) in list.iter().skip(page * per).take(per).enumerate() {
                let y = BODY + 38.0 + i as f32 * 66.0;
                let rw = w - 40.0;
                if s.reply
                    .as_ref()
                    .is_some_and(|r| r.id == n.id && r.arrival_ms == n.arrival_ms)
                {
                    add(
                        32.0,
                        y + 35.0,
                        rw - 66.0,
                        24.0,
                        UiAction::ReplyNotification(n.id, n.arrival_ms),
                    );
                    add(w - 58.0, y + 35.0, 28.0, 24.0, UiAction::SendReply);
                } else if n.can_reply() {
                    add(
                        w - 60.0,
                        y + 35.0,
                        30.0,
                        24.0,
                        UiAction::ReplyNotification(n.id, n.arrival_ms),
                    );
                }
                add(
                    20.0,
                    y,
                    rw,
                    62.0,
                    UiAction::ReadNotification(n.id, n.arrival_ms),
                );
            }
            if page > 0 {
                add(
                    w - 98.0,
                    h - 37.0,
                    32.0,
                    26.0,
                    UiAction::NotificationsPage(page - 1),
                );
            }
            if page < s.notifications.last_page() {
                add(
                    w - 54.0,
                    h - 37.0,
                    32.0,
                    26.0,
                    UiAction::NotificationsPage(page + 1),
                );
            }
        }
        TabPage::Tray => {
            let items = overflow_indices(&s.tray);
            let page = crate::pages::tray_page(s);
            let (columns, cell_w, cell_h, row_pitch) = if s.ui.tray_grid {
                let pitch = 48.0;
                let columns = (((w - 40.0) / pitch) as usize).clamp(1, TRAY_PER_PAGE);
                (columns, pitch, pitch - 6.0, pitch)
            } else {
                (5, (w - 40.0) / 5.0, 66.0, 78.0)
            };
            for (n, i) in items
                .iter()
                .skip(page * TRAY_PER_PAGE)
                .take(TRAY_PER_PAGE)
                .enumerate()
            {
                add(
                    20.0 + (n % columns) as f32 * cell_w,
                    BODY + 48.0 + (n / columns) as f32 * row_pitch,
                    cell_w - 6.0,
                    cell_h,
                    UiAction::TrayClick(*i, false),
                );
            }
            if page > 0 {
                add(w - 98.0, h - 42.0, 32.0, 28.0, UiAction::TrayPage(page - 1));
            }
            if (page + 1) * TRAY_PER_PAGE < items.len() {
                add(w - 54.0, h - 42.0, 32.0, 28.0, UiAction::TrayPage(page + 1));
            }
        }
        TabPage::Timer => {
            let t = &s.timers;
            let seg = (w - 40.0) / 4.0;
            for (i, (m, _)) in TIMER_MODES.iter().enumerate() {
                add(
                    20.0 + i as f32 * seg,
                    BODY,
                    seg - 4.0,
                    28.0,
                    UiAction::Timer(TimerAction::Mode(*m)),
                );
            }
            if t.mode == TimerMode::Reminders {
                add(
                    20.0,
                    BODY + 47.0,
                    w - 108.0,
                    36.0,
                    UiAction::Timer(TimerAction::Field),
                );
                add(
                    w - 78.0,
                    BODY + 47.0,
                    58.0,
                    36.0,
                    UiAction::Timer(TimerAction::Add),
                );
                add(
                    138.0,
                    BODY + 96.0,
                    30.0,
                    28.0,
                    UiAction::Timer(TimerAction::In(-5)),
                );
                add(
                    174.0,
                    BODY + 96.0,
                    30.0,
                    28.0,
                    UiAction::Timer(TimerAction::In(5)),
                );
                for (i, m) in REMINDER_PRESETS.iter().enumerate() {
                    add(
                        220.0 + i as f32 * 66.0,
                        BODY + 96.0,
                        60.0,
                        28.0,
                        UiAction::Timer(TimerAction::InPreset(*m)),
                    );
                }
                for (i, r) in t.reminders.iter().take(REMINDER_ROWS).enumerate() {
                    add(
                        w - 54.0,
                        BODY + 144.0 + i as f32 * 39.0,
                        28.0,
                        28.0,
                        UiAction::Timer(TimerAction::Delete(r.id)),
                    );
                }
            } else {
                let mut started = false;
                match t.mode {
                    TimerMode::Pomodoro => {
                        started = t.pomodoro_phase != isle_core::pomodoro::Phase::Idle;
                        if !started {
                            for (i, actions) in [
                                (TimerAction::Focus(-5), TimerAction::Focus(5)),
                                (TimerAction::Break(-1), TimerAction::Break(1)),
                            ]
                            .into_iter()
                            .enumerate()
                            {
                                let x = w / 2.0 - 235.0 + i as f32 * 240.0;
                                add(
                                    x + 148.0,
                                    BODY + 224.0,
                                    30.0,
                                    28.0,
                                    UiAction::Timer(actions.0),
                                );
                                add(
                                    x + 184.0,
                                    BODY + 224.0,
                                    30.0,
                                    28.0,
                                    UiAction::Timer(actions.1),
                                );
                            }
                        }
                    }
                    TimerMode::Countdown => {
                        started = t.countdown_started;
                        if !started {
                            add(
                                w / 2.0 - 120.0,
                                BODY + 224.0,
                                30.0,
                                28.0,
                                UiAction::Timer(TimerAction::Duration(-1)),
                            );
                            add(
                                w / 2.0 + 90.0,
                                BODY + 224.0,
                                30.0,
                                28.0,
                                UiAction::Timer(TimerAction::Duration(1)),
                            );
                            for (i, m) in TIMER_PRESETS.iter().enumerate() {
                                add(
                                    w / 2.0 - 125.0 + i as f32 * 65.0,
                                    BODY + 262.0,
                                    58.0,
                                    26.0,
                                    UiAction::Timer(TimerAction::Preset(*m)),
                                );
                            }
                        }
                    }
                    TimerMode::Stopwatch => started = t.stopwatch_ms > 0,
                    _ => {}
                }
                let y = h - 52.0;
                add(
                    w / 2.0 - if started { 96.0 } else { 60.0 },
                    y,
                    120.0,
                    36.0,
                    UiAction::Timer(TimerAction::Toggle),
                );
                if started {
                    add(
                        w / 2.0 + 36.0,
                        y,
                        36.0,
                        36.0,
                        UiAction::Timer(TimerAction::Reset),
                    );
                }
            }
        }
        TabPage::Clipboard | TabPage::Shelf | TabPage::Usb | TabPage::Downloads => {
            boxes.extend(crate::phase8_view::controls(tab, left, top, w, h, &s.phase8).map(|(hit, _)| hit));
            if tab == TabPage::Shelf && s.phase8.share.is_none() { boxes.extend(crate::phase8_view::shelf_rows(left, top, w, &s.phase8)); }
            if tab == TabPage::Downloads { boxes.extend(crate::phase8_view::download_recent_rows(left, top, w, h, &s.phase8)); }
        }
        TabPage::Media => {}
        TabPage::Power => {
            // Port of PowerPanel's rows (mac-island 3.1.1), side by side under the tabs.
            let actions = s.power.shown(s.hibernate_allowed);
            let cell = (w - 40.0) / actions.len().max(1) as f32;
            for (i, a) in actions.into_iter().enumerate() {
                add(20.0 + i as f32 * cell, BODY + 6.0, cell - 8.0, 78.0, UiAction::Power(a));
            }
            if s.power_confirm.is_some() {
                add(w - 236.0, BODY + 100.0, 104.0, 32.0, UiAction::PowerCancel);
                add(w - 124.0, BODY + 100.0, 104.0, 32.0, UiAction::PowerConfirm);
            }
        }
    }
    boxes
}

/// Room between two items of the closed pill.
const BAR_GAP: f32 = 8.0;
/// Room on the clock's sides when it is kept in the middle (mac-island's "a little more room").
const CLOCK_GAP: f32 = 14.0;
/// Room between the pill's edges and its outer items.
const BAR_PAD: f32 = 14.0;
/// The least a song title or the next event shrinks to before items drop.
const FLEX_MIN: f32 = 40.0;

/// The closed pill's items with something to show, in the settings' order: (item, natural width,
/// shrinks?). The clock and the timer are measured with their digits as zeros, so the pill does not
/// twitch as the time ticks.
fn bar_entries(text: &crate::text::Text, s: &IslandState) -> Vec<(isle_core::config::BarItem, f32, bool)> {
    use crate::text::Weight;
    use isle_core::config::BarItem;
    let zeros = |t: &str| t.chars().map(|c| if c.is_ascii_digit() { '0' } else { c }).collect::<String>();
    let playing = compact_media_text(s);
    let media_keys = crate::pages::compact_media_keys(s).len() as f32 * 23.;
    let natural = |item: BarItem| -> Option<(f32, bool)> {
        match item {
            BarItem::Clock => Some((text.width(&zeros(&s.clock_text), 15.0, Weight::Semibold), false)),
            BarItem::Next => {
                let t = next_text(s);
                let art = if s.chat.busy { 26.0 } else { 0.0 };
                (!t.is_empty()).then(|| (text.width(t, 12.0, Weight::Medium) + art, true))
            }
            BarItem::Date => (!s.date_text.is_empty()).then(|| (text.width(&s.date_text, 12.0, Weight::Medium), false)),
            BarItem::Media => playing.map(|t| ((text.width(t, 12.0, Weight::Medium) + 26.0).min(s.ui.media_title_width as f32) + media_keys, media_keys == 0.)),
            BarItem::Weather => (s.modules.weather && s.weather.is_some()).then_some((crate::pages::WEATHER_W, false)),
            BarItem::Notifications => (s.modules.notifications && s.notifications.unread_count() > 0).then_some((BELL_W, false)),
            BarItem::Timer => s.timer_text.as_deref().filter(|_| s.modules.timer).map(|t| (text.width(&zeros(t), 12.0, Weight::Medium), false)),
            BarItem::Tray => s.modules.tray.then_some((26.0 * (isle_core::tray::inline_indices(&s.tray).len() + 1) as f32, false)),
            BarItem::Power => Some((crate::pages::POWER_W, false)),
        }
    };
    // The song only where Música is in the list, or once at the end with "Sempre mostrar a música".
    let mut items = s.bar.clone();
    if s.ui.media_always && !items.contains(&BarItem::Media) {
        items.push(BarItem::Media);
    }
    items.into_iter().filter_map(|i| natural(i).map(|(nw, flex)| (i, nw, flex))).collect()
}

/// Items side by side, with their gaps.
fn run_width(list: &[(isle_core::config::BarItem, f32, bool)]) -> f32 {
    list.iter().map(|i| i.1).sum::<f32>() + BAR_GAP * list.len().saturating_sub(1) as f32
}

/// "Relógio no meio": how many of the other items go on the clock's left. Port of mac-island 3.1.1's
/// clock among the groups: where both sides are the most alike, the others keeping their order; the
/// clock's own place in the list wins while it is nearly as even, so items coming and going (a song,
/// the bell) do not make the rest hop sides.
fn clock_split(others: &[(isle_core::config::BarItem, f32, bool)], own: usize) -> usize {
    let side = |l: &[(isle_core::config::BarItem, f32, bool)]| if l.is_empty() { 0.0 } else { run_width(l) + CLOCK_GAP };
    let uneven = |at: usize| (side(&others[..at]) - side(&others[at..])).abs();
    let best = (0..=others.len()).min_by(|a, b| uneven(*a).total_cmp(&uneven(*b))).unwrap_or(0);
    let own = own.min(others.len());
    if uneven(own) <= uneven(best) + 16.0 { own } else { best }
}

/// What does not fit drops from the end of `list` (`keep` stay at least); the flexible ones (song
/// titles, the next event) share what the fixed ones leave.
fn fit_run(list: &mut Vec<(isle_core::config::BarItem, f32, bool)>, room: f32, keep: usize) {
    let least = |l: &[(isle_core::config::BarItem, f32, bool)]| -> f32 {
        l.iter().map(|(_, nw, flex)| if *flex { nw.min(FLEX_MIN) } else { *nw }).sum::<f32>()
            + BAR_GAP * l.len().saturating_sub(1) as f32
    };
    while list.len() > keep && least(list) > room {
        list.pop();
    }
    let mut spare = room - least(list);
    for item in list.iter_mut().filter(|i| i.2) {
        let grow = (item.1 - FLEX_MIN).max(0.0).min(spare);
        let base = item.1.min(FLEX_MIN);
        spare -= grow;
        item.1 = base + grow;
    }
}

/// The closed pill's width its items ask for (`pages::compact_tray_width` keeps it at least "Largura
/// mínima"): side by side; with "Relógio no meio", both sides as wide as the wider one, so the clock
/// is in the middle.
pub fn compact_content_width(text: &crate::text::Text, s: &IslandState) -> f32 {
    use isle_core::config::BarItem;
    let mut list = bar_entries(text, s);
    if s.ui.clock_center {
        if let Some(own) = list.iter().position(|i| i.0 == BarItem::Clock) {
            let (_, cw, _) = list.remove(own);
            let at = clock_split(&list, own);
            let side = |l: &[(BarItem, f32, bool)]| if l.is_empty() { 0.0 } else { run_width(l) + CLOCK_GAP };
            return 2.0 * side(&list[..at]).max(side(&list[at..])) + cw + 2.0 * BAR_PAD;
        }
    }
    run_width(&list) + 2.0 * BAR_PAD
}

/// The closed pill's items: `(item, x, width)`, drawn and hit tested alike. Side by side in the
/// settings' order, the group in the middle of the pill. With "Relógio no meio" (`ui.clock_center`)
/// the clock is in the middle and the others, in their order, on its two sides as evenly as they go
/// (`clock_split`), each side against the clock. What does not fit drops from the end (on the clock's
/// sides, the farthest from it first); the clock or the first item always stays.
pub fn compact_slots(
    text: &crate::text::Text,
    x: f32,
    w: f32,
    s: &IslandState,
) -> Vec<(isle_core::config::BarItem, f32, f32)> {
    use isle_core::config::BarItem;
    let mut list = bar_entries(text, s);
    let room = (w - 2.0 * BAR_PAD).max(0.0);
    if s.ui.clock_center {
        if let Some(own) = list.iter().position(|i| i.0 == BarItem::Clock) {
            let (clock, cw, _) = list.remove(own);
            let at = clock_split(&list, own);
            let mut right = list.split_off(at);
            let mut left = list;
            let cw = cw.min(room);
            let cx = x + (w - cw) / 2.0;
            let half = (room - cw) / 2.0 - CLOCK_GAP;
            left.reverse();
            fit_run(&mut left, half, 0);
            left.reverse();
            fit_run(&mut right, half, 0);
            let mut out = Vec::with_capacity(left.len() + right.len() + 1);
            let mut at = cx - CLOCK_GAP - run_width(&left);
            for (item, iw, _) in left {
                out.push((item, at, iw));
                at += iw + BAR_GAP;
            }
            out.push((clock, cx, cw));
            let mut at = cx + cw + CLOCK_GAP;
            for (item, iw, _) in right {
                out.push((item, at, iw));
                at += iw + BAR_GAP;
            }
            return out;
        }
    }
    fit_run(&mut list, room, 1);
    let mut at = x + (w - run_width(&list).min(room)) / 2.0;
    list.into_iter()
        .map(|(item, iw, _)| {
            let iw = iw.min(room);
            let slot = (item, at, iw);
            at += iw + BAR_GAP;
            slot
        })
        .collect()
}

/// The bell and its count on the bar.
pub const BELL_W: f32 = 40.0;

/// The song's title (or its lyric line) for the bar's Música slot.
pub fn compact_media_text(s: &IslandState) -> Option<&str> {
    let media = s.media.as_ref().filter(|m| s.modules.media && (m.playing || s.ui.media_always || s.ui.media_controls))?;
    if s.ui.media_lyric_title && media.playing {
        if let Some(lyric) = s.active_lyric_line.as_deref() { return Some(lyric); }
    }
    Some(&media.title)
}

/// Próximo: what comes within the countdown window, else the date (the song is Música's, not its).
pub fn next_text(s: &IslandState) -> &str {
    s.next_countdown_text.as_deref().unwrap_or(&s.date_text)
}

#[cfg(test)]
mod presentation_tests {
    use super::*;
    use isle_core::{agenda::days_from_civil, tray::TrayItem, weather::{Day, Weather}};

    fn panel(tab: TabPage) -> IslandState {
        IslandState { mode: PillMode::Expanded(tab), ..IslandState::default() }
    }

    fn action(s: &IslandState, action: UiAction) -> HitBox {
        let PillMode::Expanded(tab) = s.mode else { unreachable!() };
        let (w, h, _) = expanded_size(tab);
        expanded_hitboxes(w, w, h, 6.0, s)
            .into_iter()
            .find(|hit| hit.action == action)
            .unwrap()
    }

    #[test]
    fn agenda_month_uses_the_featured_day_column_when_enabled() {
        let mut s = panel(TabPage::CalendarWeather);
        let day = days_from_civil(2026, 11, 1);
        s.today = day;
        s.ui.agenda_featured_day = true;
        let featured = action(&s, UiAction::AgendaDay(day));

        s.ui.agenda_featured_day = false;
        let full = action(&s, UiAction::AgendaDay(day));
        assert!(full.x < featured.x && full.w > featured.w);
        assert!(full.contains(full.x + full.w / 2., full.y + full.h / 2.));
    }

    #[test]
    fn forecast_cards_extend_the_day_targets_without_changing_their_identity() {
        let mut s = panel(TabPage::Weather);
        s.weather = Some(Weather { days: vec![Day::default(); 7], ..Weather::default() });
        let regular = action(&s, UiAction::WeatherDay(2));

        s.ui.weather_cards = true;
        let card = action(&s, UiAction::WeatherDay(2));
        assert_eq!((card.x, card.y, card.w), (regular.x, regular.y, regular.w));
        assert!(card.contains(card.x + card.w / 2., regular.y + regular.h + 1.));
        assert!(card.x + card.w < action(&s, UiAction::WeatherDay(3)).x);
    }

    #[test]
    fn control_switches_reclaim_only_their_own_audio_row_space() {
        let mut s = panel(TabPage::ControlCenter);
        s.audio_available = true;
        s.mic_available = true;
        let baseline = action(&s, UiAction::SetVolume(0));
        let mic = action(&s, UiAction::ToggleMic);

        s.ui.volume_percent = false;
        let without_percent = action(&s, UiAction::SetVolume(0));
        assert!(without_percent.w > baseline.w && without_percent.x + without_percent.w < mic.x);
        assert_eq!(action(&s, UiAction::ToggleMic).x, mic.x);

        s.ui.mic_button = false;
        let (w, h, _) = expanded_size(TabPage::ControlCenter);
        let boxes = expanded_hitboxes(w, w, h, 6.0, &s);
        assert!(!boxes.iter().any(|hit| hit.action == UiAction::ToggleMic));
        assert!(action(&s, UiAction::SetVolume(0)).w > without_percent.w);
        assert_eq!(action(&s, UiAction::ToggleMute).h, 34.0);
    }

    #[test]
    fn tray_grid_packs_icons_without_changing_page_membership() {
        let mut s = panel(TabPage::Tray);
        s.tray = (0..13).map(|i| TrayItem {
            key: i.to_string(),
            pinned: i == 0,
            ..TrayItem::default()
        }).collect();
        let tile = action(&s, UiAction::TrayClick(6, false));

        s.ui.tray_grid = true;
        let icon = action(&s, UiAction::TrayClick(6, false));
        assert!(icon.w < tile.w && icon.h < tile.h && icon.y < tile.y);
        assert_eq!(icon.w, icon.h);
        assert!(icon.contains(icon.x + icon.w / 2., icon.y + icon.h / 2.));
        assert_eq!(action(&s, UiAction::TrayPage(1)).y, expanded_size(TabPage::Tray).1 - 36.0, "the pages stay at the bottom");

        s.tray_page = 1;
        let (w, h, _) = expanded_size(TabPage::Tray);
        let ids = expanded_hitboxes(w, w, h, 6.0, &s)
            .into_iter()
            .filter_map(|hit| match hit.action {
                UiAction::TrayClick(i, _) => Some(i),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(ids, [11, 12]);
    }
}
