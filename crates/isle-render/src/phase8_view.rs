//! Shared drawing and hit targets for the files and devices panels.

use super::*;
use crate::layout::BODY;
use crate::pages::HitBox;
use isle_core::phase8::{Phase8View, DOWNLOAD_ROWS, ROWS_PER_PAGE};
use isle_core::state::TabPage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase8Action {
    CopyClip(u64),
    PinClip(u64),
    DeleteClip(u64),
    ClipboardPage(usize),
    ClearClips,
    SearchClips,
    FilterPinnedClips,
    ShelfPick,
    ShelfOpen(usize),
    ShelfCopy(usize),
    ShelfRemove(usize),
    /// The file's name area: no click action, only where a press starts dragging it out.
    ShelfRow(usize),
    ShareStart,
    ShareStop,
    ShareApprove,
    ShareDeny,
    UsbOpen(usize),
    UsbEject(usize),
    UsbPage(usize),
    OpenDownloads,
    /// A recent download: opened, or shown selected in Explorer.
    RecentOpen(usize),
    RecentShow(usize),
}

const ROW_H: f32 = 44.0;

fn page_range(page: usize, len: usize) -> std::ops::Range<usize> {
    let start = page.min(len.saturating_sub(1) / ROWS_PER_PAGE) * ROWS_PER_PAGE;
    start..len.min(start + ROWS_PER_PAGE)
}

/// Where a press drags shelf file `i` out: the row left of its buttons.
pub fn shelf_rows(x: f32, y: f32, w: f32, view: &Phase8View) -> impl Iterator<Item = HitBox> {
    let top = BODY + 39.;
    (0..view.shelf.len().min(6)).map(move |i| HitBox {
        x: x + 20., y: y + top + i as f32 * 37., w: w - 203. - 26., h: 36., action: UiAction::Phase8(Phase8Action::ShelfRow(i)),
    })
}

pub fn controls(
    tab: TabPage,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    view: &Phase8View,
) -> impl Iterator<Item = (HitBox, &'static str)> {
    use Phase8Action::*;
    let mut result = Vec::new();
    let mut button = |action, label, bx, by, bw| {
        result.push((
            HitBox {
                x: x + bx,
                y: y + by,
                w: bw,
                h: 28.,
                action: UiAction::Phase8(action),
            },
            label,
        ));
    };
    let top = BODY + if tab == TabPage::Clipboard { 77. } else { 39. };
    let bottom = h - 43.;
    match tab {
        TabPage::Clipboard => {
            button(SearchClips, "Buscar", 20., BODY + 35., w - 155.);
            button(
                FilterPinnedClips,
                if view.clipboard_pins_only {
                    "Só fixados"
                } else {
                    "Todos"
                },
                w - 124.,
                BODY + 35.,
                103.,
            );
            let rows = page_range(view.clipboard_page, view.clipboard.len());
            for (offset, i) in rows.clone().enumerate() {
                let row = &view.clipboard[i];
                let y = top + offset as f32 * ROW_H + 6.;
                button(CopyClip(row.id), if view.clipboard_paste { "Colar" } else { "Copiar" }, w - 209., y, 58.);
                button(
                    PinClip(row.id),
                    if row.pinned { "Soltar" } else { "Fixar" },
                    w - 146.,
                    y,
                    56.,
                );
                button(DeleteClip(row.id), "Excluir", w - 85., y, 64.);
            }
            if view.clipboard_has_recent {
                button(ClearClips, "Limpar recentes", 20., bottom, 117.);
            }
            if rows.start > 0 {
                button(
                    ClipboardPage(rows.start / ROWS_PER_PAGE - 1),
                    "Anterior",
                    w - 165.,
                    bottom,
                    67.,
                );
            }
            if rows.end < view.clipboard.len() {
                button(
                    ClipboardPage(rows.start / ROWS_PER_PAGE + 1),
                    "Próxima",
                    w - 90.,
                    bottom,
                    69.,
                );
            }
        }
        TabPage::Shelf if view.share.is_some() => {
            button(ShareStop, "Encerrar QR", 20., bottom, 104.);
            if view
                .share
                .as_ref()
                .is_some_and(|share| share.pending_code.is_some())
            {
                button(ShareDeny, "Recusar", w - 171., bottom, 69.);
                button(ShareApprove, "Permitir", w - 96., bottom, 75.);
            }
        }
        TabPage::Shelf => {
            for i in 0..view.shelf.len().min(6) {
                let y = top + i as f32 * 37. + 4.;
                button(ShelfOpen(i), "Abrir", w - 203., y, 52.);
                button(ShelfCopy(i), "Copiar", w - 146., y, 56.);
                button(ShelfRemove(i), "Retirar", w - 85., y, 64.);
            }
            if view.shelf.len() < view.shelf_capacity {
                button(ShelfPick, "Adicionar arquivo", 20., bottom, 130.);
            }
            if view.shelf_qr && (!view.shelf.is_empty() || view.shelf_receive) {
                button(ShareStart, "Compartilhar por QR", w - 174., bottom, 153.);
            }
        }
        TabPage::Usb => {
            let rows = page_range(view.usb_page, view.usb.len());
            for (offset, i) in rows.clone().enumerate() {
                let y = top + offset as f32 * ROW_H + 6.;
                button(UsbOpen(i), "Abrir", w - 145., y, 56.);
                button(UsbEject(i), "Ejetar", w - 84., y, 63.);
            }
            if rows.start > 0 {
                button(
                    UsbPage(rows.start / ROWS_PER_PAGE - 1),
                    "Anterior",
                    w - 165.,
                    bottom,
                    67.,
                );
            }
            if rows.end < view.usb.len() {
                button(
                    UsbPage(rows.start / ROWS_PER_PAGE + 1),
                    "Próxima",
                    w - 90.,
                    bottom,
                    69.,
                );
            }
        }
        TabPage::Downloads => {
            let (top, count) = recent_layout(h, view);
            // Inside its row, centered, 5 px from the row's top, bottom and right edge.
            for i in 0..count { button(RecentShow(i), "Mostrar", w - 97., top + i as f32 * RECENT_H + 5., 72.); }
            button(OpenDownloads, "Abrir pasta Downloads", 20., bottom, 166.)
        }
        _ => {}
    }
    for (hit, _) in &mut result {
        if matches!(hit.action, UiAction::Phase8(RecentShow(_))) { hit.h = RECENT_H - RECENT_GAP - 10.; }
    }
    result.into_iter()
}

impl PillRenderer {
    pub(in crate::renderer) fn phase8_view(
        &mut self,
        tab: TabPage,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        view: &Phase8View,
        now_ms: i64,
    ) {
        let (title, empty) = match tab {
            TabPage::Clipboard => (
                "Área de transferência",
                "Copie texto, links, imagens ou arquivos para começar.",
            ),
            TabPage::Shelf => (
                "Prateleira",
                "Arraste um arquivo para a ilha ou use Adicionar arquivo.",
            ),
            TabPage::Usb => ("Dispositivos USB", "Nenhuma unidade USB conectada."),
            TabPage::Downloads => ("Downloads", "Nenhum download detectado agora."),
            _ => return,
        };
        self.label(title, x + 20., y + BODY, 18., Weight::Semibold, white());
        let top = y + BODY + if tab == TabPage::Clipboard { 77. } else { 39. };
        if let Some(share) = view.share.as_ref().filter(|_| tab == TabPage::Shelf) {
            self.draw_share(share, x + 20., top, w);
        } else {
            let has_rows = match tab {
                TabPage::Clipboard => {
                    for (n, i) in page_range(view.clipboard_page, view.clipboard.len()).enumerate()
                    {
                        let row = &view.clipboard[i];
                        let ry = top + n as f32 * ROW_H;
                        if let Some(thumb) = view.clipboard_thumbnails.get(&row.id) {
                            self.phase8_row("", x, ry, w, 40., 200.);
                            self.phase8_thumbnail(thumb, x + 25., ry + 3., 34.);
                            self.clipped_label(&row.label, x + 66., ry + 12., w - 300., 12., Weight::Medium, white());
                        } else { self.phase8_row(&row.label, x, ry, w, 40., 200.); }
                    }
                    !view.clipboard.is_empty()
                }
                TabPage::Shelf => {
                    for (n, name) in view.shelf.iter().take(6).enumerate() {
                        let ry = top + n as f32 * 37.;
                        if let Some(thumb) = view.shelf_thumbnails.get(&n) {
                            self.phase8_row("", x, ry, w, 36., 194.);
                            self.phase8_thumbnail(thumb, x + 25., ry + 3., 30.);
                            self.clipped_label(name, x + 61., ry + 10., w - 285., 12., Weight::Medium, white());
                        } else { self.phase8_row(name, x, ry, w, 36., 194.); }
                    }
                    !view.shelf.is_empty()
                }
                TabPage::Usb => {
                    for (n, i) in page_range(view.usb_page, view.usb.len()).enumerate() {
                        self.phase8_row(&view.usb[i], x, top + n as f32 * ROW_H, w, 40., 137.);
                    }
                    !view.usb.is_empty()
                }
                TabPage::Downloads => {
                    // The overall count and the taskbar's % (the Windhawk bridge) beside the title.
                    if let Some(progress) = view.downloads {
                        let mut summary = format!("{} em andamento", progress.count);
                        if let Some(p) = progress.permille { summary.push_str(&format!(" · {}%", p / 10)); }
                        self.label_right(&summary, x + w - 20., y + BODY + 5., Weight::Medium, muted());
                    }
                    for (n, row) in view.download_rows.iter().take(DOWNLOAD_ROWS).enumerate() {
                        self.download_card(&row.name, &download_description(row), Some(n), x, top + n as f32 * ACTIVE_H, w, view, now_ms);
                    }
                    let active = !view.download_rows.is_empty() || view.downloads.is_some();
                    if view.download_rows.is_empty() && let Some(progress) = view.downloads {
                        let detail = progress.permille.map_or_else(|| "Tamanho total ainda desconhecido".into(), |p| format!("{}% concluído", p / 10));
                        self.download_card("Download em andamento", &detail, None, x, top, w, view, now_ms);
                    }
                    let (recent_top, count) = recent_layout(h, view);
                    if !active && count > 0 {
                        self.clipped_label("Nenhum download agora.", x + 20., top + 8., w - 40., 13., Weight::Regular, muted());
                    }
                    if count > 0 {
                        self.label("Recentes", x + 20., y + recent_top - 20., 11., Weight::Semibold, muted());
                    }
                    for (i, file) in view.download_recent.iter().take(count).enumerate() {
                        let ry = y + recent_top + i as f32 * RECENT_H;
                        self.round(x + 20., ry, w - 40., RECENT_H - RECENT_GAP, 8., surface());
                        let meta = format!("{} · {}", size_text(file.size), ago_text(now_ms - file.modified_ms));
                        let meta_w = self.label_right(&meta, x + w - 112., ry + 10., Weight::Regular, muted());
                        self.clipped_label(&file.name, x + 34., ry + 10., w - 158. - meta_w, 12., Weight::Medium, white());
                    }
                    active || count > 0
                }
                _ => false,
            };
            if !has_rows {
                let empty = if tab == TabPage::Clipboard
                    && (!view.clipboard_query.is_empty() || view.clipboard_pins_only)
                {
                    "Nenhum item corresponde ao filtro."
                } else {
                    empty
                };
                self.clipped_label(
                    empty,
                    x + 20.,
                    top + 8.,
                    w - 40.,
                    13.,
                    Weight::Regular,
                    muted(),
                );
            }
        }
        if let Some((page, message)) = &view.message {
            if *page == tab {
                self.clipped_label(
                    message,
                    x + 20.,
                    y + h - 66.,
                    w - 40.,
                    11.,
                    Weight::Medium,
                    accent(),
                );
            }
        }
        for (hit, text) in controls(tab, x, y, w, h, view) {
            let search = hit.action == UiAction::Phase8(Phase8Action::SearchClips);
            self.round(
                hit.x,
                hit.y,
                hit.w,
                hit.h,
                8.,
                if search && view.clipboard_searching {
                    surface()
                } else {
                    tint()
                },
            );
            if search {
                let query = if view.clipboard_query.is_empty() && !view.clipboard_searching {
                    "Buscar no histórico".into()
                } else {
                    format!(
                        "{}{}",
                        view.clipboard_query,
                        if view.clipboard_searching { "|" } else { "" }
                    )
                };
                self.clipped_label(
                    &query,
                    hit.x + 10.,
                    hit.y + 7.,
                    hit.w - 20.,
                    11.,
                    Weight::Medium,
                    muted(),
                );
            } else {
                self.centered_label(
                    text,
                    hit.x + hit.w / 2.,
                    hit.y + (hit.h - 14.) / 2.,
                    11.,
                    Weight::Medium,
                    white(),
                );
            }
        }
    }

    /// One download in progress: its name and %, a bar like the pill's fill, and what's known of it.
    #[allow(clippy::too_many_arguments)]
    fn download_card(&mut self, name: &str, detail: &str, row: Option<usize>, x: f32, ry: f32, w: f32, view: &Phase8View, now_ms: i64) {
        self.round(x + 20., ry, w - 40., ACTIVE_H - 4., 8., surface());
        let fraction = view.bar_fraction(row);
        let percent_w = fraction.map_or(0., |f| self.label_right(&format!("{}%", (f * 100.).floor() as u32), x + w - 32., ry + 8., Weight::Medium, white()) + 8.);
        self.clipped_label(name, x + 32., ry + 7., w - 64. - percent_w, 13., Weight::Medium, white());
        let animated = view.bar_follows_fill(row) && view.download_animation && crate::window::animations_enabled();
        let value = if animated { Some(view.download_fill.value(now_ms)) } else { fraction };
        self.download_bar(x + 32., ry + 27., w - 64., 4., value, view.download_state, now_ms);
        self.clipped_label(detail, x + 32., ry + 35., w - 64., 10., Weight::Regular, muted());
    }

    /// Port of Island::UpdateFill's bar (mac-island 3.1.1): `value` full, or (None) a marker sliding along;
    /// yellow while paused, red on an error, as the taskbar shows them.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn download_bar(&mut self, x: f32, y: f32, w: f32, h: f32, value: Option<f32>, state: u32, now_ms: i64) {
        self.round(x, y, w, h, h / 2., line());
        let (dx, fill) = match value {
            Some(value) => (0., w * value.clamp(0., 1.)),
            None => {
                let marker = w.min((w * 0.15).max(28.));
                let offset = if crate::window::animations_enabled() { (w - marker) * (now_ms.rem_euclid(2200) as f32 / 2200.) } else { 0. };
                (offset, marker)
            }
        };
        let color = match state {
            4 => Color::from_rgba8(255, 69, 59, 255),
            8 => Color::from_rgba8(255, 204, 0, 255),
            _ => accent(),
        };
        self.round(x + dx, y, fill, h, h / 2., color);
    }

    fn phase8_row(&mut self, text: &str, x: f32, y: f32, w: f32, height: f32, reserved: f32) {
        self.round(x + 20., y, w - 40., height, 8., surface());
        self.clipped_label(
            text,
            x + 32.,
            y + (height - 15.) / 2.,
            w - 64. - reserved,
            13.,
            Weight::Medium,
            white(),
        );
    }

    // Port of isle clipRules.js CARD_PICTURE: keep the thumbnail's aspect inside its slot.
    fn phase8_thumbnail(&mut self, thumb: &isle_core::artwork::Artwork, x: f32, y: f32, size: f32) {
        let mut rgba = thumb.rgba.clone();
        for p in rgba.chunks_exact_mut(4) { for c in 0..3 { p[c] = (u16::from(p[c]) * u16::from(p[3]) / 255) as u8; } }
        let Some(dimensions) = tiny_skia::IntSize::from_wh(thumb.width, thumb.height) else { return; };
        let Some(image) = tiny_skia::Pixmap::from_vec(rgba, dimensions) else { return; };
        let scale = size * self.scale / thumb.width.max(thumb.height) as f32;
        self.layer.draw_pixmap(0, 0, image.as_ref(), &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() },
            tiny_skia::Transform::from_scale(scale, scale).post_translate(x * self.scale, (y + (size - thumb.height as f32 * scale / self.scale) / 2.) * self.scale), None);
    }

    fn draw_share(&mut self, share: &isle_core::phase8::ShareView, x: f32, y: f32, w: f32) {
        if share.size == 0 || share.size * share.size != share.modules.len() {
            return;
        }
        let available = (w * 0.42).min(212.);
        let cell = (available * self.scale / (share.size + 8) as f32)
            .floor()
            .max(1.)
            / self.scale;
        let side = (share.size + 8) as f32 * cell;
        self.round(x, y, side, side, 0., Color::WHITE);
        for (i, dark) in share.modules.iter().enumerate() {
            if *dark {
                self.round(
                    x + (i % share.size + 4) as f32 * cell,
                    y + (i / share.size + 4) as f32 * cell,
                    cell,
                    cell,
                    0.,
                    Color::BLACK,
                );
            }
        }
        let tx = x + side + 14.;
        let tw = w - side - 54.;
        self.clipped_label(
            "Escaneie no celular",
            tx,
            y + 12.,
            tw,
            14.,
            Weight::Semibold,
            white(),
        );
        self.clipped_label(
            "Use a mesma rede Wi-Fi",
            tx,
            y + 37.,
            tw,
            12.,
            Weight::Regular,
            muted(),
        );
        if let Some(code) = &share.pending_code {
            self.clipped_label(
                "Confira o código do celular",
                tx,
                y + 82.,
                tw,
                12.,
                Weight::Medium,
                white(),
            );
            self.label(code, tx, y + 107., 20., Weight::Semibold, accent());
        }
    }
}

/// The unit a size reads in, and its divisor (decimal, as browsers show them).
fn unit(bytes: u64) -> (f64, &'static str) {
    match bytes { b if b >= 1_000_000_000 => (1e9, "GB"), b if b >= 1_000_000 => (1e6, "MB"), _ => (1e3, "KB") }
}

/// A number the Brazilian way: whole from 100 up, else one decimal after a comma.
fn amount(value: f64) -> String {
    if value >= 100. { format!("{value:.0}") } else { format!("{value:.1}").replace('.', ",") }
}

fn size_text(bytes: u64) -> String {
    let (divisor, unit) = unit(bytes);
    format!("{} {unit}", amount(bytes as f64 / divisor))
}

fn duration_text(secs: u64) -> String {
    match secs { s if s < 60 => format!("{s} s"), s if s < 3600 => format!("{} min", s / 60), s => format!("{} h {} min", s / 3600, s % 3600 / 60) }
}

/// "há 3 min" for a file modified `ms` ago.
fn ago_text(ms: i64) -> String {
    match ms.max(0) / 60_000 { 0 => "agora".into(), m if m < 60 => format!("há {m} min"), m if m < 1440 => format!("há {} h", m / 60), m => format!("há {} d", m / 1440) }
}

// Port of the partial-file information shown by mac-island: only known measurements,
// as "568 de 861 MB · 12,3 MB/s · 7 s restantes".
fn download_description(row: &isle_core::phase8::DownloadRowView) -> String {
    let mut text = match row.total.filter(|v| *v > 0) {
        Some(total) => {
            let (divisor, unit) = unit(total);
            format!("{} de {} {unit}", amount(row.bytes.min(total) as f64 / divisor), amount(total as f64 / divisor))
        }
        None => size_text(row.bytes),
    };
    if let Some(rate) = row.bytes_per_second.filter(|r| *r > 0) {
        let (divisor, unit) = unit(rate);
        text.push_str(&format!(" · {}/s", format!("{:.1} {unit}", rate as f64 / divisor).replace('.', ",")));
        if let Some(total) = row.total { text.push_str(&format!(" · {} restantes", duration_text(total.saturating_sub(row.bytes) / rate))); }
    }
    text
}

const ACTIVE_H: f32 = 54.;
const RECENT_H: f32 = 38.;
const RECENT_GAP: f32 = 6.;

/// Where the Downloads tab's "Recentes" rows start (below the panel's top) and how many fit above its footer.
fn recent_layout(h: f32, view: &Phase8View) -> (f32, usize) {
    let active = if view.download_rows.is_empty() { usize::from(view.downloads.is_some()) } else { view.download_rows.len().min(DOWNLOAD_ROWS) };
    let top = BODY + 39. + if active == 0 { 30. } else { active as f32 * ACTIVE_H } + 24.;
    let fit = ((h - 70. - top) / RECENT_H).floor().max(0.) as usize;
    (top, view.download_recent.len().min(fit).min(5))
}

/// Where a click opens recent download `i`: its row left of its "Mostrar" button.
pub fn download_recent_rows(x: f32, y: f32, w: f32, h: f32, view: &Phase8View) -> impl Iterator<Item = HitBox> {
    let (top, count) = recent_layout(h, view);
    (0..count).map(move |i| HitBox {
        x: x + 20., y: y + top + i as f32 * RECENT_H, w: w - 128., h: RECENT_H - RECENT_GAP, action: UiAction::Phase8(Phase8Action::RecentOpen(i)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shelf_rows_stay_clear_of_the_buttons() {
        let view = Phase8View { shelf: vec!["a".into(), "b".into()], ..Phase8View::default() };
        let rows: Vec<_> = shelf_rows(0., 0., 560., &view).collect();
        assert_eq!(rows.len(), 2);
        let open = controls(TabPage::Shelf, 0., 0., 560., 450., &view).find(|(h, _)| h.action == UiAction::Phase8(Phase8Action::ShelfOpen(0))).unwrap().0;
        assert!(rows[0].x + rows[0].w < open.x && !rows[0].contains(open.x + 1., open.y + 1.));
    }
    #[test]
    fn download_details_only_estimate_time_from_a_known_total_and_nonzero_rate() {
        let mut row = isle_core::phase8::DownloadRowView { name: "arquivo.zip".into(), bytes: 568_000_000, total: Some(861_000_000), bytes_per_second: Some(12_300_000) };
        assert_eq!(download_description(&row), "568 de 861 MB · 12,3 MB/s · 23 s restantes");
        row.bytes_per_second = Some(500_000);
        assert_eq!(download_description(&row), "568 de 861 MB · 500,0 KB/s · 9 min restantes");
        row.total = None;
        assert_eq!(download_description(&row), "568 MB · 500,0 KB/s");
        row.bytes = 2_500_000_000;
        row.bytes_per_second = None;
        assert_eq!(download_description(&row), "2,5 GB");
        assert_eq!((ago_text(30_000), ago_text(3 * 60_000), ago_text(2 * 3_600_000), ago_text(3 * 86_400_000)), ("agora".into(), "há 3 min".into(), "há 2 h".into(), "há 3 d".into()));
    }

    #[test]
    fn recent_downloads_fit_above_the_footer_and_their_rows_miss_the_button() {
        use isle_core::phase8::{DownloadRowView, DownloadView, RecentDownloadView};
        let recent = (0..5).map(|i| RecentDownloadView { name: format!("f{i}"), size: 1, modified_ms: 0 }).collect();
        let row = DownloadRowView { name: "a".into(), bytes: 1, total: None, bytes_per_second: None };
        let mut view = Phase8View { download_recent: recent, ..Phase8View::default() };
        assert_eq!(recent_layout(450., &view).1, 5);
        view.downloads = Some(DownloadView { count: 2, permille: None });
        view.download_rows = vec![row.clone(), row];
        let (top, count) = recent_layout(450., &view);
        assert!(count >= 3 && top + count as f32 * RECENT_H <= 450. - 70.);
        let rows: Vec<_> = download_recent_rows(0., 0., 560., 450., &view).collect();
        let show = controls(TabPage::Downloads, 0., 0., 560., 450., &view).find(|(h, _)| h.action == UiAction::Phase8(Phase8Action::RecentShow(0))).unwrap().0;
        assert_eq!(rows.len(), count);
        assert!(rows[0].x + rows[0].w < show.x);
    }

    #[test]
    fn phase9_panels_render_details_and_image_cards_at_both_scales() {
        use isle_core::{phase8::{ClipRow, DownloadRowView, DownloadView}, spring::PillGeometry, state::{IslandState, PillMode}};
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/phase9-previews");
        std::fs::create_dir_all(&dir).unwrap();
        let mut cfg = isle_core::config::IsleConfig::default();
        cfg.modules.clipboard = true; cfg.modules.shelf = true; cfg.modules.downloads = true;
        let rgba = (0..272 * 216).flat_map(|i| if i % 272 < 136 { [225u8, 90, 60, 255] } else { [70u8, 150, 220, 255] }).collect();
        let thumb = std::sync::Arc::new(isle_core::artwork::Artwork::new(272, 216, rgba).unwrap());
        for scale in [1., 1.5] {
            for (name, tab) in [("downloads", TabPage::Downloads), ("clipboard", TabPage::Clipboard), ("shelf", TabPage::Shelf)] {
                let mut state = IslandState::default();
                state.modules = cfg.modules;
                state.mode = PillMode::Expanded(tab);
                state.clock_text = "22:41".into();
                state.phase8.clipboard = vec![ClipRow { id: 1, label: "Imagem · 1920 × 1080".into(), pinned: true }];
                state.phase8.clipboard_thumbnails.insert(1, thumb.clone());
                state.phase8.shelf = vec!["foto do celular.png".into()]; state.phase8.shelf_capacity = 6;
                state.phase8.shelf_thumbnails.insert(0, thumb.clone());
                state.phase8.downloads = Some(DownloadView { count: 2, permille: Some(640) });
                state.phase8.download_rows = vec![
                    DownloadRowView { name: "Wardstead-build.zip".into(), bytes: 25_000_000, total: None, bytes_per_second: Some(2_000_000) },
                    DownloadRowView { name: "referências.pdf".into(), bytes: 750_000, total: Some(1_000_000), bytes_per_second: Some(50_000) },
                ];
                let (w, h, r) = crate::layout::expanded_size(tab);
                let mut geom = PillGeometry::default(); geom.morph_to(w, h, r);
                geom.width.value = w; geom.height.value = h; geom.corner_radius.value = r;
                let mut renderer = PillRenderer::with_scale((720. * scale) as u32, (520. * scale) as u32, scale);
                let fb = renderer.render(&geom, &state, &cfg);
                let rgba = fb.bgra_premultiplied.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
                let image = tiny_skia::Pixmap::from_vec(rgba, tiny_skia::IntSize::from_wh(fb.width, fb.height).unwrap()).unwrap();
                assert!(image.pixels().iter().any(|p| p.alpha() > 0));
                image.save_png(dir.join(format!("{name}-{}.png", (scale * 100.) as u32))).unwrap();
            }
        }
    }
}
