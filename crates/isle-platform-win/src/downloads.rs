// SPDX-License-Identifier: GPL-3.0-or-later
//
// Mac Island & Dock by caliberda reads Explorer's taskbar progress from inside
// Explorer. Cimori uses the current user's BITS jobs and Downloads folder change
// notifications as standalone progress sources.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadProgress {
    pub active_count: usize,
    /// Byte-weighted progress, when all active downloads have known totals.
    pub permille: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadRow {
    pub name: String,
    pub bytes: u64,
    pub total: Option<u64>,
    pub bytes_per_second: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DownloadSnapshot {
    pub progress: Option<DownloadProgress>,
    /// TBPF_NORMAL (2), TBPF_ERROR (4), TBPF_PAUSED (8), or indeterminate (1).
    pub state: u32,
    pub rows: Vec<DownloadRow>,
}

// Port of PublishProgress's state/value IPC (mac-island 3.1.1), versioned for WM_COPYDATA.
fn parse_progress(bytes: &[u8]) -> Option<(u32, Option<u16>, usize)> {
    if bytes.len() != 16 { return None; }
    let fields: Vec<u32> = bytes.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).collect();
    let [version, state, value, count] = fields[..] else { return None; };
    if version != 1 || !matches!(state, 0 | 1 | 2 | 4 | 8) || count > 512 { return None; }
    let value = if value == u32::MAX { None } else { Some(u16::try_from(value).ok().filter(|v| *v <= 1000)?) };
    Some((state, value, count as usize))
}

/// The partial-file endings browsers use while they download.
const PARTIAL: [&str; 4] = ["crdownload", "part", "partial", "download"];

fn is_partial(path: &std::path::Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| PARTIAL.iter().any(|p| ext.eq_ignore_ascii_case(p)))
}

/// A partial file's name without its ending; Chrome's "Não confirmado 123.crdownload" (no name yet) reads
/// as "Download do navegador".
fn display_name(file: &std::path::Path) -> String {
    let stem = if is_partial(file) { file.file_stem() } else { file.file_name() }.unwrap_or_default().to_string_lossy().into_owned();
    let unnamed = ["Não confirmado ", "Unconfirmed "].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
    });
    if unnamed { "Download do navegador".into() } else { stem }
}

/// The whole size of a download from its bytes so far and the taskbar's ‰ (the Windhawk bridge).
fn total_from_permille(bytes: u64, permille: u16) -> Option<u64> {
    (bytes > 0 && permille > 0).then(|| (u128::from(bytes) * 1000 / u128::from(permille)) as u64).map(|total| total.max(bytes))
}

/// A finished file in the Downloads folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentDownload {
    pub path: std::path::PathBuf,
    pub name: String,
    pub size: u64,
    pub modified: std::time::SystemTime,
}

/// The `limit` newest files in `folder` by modified time, leaving out partial, hidden and system files.
pub fn recent_in(folder: &std::path::Path, limit: usize) -> Vec<RecentDownload> {
    let mut files: Vec<_> = std::fs::read_dir(folder).into_iter().flatten().flatten().filter_map(|entry| {
        let path = entry.path();
        let metadata = entry.metadata().ok().filter(|m| m.is_file())?;
        #[cfg(windows)]
        if std::os::windows::fs::MetadataExt::file_attributes(&metadata) & 0x6 != 0 { return None; } // HIDDEN | SYSTEM
        if is_partial(&path) { return None; }
        Some(RecentDownload { name: display_name(&path), size: metadata.len(), modified: metadata.modified().ok()?, path })
    }).collect();
    files.sort_by(|a, b| b.modified.cmp(&a.modified));
    files.truncate(limit);
    files
}

/// The newest finished files in the user's Downloads folder (none when it can't be found).
pub fn recent_downloads(limit: usize) -> Vec<RecentDownload> {
    #[cfg(windows)]
    if let Some(folder) = native::downloads_folder() { return recent_in(&folder, limit); }
    let _ = limit;
    Vec::new()
}

#[cfg(windows)]
pub use native::{subscribe, subscribe_detailed, DownloadWatcher};

#[cfg(not(windows))]
pub fn subscribe_detailed<F>(_: bool, publish: F) -> DownloadWatcher
where F: Fn(DownloadSnapshot) + Send + 'static {
    publish(DownloadSnapshot::default());
    DownloadWatcher
}

#[cfg(not(windows))]
pub struct DownloadWatcher;

#[cfg(not(windows))]
pub fn subscribe<F>(publish: F) -> DownloadWatcher
where
    F: Fn(Option<DownloadProgress>) + Send + 'static,
{
    publish(None);
    DownloadWatcher
}

#[cfg(any(windows, test))]
fn summarize_downloads(
    partial_count: usize,
    jobs: impl IntoIterator<Item = (u64, u64)>,
) -> Option<DownloadProgress> {
    let mut active_count = partial_count;
    let mut known = partial_count == 0;
    let mut transferred = 0u128;
    let mut total = 0u128;
    for (bytes, size) in jobs {
        active_count += 1;
        if size == 0 || size == u64::MAX {
            known = false;
        } else {
            transferred += u128::from(bytes.min(size));
            total += u128::from(size);
        }
    }
    (active_count > 0).then(|| DownloadProgress {
        active_count,
        permille: known
            .then(|| (transferred * 1000).checked_div(total))
            .flatten()
            .map(|value| value as u16),
    })
}

#[cfg(any(windows, test))]
#[derive(Default)]
struct BrowserDownloads {
    files: std::collections::HashMap<std::ffi::OsString, (u64, std::time::SystemTime)>,
    sampled: Option<std::time::SystemTime>,
    rows: Vec<DownloadRow>,
}

#[cfg(any(windows, test))]
impl BrowserDownloads {
    // Browser partial files can be left behind after cancellation or an app crash.
    // A growing file still counts when Windows defers updating its write timestamp.
    fn scan(
        &mut self,
        folder: &std::path::Path,
        now: std::time::SystemTime,
    ) -> (usize, Option<std::time::Duration>) {
        const RECENT: std::time::Duration = std::time::Duration::from_secs(120);
        let mut files = std::collections::HashMap::new();
        let mut active = 0;
        let mut earliest_expiry: Option<std::time::Duration> = None;
        let elapsed = self.sampled.and_then(|at| now.duration_since(at).ok()).map(|d| d.as_secs_f64());
        self.rows.clear();
        for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
            let path = entry.path();
            if !is_partial(&path) || !entry.file_type().is_ok_and(|kind| kind.is_file()) {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let Ok(modified) = metadata.modified() else {
                continue;
            };
            let name = entry.file_name();
            let size = metadata.len();
            let last_activity = match self.files.get(&name) {
                Some(&(previous_size, _)) if size != previous_size => now,
                Some(&(_, previous_activity)) => modified.max(previous_activity),
                None => modified,
            };
            files.insert(name.clone(), (size, last_activity));
            let age = now.duration_since(last_activity).unwrap_or_default();
            if let Some(expiry) = RECENT.checked_sub(age).filter(|left| !left.is_zero()) {
                active += 1;
                // Port of mac-island's growing partial-file measurements; never infer a
                // per-file total from an unrelated taskbar window's combined percentage.
                let rate = self.files.get(&name).and_then(|(old, _)| {
                    elapsed.filter(|s| *s > 0.).map(|s| (size.saturating_sub(*old) as f64 / s) as u64)
                });
                self.rows.push(DownloadRow {
                    name: display_name(&path),
                    bytes: size, total: None, bytes_per_second: rate,
                });
                earliest_expiry =
                    Some(earliest_expiry.map_or(expiry, |current| current.min(expiry)));
            }
        }
        self.files = files;
        self.sampled = Some(now);
        self.rows.sort_by(|a, b| a.name.cmp(&b.name));
        (active, earliest_expiry)
    }
}

#[cfg(windows)]
mod native {
    use super::{summarize_downloads, BrowserDownloads, DownloadProgress, DownloadSnapshot};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::{Duration, Instant, SystemTime};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
    use windows::Win32::Networking::BackgroundIntelligentTransferService::{
        BackgroundCopyManager, IBackgroundCopyManager, BG_JOB_PROGRESS, BG_JOB_STATE_CONNECTING,
        BG_JOB_STATE_TRANSFERRING, BG_JOB_TYPE_DOWNLOAD,
    };
    use windows::Win32::Storage::FileSystem::{
        FindCloseChangeNotification, FindFirstChangeNotificationW, FindNextChangeNotification,
        FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_CHANGE_SIZE,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_LOCAL_SERVER,
        COINIT_MULTITHREADED,
    };
    use windows::Win32::System::Threading::{CreateEventW, SetEvent, WaitForMultipleObjects};
    use windows::Win32::UI::Shell::{FOLDERID_Downloads, SHGetKnownFolderPath, KF_FLAG_DEFAULT};

    const IDLE_BITS_INTERVAL: Duration = Duration::from_secs(10);
    const ACTIVE_BITS_INTERVAL: Duration = Duration::from_secs(1);
    const DIRECTORY_DEBOUNCE: Duration = Duration::from_millis(250);

    pub struct DownloadWatcher {
        stop: Option<Arc<OwnedHandle>>,
    }

    impl Drop for DownloadWatcher {
        fn drop(&mut self) {
            if let Some(stop) = &self.stop {
                unsafe {
                    let _ = SetEvent(HANDLE(stop.as_raw_handle()));
                }
            }
        }
    }

    pub fn subscribe<F>(publish: F) -> DownloadWatcher
    where
        F: Fn(Option<DownloadProgress>) + Send + 'static,
    {
        subscribe_detailed(false, move |snapshot| publish(snapshot.progress))
    }

    pub fn subscribe_detailed<F>(taskbar: bool, publish: F) -> DownloadWatcher
    where F: Fn(DownloadSnapshot) + Send + 'static {
        let stop = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }
            .inspect_err(
                |err| tracing::debug!(%err, "Download watcher could not create its stop event"),
            )
            .ok()
            .map(|handle| Arc::new(unsafe { OwnedHandle::from_raw_handle(handle.0) }));
        if let Some(worker_stop) = stop.clone() {
            if let Err(err) = std::thread::Builder::new()
                .name("cimori-downloads".into())
                .spawn(move || watch(&worker_stop, taskbar, publish))
            {
                tracing::debug!(%err, "Could not start download watcher");
            }
        }
        DownloadWatcher { stop }
    }

    struct DirectoryWatch(HANDLE);

    impl DirectoryWatch {
        fn open(path: &Path) -> Option<Self> {
            let path: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
            unsafe {
                FindFirstChangeNotificationW(
                    PCWSTR(path.as_ptr()),
                    false,
                    FILE_NOTIFY_CHANGE_FILE_NAME
                        | FILE_NOTIFY_CHANGE_LAST_WRITE
                        | FILE_NOTIFY_CHANGE_SIZE,
                )
                .ok()
                .map(Self)
            }
        }
    }

    impl Drop for DirectoryWatch {
        fn drop(&mut self) {
            unsafe {
                let _ = FindCloseChangeNotification(self.0);
            }
        }
    }

    pub(super) fn downloads_folder() -> Option<PathBuf> {
        unsafe {
            let path =
                SHGetKnownFolderPath(&FOLDERID_Downloads, KF_FLAG_DEFAULT, HANDLE::default())
                    .ok()?;
            let folder = PathBuf::from(std::ffi::OsString::from_wide(path.as_wide()));
            CoTaskMemFree(Some(path.0.cast()));
            Some(folder)
        }
    }

    // BITS supports callbacks per job. Enumerating another application's jobs does not
    // give a queue-wide subscription, and setting a job callback would replace its owner's.
    fn bits_downloads(manager: &IBackgroundCopyManager) -> Vec<(u64, u64)> {
        unsafe {
            let Ok(jobs) = manager.EnumJobs(0) else {
                return Vec::new();
            };
            let mut downloads = Vec::new();
            loop {
                let mut job = [None];
                let mut fetched = 0;
                if jobs.Next(&mut job, &mut fetched).is_err() || fetched == 0 {
                    break;
                }
                let Some(job) = job[0].take() else { continue };
                if job.GetType().ok() != Some(BG_JOB_TYPE_DOWNLOAD)
                    || !matches!(
                        job.GetState().ok(),
                        Some(BG_JOB_STATE_CONNECTING | BG_JOB_STATE_TRANSFERRING)
                    )
                {
                    continue;
                }
                let mut progress = BG_JOB_PROGRESS::default();
                downloads.push(if job.GetProgress(&mut progress).is_ok() {
                    (progress.BytesTransferred, progress.BytesTotal)
                } else {
                    (0, 0)
                });
            }
            downloads
        }
    }

    struct Com;

    impl Com {
        fn initialize() -> Option<Self> {
            unsafe {
                CoInitializeEx(None, COINIT_MULTITHREADED)
                    .ok()
                    .ok()
                    .map(|_| Self)
            }
        }
    }

    impl Drop for Com {
        fn drop(&mut self) {
            unsafe { CoUninitialize() }
        }
    }

    fn watch(stop: &OwnedHandle, taskbar: bool, publish: impl Fn(DownloadSnapshot)) {
        let _com = Com::initialize();
        let mut manager: Option<IBackgroundCopyManager> = None;
        let mut folder = downloads_folder();
        let mut directory = folder.as_deref().and_then(DirectoryWatch::open);
        let mut bits = Vec::new();
        let mut partial_count = 0;
        let mut browser = BrowserDownloads::default();
        let mut last = None;
        let bridge = taskbar.then(|| super::taskbar::Host::start()).flatten();
        let mut next_bits = Instant::now();
        let mut next_scan = Some(next_bits);
        let mut directory_signaled = false;
        let stop = HANDLE(stop.as_raw_handle());

        loop {
            let now = Instant::now();
            if now >= next_bits {
                if manager.is_none() && _com.is_some() {
                    manager = unsafe {
                        CoCreateInstance(&BackgroundCopyManager, None, CLSCTX_LOCAL_SERVER).ok()
                    };
                }
                bits = manager.as_ref().map(bits_downloads).unwrap_or_default();
                next_bits = now
                    + if bits.is_empty() {
                        IDLE_BITS_INTERVAL
                    } else {
                        ACTIVE_BITS_INTERVAL
                    };

                // A missing or moved Downloads folder can be restored after startup.
                if directory.is_none() {
                    folder = downloads_folder();
                    directory = folder.as_deref().and_then(DirectoryWatch::open);
                    if directory.is_some() {
                        next_scan = Some(now);
                    }
                }
            }
            if next_scan.is_some_and(|scan| scan <= now) {
                if directory_signaled {
                    directory_signaled = false;
                    if directory
                        .as_ref()
                        .is_some_and(|d| unsafe { FindNextChangeNotification(d.0).is_err() })
                    {
                        directory = None;
                    }
                }
                let (count, earliest_expiry) = folder
                    .as_deref()
                    .map(|path| browser.scan(path, SystemTime::now()))
                    .unwrap_or_default();
                partial_count = count;
                next_scan = earliest_expiry.map(|expiry| now + expiry.min(Duration::from_secs(1)));
            }

            let mut snapshot = DownloadSnapshot {
                progress: summarize_downloads(partial_count, bits.iter().copied()),
                state: 2, rows: browser.rows.clone(),
            };
            if let Some((state, permille, count)) = bridge.as_ref().and_then(|b| b.latest()) {
                if state != 0 {
                    snapshot.progress = Some(DownloadProgress { active_count: count.max(partial_count), permille });
                    snapshot.state = state;
                    // One download: the taskbar's % is its own, so its total follows from the bytes so far.
                    if let ([row], Some(permille), true) = (&mut snapshot.rows[..], permille, count <= 1) {
                        row.total = super::total_from_permille(row.bytes, permille);
                    }
                }
            }
            if last.as_ref() != Some(&snapshot) {
                publish(snapshot.clone());
                last = Some(snapshot);
            }

            let mut handles = [stop; 3];
            let mut handle_count = 1;
            if let Some(bridge) = bridge.as_ref() {
                handles[1] = bridge.signal();
                handle_count = 2;
            }
            let directory_slot = handle_count;
            if let Some(directory) = directory.as_ref().filter(|_| !directory_signaled) {
                handles[handle_count] = directory.0;
                handle_count += 1;
            }
            let mut deadline = next_scan.map_or(next_bits, |scan| next_bits.min(scan));
            // The bridge's value goes stale after 5 s: looked at each second only while it shows a download.
            if bridge.as_ref().and_then(|b| b.latest()).is_some_and(|(state, ..)| state != 0) { deadline = deadline.min(now + Duration::from_secs(1)); }
            let timeout = deadline
                .saturating_duration_since(Instant::now())
                .as_millis() as u32
                + 1;
            let wait = unsafe { WaitForMultipleObjects(&handles[..handle_count], false, timeout) };
            if wait == WAIT_OBJECT_0 {
                return;
            }
            if bridge.is_some() && wait.0 == WAIT_OBJECT_0.0 + 1 {
                // A taskbar packet: the loop publishes it.
            } else if handle_count > directory_slot && wait.0 == WAIT_OBJECT_0.0 + directory_slot as u32 {
                directory_signaled = true;
                next_scan = Some(Instant::now() + DIRECTORY_DEBOUNCE);
            } else if wait != WAIT_TIMEOUT {
                if last.is_some() {
                    publish(DownloadSnapshot::default());
                }
                return;
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn directory_watch_signals_creation_and_rearms_for_removal() {
            let folder = std::env::temp_dir().join(format!(
                "cimori-directory-watch-test-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&folder).unwrap();
            let path = folder.join("example.crdownload");
            let watcher = DirectoryWatch::open(&folder).unwrap();
            std::fs::File::create(&path).unwrap();
            let created = unsafe { WaitForMultipleObjects(&[watcher.0], false, 1_000) };
            unsafe { FindNextChangeNotification(watcher.0) }.unwrap();
            std::fs::remove_file(&path).unwrap();
            let removed = unsafe { WaitForMultipleObjects(&[watcher.0], false, 1_000) };
            drop(watcher);
            std::fs::remove_dir(&folder).unwrap();
            assert_eq!(created, WAIT_OBJECT_0);
            assert_eq!(removed, WAIT_OBJECT_0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_packet_refuses_invalid_states_lengths_and_percentages() {
        let packet = |state: u32, value: u32| [1u32, state, value, 2].into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>();
        assert_eq!(parse_progress(&packet(8, 750)), Some((8, Some(750), 2)));
        assert_eq!(parse_progress(&packet(1, u32::MAX)), Some((1, None, 2)));
        assert!(parse_progress(&packet(3, 10)).is_none());
        assert!(parse_progress(&packet(2, 1001)).is_none());
        assert!(parse_progress(&packet(2, 5)[..15]).is_none());
    }

    #[test]
    fn partial_names_lose_their_ending_and_chrome_unnamed_files_read_as_a_browser_download() {
        let name = |n: &str| display_name(std::path::Path::new(n));
        assert_eq!(name("video.mp4.crdownload"), "video.mp4");
        assert_eq!(name("setup.exe.PART"), "setup.exe");
        assert_eq!(name("iso.img.partial"), "iso.img");
        assert_eq!(name("Não confirmado 123456.crdownload"), "Download do navegador");
        assert_eq!(name("Unconfirmed 9.crdownload"), "Download do navegador");
        assert_eq!(name("Não confirmado plano.pdf.crdownload"), "Não confirmado plano.pdf");
        assert_eq!(total_from_permille(568_000_000, 660), Some(860_606_060));
        assert_eq!(total_from_permille(0, 500), None);
        assert_eq!(total_from_permille(10, 0), None);
    }

    #[test]
    fn recent_lists_the_newest_finished_files_without_partials_or_hidden_ones() {
        use std::fs::{File, FileTimes};
        use std::time::{Duration, SystemTime};
        let folder = std::env::temp_dir().join(format!("cimori-recent-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let now = SystemTime::now();
        for (i, name) in ["a.zip", "b.pdf", "c.png", "d.txt", "e.mp3", "f.iso", "g.crdownload"].iter().enumerate() {
            File::create(folder.join(name)).unwrap().set_times(FileTimes::new().set_modified(now - Duration::from_secs(i as u64 * 60))).unwrap();
        }
        std::fs::create_dir(folder.join("pasta")).unwrap();
        let hidden = folder.join("desktop.ini");
        File::create(&hidden).unwrap();
        std::process::Command::new("attrib").args(["+h", "+s"]).arg(&hidden).status().unwrap();
        let names: Vec<_> = recent_in(&folder, 5).into_iter().map(|r| r.name).collect();
        std::process::Command::new("attrib").args(["-h", "-s"]).arg(&hidden).status().unwrap();
        std::fs::remove_dir_all(&folder).unwrap();
        assert_eq!(names, ["a.zip", "b.pdf", "c.png", "d.txt", "e.mp3"]);
    }

    #[test]
    fn partial_file_rows_measure_growth_without_guessing_a_total() {
        let folder = std::env::temp_dir().join(format!("cimori-rows-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("video.mp4.crdownload");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(100).unwrap();
        let now = std::time::SystemTime::now();
        let mut browser = BrowserDownloads::default();
        browser.scan(&folder, now);
        file.set_len(1100).unwrap();
        browser.scan(&folder, now + std::time::Duration::from_secs(2));
        assert_eq!(browser.rows[0].name, "video.mp4");
        assert_eq!(browser.rows[0].bytes, 1100);
        assert_eq!(browser.rows[0].bytes_per_second, Some(500));
        assert_eq!(browser.rows[0].total, None);
        drop(file);
        std::fs::remove_file(path).unwrap();
        browser.scan(&folder, now);
        assert!(browser.rows.is_empty());
        std::fs::remove_dir(folder).unwrap();
    }

    #[test]
    fn computes_byte_weighted_progress_and_clamps_overruns() {
        assert_eq!(
            summarize_downloads(0, [(100, 100), (100, 700)]),
            Some(DownloadProgress {
                active_count: 2,
                permille: Some(250),
            })
        );
        assert_eq!(
            summarize_downloads(0, [(u64::MAX, 100)]).unwrap().permille,
            Some(1000)
        );
    }

    #[test]
    fn reports_indeterminate_progress_when_any_download_has_no_total() {
        for (partials, jobs) in [(1, vec![(1, 2)]), (0, vec![(1, u64::MAX), (0, 0)])] {
            assert_eq!(
                summarize_downloads(partials, jobs),
                Some(DownloadProgress {
                    active_count: 2,
                    permille: None,
                })
            );
        }
        assert_eq!(summarize_downloads(0, []), None);
    }

    #[test]
    fn counts_recent_browser_partial_files_but_ignores_stale_files_and_directories() {
        use std::fs::{self, File, FileTimes};
        use std::time::{Duration, SystemTime};

        let now = SystemTime::now();
        let folder = std::env::temp_dir().join(format!(
            "cimori-downloads-test-{}-{}",
            std::process::id(),
            now.duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&folder).unwrap();
        for name in ["one.CRDOWNLOAD", "two.part", "three.download", "saved.zip"] {
            File::create(folder.join(name)).unwrap();
        }
        File::create(folder.join("stale.crdownload"))
            .unwrap()
            .set_times(FileTimes::new().set_modified(now - Duration::from_secs(3600)))
            .unwrap();
        fs::create_dir(folder.join("folder.part")).unwrap();

        let count = BrowserDownloads::default().scan(&folder, now).0;
        fs::remove_dir_all(&folder).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn growing_partial_files_refresh_activity_even_when_last_write_is_cached() {
        use std::fs::{self, File, FileTimes};
        use std::time::{Duration, SystemTime};

        let now = SystemTime::now();
        let folder = std::env::temp_dir().join(format!(
            "cimori-download-growth-test-{}-{}",
            std::process::id(),
            now.duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&folder).unwrap();
        let path = folder.join("in-flight.crdownload");
        let old = now - Duration::from_secs(3600);
        {
            let file = File::create(&path).unwrap();
            file.set_times(FileTimes::new().set_modified(old)).unwrap();
        }

        let mut browser = BrowserDownloads::default();
        assert_eq!(browser.scan(&folder, now).0, 0);
        {
            let file = File::options().write(true).open(&path).unwrap();
            file.set_len(1024).unwrap();
            file.set_times(FileTimes::new().set_modified(old)).unwrap();
        }
        assert_eq!(browser.scan(&folder, now).0, 1);
        assert_eq!(browser.scan(&folder, now + Duration::from_secs(121)).0, 0);
        fs::remove_dir_all(&folder).unwrap();
    }
}

#[cfg(windows)]
mod taskbar {
    use std::{cell::RefCell, os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle}, sync::{Arc, Mutex}, time::{Duration, Instant}};
    use windows::{core::{w, PCWSTR}, Win32::{Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM},
        System::{DataExchange::COPYDATASTRUCT, LibraryLoader::GetModuleHandleW, Threading::{CreateEventW, SetEvent}}, UI::WindowsAndMessaging::*}};
    type Progress = Arc<Mutex<Option<(Instant, u32, Option<u16>, usize)>>>;
    thread_local! { static VALUE: RefCell<Option<(Progress, Arc<OwnedHandle>)>> = const { RefCell::new(None) }; }
    pub struct Host { hwnd: isize, value: Progress, signal: Arc<OwnedHandle> }
    impl Host {
        // Port of mac-island's progress IPC; a separate host keeps the tray module independent.
        pub fn start() -> Option<Self> {
            let value = Progress::default();
            // Set on every packet, so the watcher sleeps while the taskbar says nothing.
            let signal = Arc::new(unsafe { OwnedHandle::from_raw_handle(CreateEventW(None, false, false, PCWSTR::null()).ok()?.0) });
            let shared = (value.clone(), signal.clone());
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            std::thread::spawn(move || unsafe {
                VALUE.with(|v| *v.borrow_mut() = Some(shared));
                let instance = GetModuleHandleW(None).unwrap_or_default();
                let class = w!("CimoriDownloadHost");
                RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(proc), hInstance: instance.into(), lpszClassName: class, ..Default::default() });
                let host = CreateWindowExW(WS_EX_TOOLWINDOW, class, w!("Cimori downloads"), WS_POPUP,
                    0, 0, 0, 0, None, None, instance, None);
                let Ok(host) = host else { let _ = tx.send(0); return; };
                if tx.send(host.0 as isize).is_err() { let _ = DestroyWindow(host); return; }
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() { let _ = TranslateMessage(&msg); DispatchMessageW(&msg); }
                VALUE.with(|v| *v.borrow_mut() = None);
            });
            let hwnd = rx.recv_timeout(Duration::from_secs(2)).ok()?;
            (hwnd != 0).then_some(Self { hwnd, value, signal })
        }
        pub fn signal(&self) -> HANDLE { HANDLE(self.signal.as_raw_handle()) }
        pub fn latest(&self) -> Option<(u32, Option<u16>, usize)> {
            self.value.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref()
                .filter(|(at, ..)| at.elapsed() < Duration::from_secs(5)).map(|(_, s, v, n)| (*s, *v, *n))
        }
    }
    impl Drop for Host { fn drop(&mut self) { unsafe { let _ = PostMessageW(HWND(self.hwnd as *mut _), WM_CLOSE, WPARAM(0), LPARAM(0)); } } }
    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if msg == WM_COPYDATA && lp.0 != 0 && FindWindowW(w!("Shell_TrayWnd"), None).is_ok_and(|h| h.0 as usize == wp.0) {
            let data = &*(lp.0 as *const COPYDATASTRUCT);
            if data.dwData == 0x43494D03 && data.cbData == 16 && !data.lpData.is_null() {
                if let Some((s, v, n)) = super::parse_progress(std::slice::from_raw_parts(data.lpData.cast(), 16)) {
                    VALUE.with(|slot| { if let Some((value, signal)) = slot.borrow().as_ref() {
                        *value.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((Instant::now(), s, v, n));
                        let _ = SetEvent(HANDLE(signal.as_raw_handle()));
                    }});
                    return LRESULT(1);
                }
            }
        }
        if msg == WM_DESTROY { PostQuitMessage(0); return LRESULT(0); }
        DefWindowProcW(hwnd, msg, wp, lp)
    }
}
