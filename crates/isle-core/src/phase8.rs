//! Presentation state for the files and devices panels.

pub const ROWS_PER_PAGE: usize = 5;

#[derive(Debug, Clone)]
pub struct ClipRow {
    pub id: u64,
    pub label: String,
    pub pinned: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Phase8View {
    pub clipboard: Vec<ClipRow>,
    pub clipboard_page: usize,
    pub clipboard_query: String,
    pub clipboard_searching: bool,
    pub clipboard_pins_only: bool,
    pub clipboard_has_recent: bool,
    pub clipboard_thumbnails: std::collections::HashMap<u64, std::sync::Arc<crate::artwork::Artwork>>,
    pub clipboard_paste: bool,
    pub shelf_thumbnails: std::collections::HashMap<usize, std::sync::Arc<crate::artwork::Artwork>>,
    pub shelf: Vec<String>,
    pub shelf_capacity: usize,
    pub shelf_qr: bool,
    pub shelf_receive: bool,
    pub share: Option<ShareView>,
    pub usb: Vec<String>,
    pub usb_page: usize,
    pub downloads: Option<DownloadView>,
    pub download_meter: bool,
    pub download_rows: Vec<DownloadRowView>,
    pub download_state: u32,
    pub download_animation: bool,
    pub download_fill: DownloadFill,
    /// The newest finished files in Downloads, newest first.
    pub download_recent: Vec<RecentDownloadView>,
    pub message: Option<(crate::state::TabPage, String)>,
}

#[derive(Debug, Clone, Default)]
pub struct RecentDownloadView {
    pub name: String,
    pub size: u64,
    pub modified_ms: i64,
}

/// The Downloads tab draws at most this many in-progress rows.
pub const DOWNLOAD_ROWS: usize = 2;

impl Phase8View {
    /// How full a bar in the Downloads tab is (`None`: it slides, no total known): row `i`, or the summary
    /// (`None`). With one row the taskbar's overall % is that row's.
    pub fn bar_fraction(&self, row: Option<usize>) -> Option<f32> {
        let overall = self.downloads.and_then(|d| d.permille).map(|p| f32::from(p) / 1000.);
        let Some(i) = row else { return overall };
        let r = self.download_rows.get(i)?;
        let own = r.total.filter(|t| *t > 0).map(|t| (r.bytes.min(t) as f64 / t as f64) as f32);
        if self.download_rows.len() == 1 { overall.or(own) } else { own }
    }

    /// Whether a bar in the Downloads tab follows the pill's fill tween (the taskbar's %).
    pub fn bar_follows_fill(&self, row: Option<usize>) -> bool {
        self.downloads.is_some_and(|d| d.permille.is_some()) && (row.is_none() || self.download_rows.len() == 1)
    }

    /// Whether the Downloads tab shows a sliding (indeterminate) bar: it needs frames while open.
    pub fn bar_sliding(&self) -> bool {
        if self.download_rows.is_empty() { self.downloads.is_some() && self.bar_fraction(None).is_none() }
        else { (0..self.download_rows.len().min(DOWNLOAD_ROWS)).any(|i| self.bar_fraction(Some(i)).is_none()) }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DownloadRowView {
    pub name: String,
    pub bytes: u64,
    pub total: Option<u64>,
    pub bytes_per_second: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DownloadFill {
    pub from: f32,
    pub target: f32,
    pub at_ms: i64,
    pub ending: bool,
}

impl DownloadFill {
    // Port of Island::UpdateFill (mac-island 3.1.1): smooth start/value and done/cancel tail.
    pub fn update(&mut self, progress: Option<u16>, active: bool, now: i64) {
        let current = self.value(now);
        let target = if active { progress.map_or(0.25, |p| f32::from(p) / 1000.) }
            else if self.target >= 0.97 { 1. } else { 0. };
        // The same reading again (each details event repeats it) keeps the tween going, not restarted.
        if target == self.target && self.ending != active { return; }
        self.from = current;
        self.target = target;
        self.at_ms = now;
        self.ending = !active;
    }
    pub fn value(&self, now: i64) -> f32 {
        let t = ((now - self.at_ms).max(0) as f32 / 450.).min(1.);
        self.from + (self.target - self.from) * (1. - (1. - t).powi(3))
    }
    pub fn visible(&self, now: i64) -> bool { !self.ending || now - self.at_ms < 900 }
    /// Still moving: the tween, or the end tail. Frames are only needed while this holds.
    pub fn is_animating(&self, now: i64) -> bool { now - self.at_ms < if self.ending { 900 } else { 450 } }
}

#[cfg(test)]
mod tests {
    #[test]
    fn download_fill_animates_only_while_it_moves() {
        let mut fill = DownloadFill::default();
        fill.update(Some(500), true, 1000);
        assert!(fill.is_animating(1100) && !fill.is_animating(1450));
        fill.update(None, false, 2000);
        assert!(fill.is_animating(2800) && !fill.is_animating(2900));
    }

    #[test]
    fn download_fill_ignores_a_repeated_reading() {
        let mut fill = DownloadFill::default();
        fill.update(Some(500), true, 1000);
        fill.update(Some(500), true, 1400);
        assert!(!fill.is_animating(1450));
        fill.update(None, false, 2000);
        fill.update(None, false, 2800);
        assert!(!fill.is_animating(2900));
        fill.update(Some(600), true, 3000);
        assert!(fill.is_animating(3100));
    }

    use super::*;
    #[test]
    fn one_download_takes_the_taskbar_percent_and_several_keep_their_own() {
        let row = |bytes, total| DownloadRowView { name: "a".into(), bytes, total, bytes_per_second: None };
        let mut view = Phase8View { downloads: Some(DownloadView { count: 1, permille: Some(660) }), download_rows: vec![row(10, None)], ..Default::default() };
        assert_eq!(view.bar_fraction(Some(0)), Some(0.66));
        assert!(view.bar_follows_fill(Some(0)) && !view.bar_sliding());
        view.download_rows.push(row(50, Some(100)));
        assert_eq!(view.bar_fraction(Some(1)), Some(0.5));
        assert_eq!(view.bar_fraction(Some(0)), None);
        assert!(view.bar_sliding() && !view.bar_follows_fill(Some(1)) && view.bar_follows_fill(None));
        view.downloads = Some(DownloadView { count: 1, permille: None });
        view.download_rows.clear();
        assert!(view.bar_sliding());
    }

    #[test]
    fn download_fill_finishes_near_complete_and_drains_cancelled_work() {
        let mut fill = DownloadFill::default();
        fill.update(Some(980), true, 1000);
        assert_eq!(fill.value(1000), 0.);
        assert!((fill.value(1450) - 0.98).abs() < 0.001);
        fill.update(None, false, 1500);
        assert_eq!(fill.target, 1.);
        assert!(fill.visible(2000));
        assert!(!fill.visible(2400));
        fill.update(Some(250), true, 2500);
        fill.update(None, false, 3000);
        assert_eq!(fill.value(3450), 0.);
    }
}

#[derive(Debug, Clone)]
pub struct ShareView {
    /// Modules in row-major order. The renderer adds the white quiet zone.
    pub modules: std::sync::Arc<[bool]>,
    pub size: usize,
    pub pending_code: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct DownloadView {
    pub count: usize,
    /// None when a source can confirm activity but cannot report a total.
    pub permille: Option<u16>,
}
