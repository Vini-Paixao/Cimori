//! Where everything in the settings window sits, in logical pixels (scroll already applied).
//! Measures from the approved mockup: side bar 236 (64 below 760 wide), header 64, preview 132,
//! cards padded 18, rows at least 44, two columns from 760 of content.
use super::{SettingsAction, SettingsView};
use crate::text::{Text, Weight};
use isle_core::config::IsleConfig;
use isle_core::prefs::{self, Pref, PrefKind, PrefValue, SettingsTab};

pub const NAV_W: f32 = 236.0;
pub const NAV_NARROW: f32 = 64.0;
pub const NARROW_BELOW: f32 = 760.0;
pub const HEADER: f32 = 64.0;
pub const CARD_PAD: f32 = 18.0;
pub const CARD_TITLE: f32 = 32.0;
pub const ROW: f32 = 44.0;
pub const DESC: f32 = 16.0;
pub const GAP: f32 = 14.0;
pub const PREVIEW_PILL: f32 = 86.0;
pub const CHIP_H: f32 = 26.0;
pub const CHIP_LINE: f32 = 32.0;
pub const CHIP_X: f32 = 18.0;
pub const SWITCH: (f32, f32) = (36.0, 20.0);
pub const SLIDER_W: f32 = 200.0;
pub const FIELD_W: f32 = 240.0;
pub const MAP_H: f32 = 110.0;
pub const RESULT_H: f32 = 30.0;
pub const NAV_ITEM: f32 = 32.0;
pub const NAV_GROUP: f32 = 26.0;
pub const NAV_TOP: f32 = 108.0;
pub const NAV_TOP_NARROW: f32 = 16.0;
pub const TWO_COLUMNS_FROM: f32 = 760.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub action: SettingsAction,
}

impl Hit {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
}

#[derive(Clone)]
pub struct RowBox {
    pub pref: &'static Pref,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub card: String,
    pub enabled: bool,
    /// The name's line height: 44 alone, 60 with a description under it.
    pub head: f32,
}

#[derive(Debug, Clone)]
pub struct CardBox {
    pub title: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// A side-bar entry: a tab, or a group's title (`tab` None).
#[derive(Debug, Clone)]
pub struct NavEntry {
    pub tab: Option<SettingsTab>,
    pub label: &'static str,
    pub y: f32,
}

#[derive(Clone)]
pub struct Layout {
    pub w: f32,
    pub h: f32,
    pub narrow: bool,
    pub nav_w: f32,
    pub nav_max_scroll: f32,
    /// Tabs (clickable) of the side bar.
    pub nav: Vec<Hit>,
    pub nav_entries: Vec<NavEntry>,
    /// The search field (side bar when wide, header when narrow).
    pub search: (f32, f32, f32, f32),
    pub cards: Vec<CardBox>,
    pub rows: Vec<RowBox>,
    pub hits: Vec<Hit>,
    /// Everything below the header, unscrolled: what the scroll is clamped against.
    pub content_h: f32,
    pub preview: Option<(f32, f32, f32, f32)>,
}

impl Layout {
    pub fn max_scroll(&self) -> f32 {
        (self.content_h - (self.h - HEADER)).max(0.0)
    }
}

fn chip_label(text: &Text, label: &str, removable: bool) -> f32 {
    text.width(label, 12.0, Weight::Medium) + 24.0 + if removable { 16.0 } else { 0.0 }
}

/// A chips row's items: (key, label, in?) in drawing order: the ones in (in their order), then
/// the rest.
pub fn chip_items(pref: &str, cfg: &IsleConfig, view: &SettingsView) -> Vec<(String, String, bool)> {
    let Some(p) = prefs::by_id(pref) else { return vec![] };
    let PrefValue::List(current) = (p.get)(cfg) else { return vec![] };
    let mut choices: Vec<(String, String)> =
        prefs::chip_choices(pref).iter().map(|(k, l)| (k.to_string(), l.to_string())).collect();
    choices.extend(view.dynamic(pref).iter().cloned());
    for key in &current {
        if !choices.iter().any(|(k, _)| k == key) {
            choices.push((key.clone(), key.rsplit(['\\', '/', '!']).next().unwrap_or(key).to_string()));
        }
    }
    let label = |k: &str| choices.iter().find(|(c, _)| c == k).map(|(_, l)| l.clone()).unwrap_or_default();
    // Muted apps keep one fixed order (on or off); the others list what is in first.
    if pref == "muted_apps" {
        return choices.iter().map(|(k, l)| (k.clone(), l.clone(), current.contains(k))).collect();
    }
    let mut out: Vec<_> = current.iter().map(|k| (k.clone(), label(k), true)).collect();
    out.extend(choices.iter().filter(|(k, _)| !current.contains(k)).map(|(k, l)| (k.clone(), l.clone(), false)));
    out
}

/// Chips flowed into lines `width` wide: (index into items, x offset, line).
fn flow(text: &Text, items: &[(String, String, bool)], width: f32, ordered: bool) -> Vec<(f32, usize, f32)> {
    let (mut x, mut line) = (0.0, 0usize);
    let mut out = Vec::new();
    for (_, label, on) in items {
        let w = chip_label(text, label, *on && ordered);
        if x > 0.0 && x + w > width {
            x = 0.0;
            line += 1;
        }
        out.push((x, line, w));
        x += w + 6.0;
    }
    out
}

fn ordered(pref: &str) -> bool {
    pref != "muted_apps"
}

/// Choices too many for a segmented control, shown as a list of rows (the icon packs, the opening tab).
pub fn listed(id: &str) -> bool {
    matches!(id, "icon_pack" | "ui.open_tab")
}

fn choice_width(text: &Text, options: &[&str]) -> f32 {
    options.iter().map(|o| text.width(o, 12.0, Weight::Medium) + 20.0).sum::<f32>() + 6.0
}

/// Most lines a description wraps to; past them it ends in "…".
const DESC_LINES: usize = 5;

/// The text under a row's name: its description and, when it waits for a restart, that note.
fn desc_text(p: &Pref) -> String {
    match (p.desc.is_empty(), p.restart) {
        (true, true) => isle_core::prefs::RESTART_NOTE.to_string(),
        (false, true) => format!("{} · {}", p.desc, isle_core::prefs::RESTART_NOTE),
        (false, false) => p.desc.to_string(),
        (true, false) => String::new(),
    }
}

/// The room left of a row's control, where its name and description go.
pub fn name_room(row: &RowBox, text: &Text, view: &SettingsView) -> f32 {
    if listed(row.pref.id) {
        return row.w;
    }
    let (cx, ..) = control_rect(row, text);
    match row.pref.kind {
        PrefKind::Chips | PrefKind::Monitors | PrefKind::CitySearch => row.w,
        // Its options went below the name: the whole width is the name's.
        PrefKind::Choice(_) if cx <= row.x => row.w,
        PrefKind::Slider { .. } => (cx - row.x - 64.0).max(24.0),
        // An Info row's text lies at its right: the name and description take the rest.
        PrefKind::Info => (row.w - text.width(view.info(row.pref.id), 12.0, Weight::Regular) - 16.0).max(80.0),
        _ => (cx - row.x - 12.0).max(80.0),
    }
}

/// A row's description as drawn: wrapped to its room, up to `DESC_LINES` lines.
pub fn desc_lines(row: &RowBox, text: &Text, view: &SettingsView) -> Vec<String> {
    let desc = desc_text(row.pref);
    if desc.is_empty() {
        return Vec::new();
    }
    text.wrap(&desc, name_room(row, text, view), 11.5, Weight::Regular, DESC_LINES)
}

/// The row's height for `w` of room.
fn row_height(p: &'static Pref, w: f32, cfg: &IsleConfig, view: &SettingsView, text: &Text) -> (f32, f32) {
    let probe = RowBox { pref: p, x: 0.0, y: 0.0, w, h: 0.0, card: String::new(), enabled: true, head: ROW };
    let lines = desc_lines(&probe, text, view).len();
    let head = if lines == 0 { ROW } else { ROW + DESC * lines as f32 };
    let below = match p.kind {
        PrefKind::Choice(options) if listed(p.id) => options.len() as f32*34.+6.,
        PrefKind::Chips => {
            let items = chip_items(p.id, cfg, view);
            let lines = flow(text, &items, w, ordered(p.id)).iter().map(|f| f.1).max().map_or(0, |l| l + 1);
            lines as f32 * CHIP_LINE + 6.0
        }
        PrefKind::Monitors if p.id == "monitors" || p.id == "main_monitor" => MAP_H + 8.0,
        PrefKind::CitySearch => 38.0 + view.city_results.len() as f32 * RESULT_H,
        PrefKind::Choice(options) if choice_width(text, options) > w * 0.6 => 36.0,
        _ => 0.0,
    };
    (head + below, head)
}

pub fn layout(view: &SettingsView, cfg: &IsleConfig, text: &Text, w: f32, h: f32) -> Layout {
    let narrow = w < NARROW_BELOW;
    let nav_w = if narrow { NAV_NARROW } else { NAV_W };
    let mut hits = Vec::new();

    // Side bar.
    let tabs: Vec<SettingsTab> =
        SettingsTab::ALL.into_iter().filter(|t| *t != SettingsTab::Monitors || view.monitors.len() >= 2).collect();
    let mut nav = Vec::new();
    let mut nav_entries = Vec::new();
    let nav_top = if narrow { NAV_TOP_NARROW } else { NAV_TOP };
    let item_h = if narrow { 40. } else { NAV_ITEM };
    let groups = if narrow { 0. } else { 3. * NAV_GROUP };
    let nav_max_scroll = (nav_top + tabs.len() as f32 * item_h + groups + 6. - h).max(0.);
    let mut y = nav_top - view.nav_scroll.clamp(0., nav_max_scroll);
    let mut group = "";
    for tab in tabs {
        if !narrow && tab.group() != group {
            group = tab.group();
            nav_entries.push(NavEntry { tab: None, label: group, y });
            y += NAV_GROUP;
        }
        let item_h = if narrow { 40.0 } else { NAV_ITEM };
        let top_limit = if narrow { NAV_TOP_NARROW } else { NAV_TOP };
        nav_entries.push(NavEntry { tab: Some(tab), label: tab.title(), y });
        if y >= top_limit - 0.01 && y + item_h <= h {
            let (x, iw) = if narrow { (12.0, 40.0) } else { (10.0, nav_w - 20.0) };
            nav.push(Hit { x, y, w: iw, h: item_h - 2.0, action: SettingsAction::Tab(tab) });
        }
        y += item_h;
    }
    hits.extend(nav.iter().cloned());

    // Search: in the side bar when wide, at the header's right when narrow.
    let search = if narrow { (w - 16.0 - 200.0_f32.min(w - nav_w - 180.0), 15.0, 200.0_f32.min(w - nav_w - 180.0), 34.0) } else { (14.0, 58.0, nav_w - 28.0, 34.0) };
    let searching = !view.search.trim().is_empty();
    if searching {
        hits.push(Hit { x: search.0, y: search.1, w: search.2 - 28.0, h: search.3, action: SettingsAction::FocusSearch });
        hits.push(Hit { x: search.0 + search.2 - 28.0, y: search.1, w: 28.0, h: search.3, action: SettingsAction::ClearSearch });
    } else {
        hits.push(Hit { x: search.0, y: search.1, w: search.2, h: search.3, action: SettingsAction::FocusSearch });
    }

    // Content.
    let pad = if narrow { 16.0 } else { 32.0 };
    let x0 = nav_w + pad;
    let cw = (w - nav_w - pad * 2.0).max(120.0);
    let top = HEADER + 8.0;
    let mut y = top - view.scroll;
    let mut rows = Vec::new();
    let mut cards = Vec::new();

    // The preview (Bar, Look, Hiding), with the bar's items under the pill on the Bar tab.
    let mut preview = None;
    if !searching && matches!(view.tab, SettingsTab::Bar | SettingsTab::Look | SettingsTab::Hiding) {
        let pill_box = PREVIEW_PILL + (cfg.ui.pill_height_px() - 34.).max(0.);
        let mut ph = pill_box;
        if view.tab == SettingsTab::Bar {
            let strip = strip_pref(cfg);
            let items = chip_items(strip, cfg, view);
            let lines = flow(text, &items, cw - 2.0 * CHIP_X, true);
            let n = lines.iter().map(|f| f.1).max().map_or(0, |l| l + 1);
            let strip_y = y + pill_box;
            for ((key, _, on), (dx, line, cwid)) in items.iter().zip(lines) {
                let (cx, cy) = (x0 + CHIP_X + dx, strip_y + line as f32 * CHIP_LINE);
                chip_hits(&mut hits, strip, key, *on, true, cx, cy, cwid, items.iter().take_while(|i| i.0 != *key).filter(|i| i.2).count(), top, h);
            }
            ph += n as f32 * CHIP_LINE + 10.0;
        }
        preview = Some((x0, y, cw, ph));
        y += ph + GAP;
    }

    // Cards: the tab's rows (or the search's), grouped by card in table order.
    let step = view.welcome_step.min(prefs::WELCOME.len() - 1);
    let welcome = !searching && view.tab == SettingsTab::Welcome;
    let found: Vec<&'static Pref> = if searching {
        prefs::search(&view.search)
    } else if welcome {
        prefs::WELCOME[step].1.iter().filter_map(|id| prefs::by_id(id)).filter(|p| p.id != "welcome.back" || step > 0).collect()
    } else {
        // "Baixar" only when a newer version was found.
        prefs::all().iter().filter(|p| p.tab == view.tab && !(view.tab == SettingsTab::Bar && p.id == strip_pref(cfg))
            && (p.id != "action.download_update" || view.update.is_some())).collect()
    };
    let mut groups: Vec<(String, Vec<&'static Pref>)> = Vec::new();
    for p in found {
        let title = if searching {
            format!("{} · {}", p.tab.title(), p.card)
        } else if welcome {
            format!("Passo {} de {} · {}", step + 1, prefs::WELCOME.len(), prefs::WELCOME[step].0)
        } else {
            p.card.to_string()
        };
        match groups.iter_mut().find(|g| g.0 == title) {
            Some(g) => g.1.push(p),
            None => groups.push((title, vec![p])),
        }
    }
    let columns = if cw >= TWO_COLUMNS_FROM && !welcome { 2 } else { 1 };
    let col_w = (cw - GAP * (columns as f32 - 1.0)) / columns as f32;
    let mut col_y = vec![y; columns];
    for (title, list) in groups {
        let c = (0..columns).min_by(|a, b| col_y[*a].total_cmp(&col_y[*b])).unwrap_or(0);
        let cx = x0 + c as f32 * (col_w + GAP);
        let inner = col_w - 2.0 * CARD_PAD;
        let mut ry = col_y[c] + CARD_TITLE;
        for p in list {
            let (rh, head) = row_height(p, inner, cfg, view, text);
            let enabled = (p.enabled)(cfg);
            let row = RowBox { pref: p, x: cx + CARD_PAD, y: ry, w: inner, h: rh, card: title.clone(), enabled, head };
            if enabled {
                row_hits(&mut hits, &row, cfg, view, text, top, h);
            }
            rows.push(row);
            ry += rh;
        }
        let card_h = ry + CARD_PAD * 0.5 - col_y[c];
        cards.push(CardBox { title, x: cx, y: col_y[c], w: col_w, h: card_h });
        col_y[c] += card_h + GAP;
    }
    let bottom = col_y.iter().cloned().fold(y, f32::max);
    let content_h = bottom + view.scroll - top + 16.0;

    Layout { w, h, narrow, nav_w, nav_max_scroll, nav, nav_entries, search, cards, rows, hits, content_h, preview }
}

/// The chips under the Bar tab's preview: the notch's items when the island is a notch.
pub fn strip_pref(cfg: &IsleConfig) -> &'static str {
    if cfg.ui.pill_style == isle_core::config::PillStyle::Notch { "notch_items" } else { "bar_items" }
}

/// Keeps a hit only when it is whole inside the scrolled area.
fn push(hits: &mut Vec<Hit>, hit: Hit, top: f32, bottom: f32) {
    if hit.y >= top - 0.01 && hit.y + hit.h <= bottom + 0.01 && hit.w > 0.0 {
        hits.push(hit);
    }
}

#[allow(clippy::too_many_arguments)]
fn chip_hits(hits: &mut Vec<Hit>, pref: &'static str, key: &str, on: bool, ordered: bool, x: f32, y: f32, w: f32, index: usize, top: f32, bottom: f32) {
    if on && ordered {
        push(hits, Hit { x, y, w: w - 16.0, h: CHIP_H, action: SettingsAction::ChipGrab(pref, index) }, top, bottom);
        push(hits, Hit { x: x + w - 16.0, y, w: 16.0, h: CHIP_H, action: SettingsAction::ChipToggle(pref, key.to_string()) }, top, bottom);
    } else {
        push(hits, Hit { x, y, w, h: CHIP_H, action: SettingsAction::ChipToggle(pref, key.to_string()) }, top, bottom);
    }
}

/// Where a row's control sits: its right part (or the line under the name for wide controls).
pub fn control_rect(row: &RowBox, text: &Text) -> (f32, f32, f32, f32) {
    if listed(row.pref.id) { return (row.x,row.y+row.head,row.w,row.h-row.head); }
    let mid = row.y + row.head / 2.0;
    match row.pref.kind {
        PrefKind::Switch => (row.x + row.w - SWITCH.0, mid - SWITCH.1 / 2.0, SWITCH.0, SWITCH.1),
        PrefKind::Choice(options) => {
            let cw = choice_width(text, options);
            if cw > row.w * 0.6 {
                (row.x, row.y + row.head, cw.min(row.w), 30.0)
            } else {
                (row.x + row.w - cw, mid - 15.0, cw, 30.0)
            }
        }
        PrefKind::Slider { .. } => {
            let sw = SLIDER_W.min(row.w * 0.45);
            (row.x + row.w - sw, mid - 10.0, sw, 20.0)
        }
        PrefKind::Color(c) => {
            let cw = c.len() as f32 * 28.0;
            (row.x + row.w - cw, mid - 12.0, cw, 24.0)
        }
        PrefKind::Text { .. } => {
            let fw = FIELD_W.min(row.w * 0.5);
            (row.x + row.w - fw, mid - 15.0, fw, 30.0)
        }
        PrefKind::Action(_) => {
            let bw = text.width(action_label(row.pref), 12.0, Weight::Medium) + 28.0;
            (row.x + row.w - bw, mid - 15.0, bw, 30.0)
        }
        PrefKind::Chips => (row.x, row.y + row.head, row.w, row.h - row.head),
        PrefKind::Monitors => (row.x, row.y + row.head, row.w, MAP_H),
        PrefKind::CitySearch => (row.x, row.y + row.head, row.w.min(360.0), 30.0),
        PrefKind::Info => (row.x + row.w * 0.4, row.y, row.w * 0.6, row.head),
        PrefKind::Shortcut => {
            let sw = SHORTCUT_W.min(row.w * 0.55);
            (row.x + row.w - sw, mid - 15.0, sw, 30.0)
        }
    }
}

/// A shortcut row's button (its keys, or "Pressione as teclas…"), its × included.
pub const SHORTCUT_W: f32 = 200.0;

/// The button's text of an Action row.
pub fn action_label(p: &Pref) -> &'static str {
    use isle_core::prefs::ActionId::*;
    match p.kind {
        PrefKind::Action(a) => match a {
            OpenFolder => "Abrir pasta",
            Quit => "Sair",
            InstallHooks => "Instalar hooks",
            RemoveHooks => "Remover",
            Export => "Exportar",
            Import => "Importar",
            Restore => "Restaurar",
            PickIconFolder => "Escolher pasta",
            DeleteIconPack => "Excluir",
            ReloadIcons => "Recarregar",
            OpenWindowsBanners => "Abrir notificações",
            OpenBridgeReadme => "Ver instruções",
            OpenLink(url) if url == isle_core::prefs::ISSUES => "Reportar",
            OpenLink(url) if url == isle_core::prefs::REPO => "Abrir no GitHub",
            OpenLink(url) if url == isle_core::prefs::LICENSE => "Ler a licença",
            OpenLink(_) => "Abrir site",
            CheckUpdates => "Verificar",
            DownloadUpdate => "Baixar",
            WelcomeBack => "Voltar",
            WelcomeNext => "Próximo",
            WelcomeFinish => "Concluir",
            ShowWelcome => "Mostrar",
            CopyBridge => "Copiar código da ponte",
        },
        _ => "",
    }
}

/// Monitors drawn side by side in their order: (device, x, y, w, h).
pub fn monitor_rects(view: &SettingsView, area: (f32, f32, f32, f32)) -> Vec<(String, f32, f32, f32, f32)> {
    let n = view.monitors.len().max(1) as f32;
    let gap = 10.0;
    let mw = ((area.2 - gap * (n - 1.0)) / n).min(160.0);
    let mh = (mw * 9.0 / 16.0).min(area.3 - 8.0);
    view.monitors
        .iter()
        .enumerate()
        .map(|(i, m)| (m.device.clone(), area.0 + i as f32 * (mw + gap), area.1 + 4.0, mw, mh))
        .collect()
}

fn row_hits(hits: &mut Vec<Hit>, row: &RowBox, cfg: &IsleConfig, view: &SettingsView, text: &Text, top: f32, bottom: f32) {
    let id = row.pref.id;
    let (cx, cy, cw, ch) = control_rect(row, text);
    match row.pref.kind {
        PrefKind::Choice(options) if listed(id) => {
            for i in 0..options.len() { push(hits, Hit {x:cx,y:cy+i as f32*34.,w:cw,h:32.,action:SettingsAction::Choose(id,i)},top,bottom); }
        }
        PrefKind::Switch => push(hits, Hit { x: row.x, y: row.y, w: row.w, h: row.head, action: SettingsAction::Toggle(id) }, top, bottom),
        PrefKind::Choice(options) => {
            let mut x = cx + 3.0;
            for (i, o) in options.iter().enumerate() {
                let ow = text.width(o, 12.0, Weight::Medium) + 20.0;
                push(hits, Hit { x, y: cy + 3.0, w: ow, h: ch - 6.0, action: SettingsAction::Choose(id, i) }, top, bottom);
                x += ow;
            }
        }
        PrefKind::Slider { .. } => push(hits, Hit { x: cx, y: cy, w: cw, h: ch, action: SettingsAction::Slide(id, 0.0) }, top, bottom),
        PrefKind::Color(colors) => {
            for (i, c) in colors.iter().enumerate() {
                push(hits, Hit { x: cx + i as f32 * 28.0, y: cy, w: 24.0, h: 24.0, action: SettingsAction::Pick(id, *c) }, top, bottom);
            }
        }
        PrefKind::Text { .. } => push(hits, Hit { x: cx, y: cy, w: cw, h: ch, action: SettingsAction::Edit(id) }, top, bottom),
        PrefKind::Action(a) => push(hits, Hit { x: cx, y: cy, w: cw, h: ch, action: SettingsAction::Run(a) }, top, bottom),
        PrefKind::Chips => {
            let items = chip_items(id, cfg, view);
            let ord = ordered(id);
            for (n, ((key, _, on), (dx, line, w))) in items.iter().zip(flow(text, &items, row.w, ord)).enumerate() {
                let index = items[..n].iter().filter(|i| i.2).count();
                chip_hits(hits, id, key, *on, ord, cx + dx, cy + line as f32 * CHIP_LINE, w, index, top, bottom);
            }
        }
        PrefKind::Monitors => {
            for (device, mx, my, mw, mh) in monitor_rects(view, (cx, cy, cw, ch)) {
                let action = if id == "main_monitor" { SettingsAction::MainMonitor(device) } else { SettingsAction::Monitor(device) };
                push(hits, Hit { x: mx, y: my, w: mw, h: mh, action }, top, bottom);
            }
        }
        PrefKind::CitySearch => {
            push(hits, Hit { x: cx, y: cy, w: cw, h: ch, action: SettingsAction::Edit(id) }, top, bottom);
            for i in 0..view.city_results.len() {
                let y = cy + 38.0 + i as f32 * RESULT_H;
                push(hits, Hit { x: cx, y, w: cw, h: RESULT_H - 2.0, action: SettingsAction::PickCity(i) }, top, bottom);
            }
        }
        PrefKind::Info => {}
        PrefKind::Shortcut => {
            push(hits, Hit { x: cx, y: cy, w: cw - 30.0, h: ch, action: SettingsAction::Record(id) }, top, bottom);
            push(hits, Hit { x: cx + cw - 28.0, y: cy, w: 28.0, h: ch, action: SettingsAction::ClearShortcut(id) }, top, bottom);
        }
    }
}

/// The action under the pointer; a slider's carries the value under it.
pub fn hit(layout: &Layout, x: f32, y: f32) -> Option<SettingsAction> {
    let b = layout.hits.iter().find(|b| b.contains(x, y))?;
    Some(match &b.action {
        SettingsAction::Slide(id, _) => SettingsAction::Slide(id, slider_value(id, (x - b.x) / b.w)),
        other => other.clone(),
    })
}

/// A slider's value at `t` (0 at its left, 1 at its right), stepped and clamped.
pub fn slider_value(id: &str, t: f32) -> f32 {
    let Some(PrefKind::Slider { min, max, step, .. }) = prefs::by_id(id).map(|p| p.kind) else { return 0.0 };
    let raw = min + t.clamp(0.0, 1.0) * (max - min);
    (min + ((raw - min) / step).round() * step).clamp(min, max)
}
