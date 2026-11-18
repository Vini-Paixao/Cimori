// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Adapted for isle-win from Mac Island & Dock by caliberda v3.1.0
// (https://github.com/ramensoftware/windhawk-mods, mac-island-dock, MediaThreadProc):
// GSMTC events, Windows' current session, timeline correction and transport commands in Rust.

use anyhow::Result;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaStatus {
    pub artwork: Option<std::sync::Arc<isle_core::artwork::Artwork>>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub app_id: String,
    pub playing: bool,
    pub position_ms: i64,
    pub duration_ms: i64,
    /// Unix milliseconds when position was sampled (already adjusted to this time).
    pub sampled_at_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaCommand {
    PlayPause,
    Next,
    Previous,
}

#[cfg(any(windows, test))]
#[derive(Clone, Copy)]
struct Timeline {
    start_100ns: i64,
    end_100ns: i64,
    position_100ns: i64,
    updated_at_ms: i64,
}

#[cfg(any(windows, test))]
fn timeline_ms(t: Timeline, playing: bool, sampled_at_ms: i64) -> (i64, i64) {
    let duration = t.end_100ns.saturating_sub(t.start_100ns).max(0) / 10_000;
    let mut position = t.position_100ns.saturating_sub(t.start_100ns).max(0) / 10_000;
    let elapsed = sampled_at_ms.saturating_sub(t.updated_at_ms);
    // Like the mod, ignore missing, future or implausibly old update times.
    if playing && (1..3_600_000).contains(&elapsed) {
        position = position.saturating_add(elapsed);
    }
    if duration > 0 {
        position = position.min(duration);
    }
    (position, duration)
}

#[cfg(any(windows, test))]
fn build_status(mut status: MediaStatus, timeline: Option<Timeline>) -> Option<MediaStatus> {
    if status.title.trim().is_empty() {
        return None;
    }
    if let Some(timeline) = timeline {
        (status.position_ms, status.duration_ms) =
            timeline_ms(timeline, status.playing, status.sampled_at_ms);
    }
    Some(status)
}

#[cfg(any(windows, test))]
fn pick_session<T>(current: Option<T>, sessions: impl IntoIterator<Item = T>) -> Option<T> {
    current.or_else(|| sessions.into_iter().next())
}

#[cfg(windows)]
pub use win::MediaWatch;

#[cfg(not(windows))]
pub struct MediaWatch;

/// Subscribes to media changes. The callback runs serially on a dedicated MTA thread.
/// Paused tracks remain visible; no session or an empty title is reported as `None`.
/// Dropping the watch removes subscriptions and waits for an in-flight callback.
pub fn watch(on_change: impl Fn(Option<MediaStatus>) + Send + Sync + 'static,
) -> Result<MediaWatch> {
    #[cfg(windows)]
    return win::watch(on_change);
    #[cfg(not(windows))]
    {
        let _ = on_change;
        Ok(MediaWatch)
    }
}

/// Sends a transport command to the system's current session; no session is a no-op.
pub fn send(cmd: MediaCommand) -> Result<()> {
    #[cfg(windows)]
    return win::send(cmd);
    #[cfg(not(windows))]
    {
        let _ = cmd;
        Ok(())
    }
}

#[cfg(windows)]
mod win {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    };
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use anyhow::{Context, anyhow, bail};
    use windows::Foundation::{
        AsyncStatus, EventRegistrationToken, IAsyncOperation, TypedEventHandler,
    };
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
    };
    use windows::Storage::Streams::DataReader;
    use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};

    use super::*;

    #[derive(Clone)]
    struct Notifier {
        tx: SyncSender<()>,
        epoch: Arc<AtomicU64>,
        metadata: Arc<AtomicU64>,
    }
    impl Notifier {
        fn try_send(&self, _: ()) -> Result<(), mpsc::TrySendError<()>> {
            self.epoch.fetch_add(1, Ordering::AcqRel);
            self.tx.try_send(())
        }
        fn metadata_changed(&self) {
            self.metadata.fetch_add(1, Ordering::AcqRel);
            let _ = self.try_send(());
        }
    }
    fn wait_until_deadline(
        status: impl Fn() -> windows::core::Result<AsyncStatus>,
        cancel: impl Fn(),
        stop: &AtomicBool,
        notify: &Notifier,
        epoch: u64, deadline:Instant,
    ) -> Result<()> {
        while status()? == AsyncStatus::Started {
            if stop.load(Ordering::Acquire)
                || notify.epoch.load(Ordering::Acquire) != epoch
                || Instant::now() >= deadline
            {
                cancel();
                bail!("media operation cancelled or timed out");
            }
            thread::sleep(Duration::from_millis(10));
        }
        if stop.load(Ordering::Acquire) || notify.epoch.load(Ordering::Acquire) != epoch {
            cancel();
            bail!("media snapshot superseded");
        }
        Ok(())
    }
    fn wait_until(status:impl Fn()->windows::core::Result<AsyncStatus>,cancel:impl Fn(),stop:&AtomicBool,notify:&Notifier,epoch:u64)->Result<()>{wait_until_deadline(status,cancel,stop,notify,epoch,Instant::now()+Duration::from_secs(2))}
    fn next_cover_retry(start:Instant,attempts:usize,now:Instant)->Option<Instant>{
        if attempts>=5{return None}
        [500,1500,3500,5500].into_iter().map(|ms|start+Duration::from_millis(ms)).find(|at|*at>now)
    }
    fn enrich_status(mut status:MediaStatus,publish_first:bool,publish:&dyn Fn(Option<MediaStatus>),load:impl FnOnce()->Option<Arc<isle_core::artwork::Artwork>>)->MediaStatus{
        if publish_first{publish(Some(status.clone()));}
        status.artwork=load();status
    }
    fn await_op<T: windows::core::RuntimeType + 'static>(
        op: IAsyncOperation<T>,
        stop: &AtomicBool,
        notify: &Notifier,
        epoch: u64,
    ) -> Result<T> {
        wait_until(
            || op.Status(),
            || {
                let _ = op.Cancel();
            },
            stop,
            notify,
            epoch,
        )?;
        Ok(op.GetResults()?)
    }
    struct CoverCache {
        key: String,
        image: Option<Arc<isle_core::artwork::Artwork>>,
        metadata: u64,
        changed: Instant,
        attempts: usize,
        retry: Option<Instant>,
    }
    impl Default for CoverCache {
        fn default() -> Self {
            Self {
                key: String::new(),
                image: None,
                metadata: u64::MAX,
                changed: Instant::now(),
                attempts: 0,
                retry: None,
            }
        }
    }
    impl CoverCache {
        fn select(&mut self, key: String) {
            if self.key != key {
                *self = Self {
                    key,
                    changed: Instant::now(),
                    ..Default::default()
                };
            }
        }
        fn update(
            &mut self,
            properties:&windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
            stop: &AtomicBool,
            notify: &Notifier,
            epoch: u64,
        ) {
            let metadata = notify.metadata.load(Ordering::Acquire);
            if self.metadata == metadata && self.retry.is_none_or(|at| Instant::now() < at) {
                return;
            }
            let is_retry=self.metadata==metadata;
            let deadline=if is_retry{(Instant::now()+Duration::from_secs(2)).min(self.changed+Duration::from_secs(6))}else{Instant::now()+Duration::from_secs(2)};
            self.metadata = metadata;
            let found = (|| -> Result<Option<Arc<isle_core::artwork::Artwork>>> {
                let Ok(thumbnail) = properties.Thumbnail() else {
                    return Ok(None);
                };
                let open=thumbnail.OpenReadAsync()?;
                wait_until_deadline(||open.Status(),||{let _=open.Cancel();},stop,notify,epoch,deadline)?;
                let stream=open.GetResults()?;
                let size = stream.Size()?;
                anyhow::ensure!(
                    isle_core::artwork::valid_encoded_size(size),
                    "invalid media artwork size"
                );
                let reader = DataReader::CreateDataReader(&stream)?;
                let operation = reader.LoadAsync(size as u32)?;
                wait_until_deadline(
                    || operation.Status(),
                    || {
                        let _ = operation.Cancel();
                    },
                    stop,
                    notify,
                    epoch, deadline,
                )?;
                anyhow::ensure!(
                    operation.GetResults()? == size as u32,
                    "incomplete media artwork"
                );
                let mut bytes = vec![0; size as usize];
                reader.ReadBytes(&mut bytes)?;
                let image = crate::artwork::decode(&bytes)?;
                Ok(Some(Arc::new(image)))
            })();
            if let Ok(image) = found {
                if image.is_some() {
                    self.image = image;
                }
            }
            self.attempts += 1;
            self.retry=next_cover_retry(self.changed,self.attempts,Instant::now());
        }
    }

    struct Apartment;

    #[cfg(test)]
    mod artwork_lifecycle_tests {
        use super::*;
        #[test]
        fn slow_first_read_chooses_a_later_retry_slot(){
            let start=Instant::now();
            assert_eq!(next_cover_retry(start,1,start+Duration::from_millis(600)),Some(start+Duration::from_millis(1500)));
            assert_eq!(next_cover_retry(start,1,start+Duration::from_millis(2100)),Some(start+Duration::from_millis(3500)));
            assert_eq!(next_cover_retry(start,5,start),None);
            assert_eq!(next_cover_retry(start,1,start+Duration::from_secs(6)),None);
        }
        #[test]
        fn metadata_is_published_before_a_slow_artwork_loader(){
            let order=std::cell::RefCell::new(Vec::new());
            let status=MediaStatus{title:"new song".into(),..Default::default()};
            let result=enrich_status(status,true,&|s|{assert!(s.unwrap().artwork.is_none());order.borrow_mut().push("metadata");},||{order.borrow_mut().push("artwork");None});
            assert_eq!(result.title,"new song");assert_eq!(*order.borrow(),["metadata","artwork"]);
        }
        #[test]
        fn changing_app_or_track_clears_artwork_and_retry_budget() {
            let mut c = CoverCache::default();
            c.select("app\0song\0artist\0album".into());
            c.image = Some(Arc::new(
                isle_core::artwork::Artwork::new(1, 1, vec![255; 4]).unwrap(),
            ));
            c.attempts = 4;
            c.retry = Some(Instant::now());
            c.select("app\0song\0artist\0album".into());
            assert!(c.image.is_some());
            c.select("other-app\0song\0artist\0album".into());
            assert!(c.image.is_none());
            assert_eq!(c.attempts, 0);
            assert!(c.retry.is_none());
        }
        #[test]
        fn superseded_or_stopped_reads_cancel_without_waiting() {
            let (tx, _rx) = mpsc::sync_channel(1);
            let notify = Notifier {
                tx,
                epoch: Arc::new(AtomicU64::new(1)),
                metadata: Arc::new(AtomicU64::new(0)),
            };
            let cancelled = AtomicBool::new(false);
            let stop = AtomicBool::new(false);
            assert!(
                wait_until(
                    || Ok(AsyncStatus::Started),
                    || { cancelled.store(true, Ordering::Relaxed) },
                    &stop,
                    &notify,
                    0
                )
                .is_err()
            );
            assert!(cancelled.load(Ordering::Relaxed));
            cancelled.store(false, Ordering::Relaxed);
            stop.store(true, Ordering::Relaxed);
            assert!(
                wait_until(
                    || Ok(AsyncStatus::Started),
                    || { cancelled.store(true, Ordering::Relaxed) },
                    &stop,
                    &notify,
                    1
                )
                .is_err()
            );
            assert!(cancelled.load(Ordering::Relaxed));
        }
    }

    impl Apartment {
        fn new() -> Result<Self> {
            // This guard is created and dropped on the same dedicated thread.
            unsafe {
                RoInitialize(RO_INIT_MULTITHREADED)?;
            }
            Ok(Self)
        }
    }

    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                RoUninitialize();
            }
        }
    }

    /// Tokens belong to their original source, including on partial registration failure.
    struct ManagerEvents {
        manager: Manager,
        current: Option<EventRegistrationToken>,
        sessions: Option<EventRegistrationToken>,
    }

    impl ManagerEvents {
        fn new(manager: Manager, changed: &Notifier) -> Result<Self> {
            let mut events = Self {
                manager,
                current: None,
                sessions: None,
            };
            let notify = changed.clone();
            events.current = Some(events.manager.CurrentSessionChanged(
                &TypedEventHandler::new(move |_, _| {
                    notify.metadata_changed();
                    Ok(())
                }),
            )?);
            let notify = changed.clone();
            events.sessions = Some(events.manager.SessionsChanged(&TypedEventHandler::new(
                move |_, _| {
                    let _ = notify.try_send(());
                    Ok(())
                },
            ))?);
            Ok(events)
        }
    }

    impl Drop for ManagerEvents {
        fn drop(&mut self) {
            if let Some(token) = self.current.take() {
                let _ = self.manager.RemoveCurrentSessionChanged(token);
            }
            if let Some(token) = self.sessions.take() {
                let _ = self.manager.RemoveSessionsChanged(token);
            }
        }
    }

    struct SessionEvents {
        session: Session,
        properties: Option<EventRegistrationToken>,
        playback: Option<EventRegistrationToken>,
        timeline: Option<EventRegistrationToken>,
    }

    impl SessionEvents {
        fn new(session: Session, changed: &Notifier) -> Result<Self> {
            let mut events = Self {
                session,
                properties: None,
                playback: None,
                timeline: None,
            };
            let notify = changed.clone();
            events.properties = Some(events.session.MediaPropertiesChanged(
                &TypedEventHandler::new(move |_, _| {
                    notify.metadata_changed();
                    Ok(())
                }),
            )?);
            let notify = changed.clone();
            events.playback = Some(events.session.PlaybackInfoChanged(&TypedEventHandler::new(
                move |_, _| {
                    let _ = notify.try_send(());
                    Ok(())
                },
            ))?);
            let notify = changed.clone();
            events.timeline = Some(events.session.TimelinePropertiesChanged(
                &TypedEventHandler::new(move |_, _| {
                    let _ = notify.try_send(());
                    Ok(())
                }),
            )?);
            Ok(events)
        }
    }

    impl Drop for SessionEvents {
        fn drop(&mut self) {
            if let Some(token) = self.properties.take() {
                let _ = self.session.RemoveMediaPropertiesChanged(token);
            }
            if let Some(token) = self.playback.take() {
                let _ = self.session.RemovePlaybackInfoChanged(token);
            }
            if let Some(token) = self.timeline.take() {
                let _ = self.session.RemoveTimelinePropertiesChanged(token);
            }
        }
    }

    fn current_session(manager: &Manager) -> Result<Option<Session>> {
        // windows 0.58's Type::from_abi maps a successful null to Error::empty (S_OK).
        let current = match manager.GetCurrentSession() {
            Ok(session) => Some(session),
            Err(err) if err.code() == windows::core::Error::empty().code() => None,
            Err(err) => return Err(err.into()),
        };
        if current.is_some() {
            return Ok(pick_session(current, []));
        }
        let sessions = manager.GetSessions()?;
        let first = if sessions.Size()? > 0 {
            Some(sessions.GetAt(0)?)
        } else {
            None
        };
        Ok(pick_session(current, first))
    }

    fn read_status(session: &Session,
        cover: &mut CoverCache,
        stop: &AtomicBool,
        notify: &Notifier,
        epoch: u64, publish:&dyn Fn(Option<MediaStatus>),
    ) -> Result<Option<MediaStatus>> {
        let properties = await_op(session.TryGetMediaPropertiesAsync()?, stop, notify, epoch)?;
        let title = properties.Title()?.to_string();
        let artist = properties.Artist()?.to_string();
        let album = properties.AlbumTitle()?.to_string();
        let app_id = session.SourceAppUserModelId()?.to_string();
        let key=format!("{app_id}\0{title}\0{artist}\0{album}");let track_changed=cover.key!=key;
        cover.select(key);
        let playback = session.GetPlaybackInfo()?;
        // Some apps omit timeline information while switching tracks.
        let timeline = session.GetTimelineProperties().ok().and_then(|t| {
            Some(Timeline {
                start_100ns: t.StartTime().ok()?.Duration,
                end_100ns: t.EndTime().ok()?.Duration,
                position_100ns: t.Position().ok()?.Duration,
                updated_at_ms: (t.LastUpdatedTime().ok()?.UniversalTime / 10_000)
                    .saturating_sub(11_644_473_600_000),
            })
        });
        let sampled_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(i64::MAX as u128) as i64;
        let status=build_status(
            MediaStatus {
                artwork: cover.image.clone(),
                title,
                artist,
                album,
                app_id,
                playing: playback.PlaybackStatus()? == PlaybackStatus::Playing,
                sampled_at_ms,
                ..MediaStatus::default()
            },
            timeline,
        );
        Ok(status.map(|status|enrich_status(status,track_changed,&|snapshot|{if !stop.load(Ordering::Acquire)&&notify.epoch.load(Ordering::Acquire)==epoch{publish(snapshot)}},||{cover.update(&properties,stop,notify,epoch);cover.image.clone()})))
    }

    fn refresh(
        manager: &Manager,
        active: &mut Option<SessionEvents>,
        changed: &Notifier,
        cover: &mut CoverCache,
        stop: &AtomicBool,
        epoch: u64, publish:&dyn Fn(Option<MediaStatus>),
    ) -> Result<Option<MediaStatus>> {
        let current = current_session(manager)?;
        if active.as_ref().map(|events| &events.session) != current.as_ref() {
            // Revoke the previous session even when subscribing to the new one fails.
            *active = None;
            if let Some(session) = current {
                *active = Some(SessionEvents::new(session, changed)?);
            }
        }
        active
            .as_ref()
            .map(|events| read_status(&events.session, cover, stop, changed, epoch,publish))
            .transpose()
            .map(Option::flatten)
    }

    pub struct MediaWatch {
        changed: Notifier,
        stopped: Arc<AtomicBool>,
        worker: Option<JoinHandle<()>>,
    }

    impl Drop for MediaWatch {
        fn drop(&mut self) {
            self.stopped.store(true, Ordering::Release);
            let _ = self.changed.try_send(());
            if let Some(worker) = self.worker.take() {
                // A callback may drop its own watch; that thread cleans up as it returns.
                if worker.thread().id() != thread::current().id() {
                    let _ = worker.join();
                }
            }
        }
    }

    pub fn watch(on_change: impl Fn(Option<MediaStatus>) + Send + Sync + 'static,
    ) -> Result<MediaWatch> {
        // Coalesce bursts without blocking a WinRT event dispatch thread.
        let (tx, changes) = mpsc::sync_channel(1);
        let changed = Notifier {
            tx,
            epoch: Arc::new(AtomicU64::new(0)),
            metadata: Arc::new(AtomicU64::new(0)),
        };
        let (ready, started) = mpsc::sync_channel(1);
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let notify = changed.clone();
        let worker = thread::Builder::new()
            .name("isle-media".into())
            .spawn(move || {
                let setup = (|| -> Result<_> {
                    let apartment = Apartment::new()?;
                    let manager = await_op(
                        Manager::RequestAsync()?,
                        &stop,
                        &notify,
                        notify.epoch.load(Ordering::Acquire),
                    )?;
                    let events = ManagerEvents::new(manager, &notify)?;
                    let active = current_session(&events.manager)?
                        .map(|session| SessionEvents::new(session, &notify))
                        .transpose()?;
                    Ok((apartment, events, active))
                })();
                let (_apartment, events, mut active) = match setup {
                    Ok(setup) => setup,
                    Err(err) => {
                        let _ = ready.send(Err(err));
                        return;
                    }
                };
                if ready.send(Ok(())).is_err() {
                    return;
                }
                // Only native events and a bounded, six-second artwork retry window.
                let mut cover = CoverCache::default();
                loop {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    let epoch = notify.epoch.load(Ordering::Acquire);
                    let status = match refresh(&events.manager, &mut active, &notify,
                        &mut cover,
                        &stop,
                        epoch, &on_change,
                    ) {
                        Ok(status) => status,
                        Err(err) => {
                            tracing::warn!("GSMTC media read failed: {err:#}");
                            None
                        }
                    };
                    if notify.epoch.load(Ordering::Acquire) != epoch {
                        continue;
                    }
                    if status.is_none() {
                        cover = CoverCache::default();
                    }
                    if !stop.load(Ordering::Acquire) {
                        on_change(status);
                    }
                    if let Some(at) = cover.retry {
                        match changes.recv_timeout(at.saturating_duration_since(Instant::now())) {
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            _ => {}
                        }
                    } else if changes.recv().is_err() {
                        break;
                    }
                }
            })
            .context("starting media watcher")?;
        let watch = MediaWatch {
            changed,
            stopped,
            worker: Some(worker),
        };
        started
            .recv()
            .context("media watcher stopped during initialization")??;
        Ok(watch)
    }

    pub fn send(cmd: MediaCommand) -> Result<()> {
        // A fresh MTA works even when called by an STA UI thread.
        thread::Builder::new()
            .name("isle-media-command".into())
            .spawn(move || -> Result<()> {
                let _apartment = Apartment::new()?;
                let manager = Manager::RequestAsync()?.get()?;
                let Some(session) = current_session(&manager)? else {
                    return Ok(());
                };
                let accepted = match cmd {
                    MediaCommand::PlayPause => session.TryTogglePlayPauseAsync()?.get()?,
                    MediaCommand::Next => session.TrySkipNextAsync()?.get()?,
                    MediaCommand::Previous => session.TrySkipPreviousAsync()?.get()?,
                };
                if !accepted {
                    bail!("media session rejected {cmd:?}");
                }
                Ok(())
            })?
            .join()
            .map_err(|_| anyhow!("media command thread panicked"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_current_session_wins_else_first_available() {
        assert_eq!(pick_session(Some(2), [1, 2]), Some(2));
        assert_eq!(pick_session(None, [1, 2]), Some(1));
        assert_eq!(pick_session::<i32>(None, []), None);
    }

    fn timeline() -> Timeline {
        Timeline {
            start_100ns: 50_000_000,
            end_100ns: 150_000_000,
            position_100ns: 70_000_000,
            updated_at_ms: 1_000,
        }
    }

    #[test]
    fn timeline_is_relative_to_start_and_extrapolated_only_while_playing() {
        assert_eq!(timeline_ms(timeline(), false, 4_000), (2_000, 10_000));
        assert_eq!(timeline_ms(timeline(), true, 4_000), (5_000, 10_000));
        assert_eq!(timeline_ms(timeline(), true, 20_000), (10_000, 10_000));
        assert_eq!(timeline_ms(timeline(), true, 500), (2_000, 10_000));
        assert_eq!(timeline_ms(timeline(), true, 3_601_000), (2_000, 10_000));
    }

    #[test]
    fn timeline_handles_invalid_ranges_and_extreme_ticks() {
        let mut t = timeline();
        t.position_100ns = 0;
        assert_eq!(timeline_ms(t, false, 1_000), (0, 10_000));
        t.end_100ns = 0;
        assert_eq!(timeline_ms(t, false, 1_000), (0, 0));
        t.start_100ns = i64::MIN;
        t.end_100ns = i64::MAX;
        t.position_100ns = i64::MAX;
        let (position, duration) = timeline_ms(t, false, 1_000);
        assert!(duration > 0);
        assert_eq!(position, duration);
    }

    #[test]
    fn empty_metadata_hides_the_session_but_a_paused_track_stays() {
        let mut status = MediaStatus::default();
        assert_eq!(build_status(status.clone(), None), None);
        status.title = "Song".into();
        status.artist = "Artist".into();
        status.album = "Album".into();
        status.app_id = "player.exe".into();
        status.sampled_at_ms = 4_000;
        let paused = build_status(status.clone(), Some(timeline())).unwrap();
        assert_eq!(paused.position_ms, 2_000);
        assert_eq!(paused.duration_ms, 10_000);
        assert!(!paused.playing);
        assert_eq!(paused.sampled_at_ms, 4_000);
        assert_eq!(paused.album, "Album");
        status.title = "  ".into();
        assert_eq!(build_status(status, None), None);
    }

    #[cfg(not(windows))]
    #[test]
    fn linux_stubs_need_no_runtime_or_media_service() {
        let _watch = watch(|_| panic!("no events on Linux")).unwrap();
        for command in [
            MediaCommand::PlayPause,
            MediaCommand::Next,
            MediaCommand::Previous,
        ] {
            send(command).unwrap();
        }
    }
}
