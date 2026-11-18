// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 zickrian

//! Windows port of Isle's `isle-daemon/src/share` at a29a7c4. The phone opens a
//! short-lived HTTP server on a single LAN interface and requests local approval.
//! A session serves the files opened from the Shelf when it starts.

mod http;
mod net;
mod upload;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use net::Lan;
use qrcode::{Color, EcLevel, QrCode};
use subtle::ConstantTimeEq;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;
use tokio::task::JoinSet;

const MAX_CONNECTIONS: usize = 12;
const MAX_ACTIVE_PEERS: usize = 3;
const MAX_PEERS: usize = 16;
const APPROVAL_TIMEOUT: Duration = Duration::from_secs(120);
const NETWORK_CHECK: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct ShareOptions {
    pub receive: Option<PathBuf>,
    pub capacity: usize,
    pub ports: (u16, u16),
}
impl Default for ShareOptions {
    fn default() -> Self { Self { receive: None, capacity: 6, ports: (47831, 47839) } }
}

/// The preferred private Ethernet or Wi-Fi address Windows uses for outbound traffic.
pub fn local_address() -> io::Result<Ipv4Addr> {
    net::local_address()
}

/// QR modules without the four-module quiet zone required by the renderer.
pub struct QrMatrix {
    width: usize,
    colors: Vec<Color>,
}

impl QrMatrix {
    pub fn width(&self) -> usize {
        self.width
    }

    pub fn dark(&self, x: usize, y: usize) -> bool {
        self.colors[y * self.width + x] == Color::Dark
    }
}

/// Match this code to the one displayed on the requesting phone.
#[derive(Clone)]
pub struct PendingPeer {
    pub code: String,
}

/// An active phone-sharing session. Stopping or dropping it closes the listener and transfers.
pub struct ShelfShare {
    url: String,
    qr: QrMatrix,
    session: Arc<Session>,
}

impl ShelfShare {
    /// Bind to one private IPv4 interface and take a snapshot of the selected shelf paths.
    pub fn start(
        address: Ipv4Addr,
        paths: Vec<PathBuf>,
        idle: Duration,
        on_change: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self> {
        if !address.is_private() {
            bail!("Conecte-se a uma rede local privada para compartilhar.");
        }
        Self::start_on(net::lan(address)?, paths, idle, on_change)
    }

    pub fn start_receiving(address: Ipv4Addr, paths: Vec<PathBuf>, idle: Duration,
        options: ShareOptions, on_change: impl Fn() + Send + Sync + 'static) -> Result<Self> {
        if !address.is_private() { bail!("Conecte-se a uma rede local privada para compartilhar."); }
        Self::start_with_options(net::lan(address)?, paths, idle, options, on_change)
    }

    fn start_on(
        lan: Lan,
        paths: Vec<PathBuf>,
        idle: Duration,
        on_change: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self> {
        Self::start_with_options(lan, paths, idle, ShareOptions::default(), on_change)
    }

    fn start_with_options(lan: Lan, paths: Vec<PathBuf>, idle: Duration, options: ShareOptions,
        on_change: impl Fn() + Send + Sync + 'static) -> Result<Self> {
        if idle.is_zero() {
            bail!("O tempo de compartilhamento deve ser maior que zero.");
        }
        let files: Vec<_> = paths
            .iter()
            .take(isle_core::shelf::CAPACITY)
            .filter_map(|path| SharedFile::open(path))
            .collect();
        if files.is_empty() && !paths.is_empty() {
            bail!("Nenhum arquivo da prateleira está disponível.");
        }
        let token = random_token()?;
        // Port of share/server.rs::bind: fixed range reserved for phone sharing;
        // never fall back to the agents' 47830 or an undocumented random port.
        let listener = (options.ports.0..=options.ports.1)
            .find_map(|port| std::net::TcpListener::bind((lan.address, port)).ok())
            .context("As portas 47831–47839 estão ocupadas. Encerre outro compartilhamento.")?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?.to_string();
        let url = format!("http://{address}/{token}/");
        let qr = QrCode::with_error_correction_level(url.as_bytes(), EcLevel::L)?;
        let qr = QrMatrix {
            width: qr.width(),
            colors: qr.into_colors(),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let listener = {
            let _entered = runtime.enter();
            TcpListener::from_std(listener)?
        };
        let session = Arc::new(Session {
            address,
            token,
            idle,
            on_change: Box::new(on_change),
            active: AtomicBool::new(true),
            changed: Notify::new(),
            state: Mutex::new(State {
                last_activity: Instant::now(),
                peers: HashMap::new(),
                received: Vec::new(), received_bytes: 0, reserved_bytes: 0, uploading: 0,
                landed: files.len(),
            }),
            options,
        });
        let serving = session.clone();
        std::thread::Builder::new()
            .name("shelf-share".into())
            .spawn(move || {
                runtime.block_on(serve(&serving, lan, listener, Arc::new(files)));
                serving.end();
                // A device can stall a blocking read. The sockets are already closed at this point.
                runtime.shutdown_background();
            })?;
        Ok(Self { url, qr, session })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn qr(&self) -> &QrMatrix {
        &self.qr
    }

    pub fn is_active(&self) -> bool {
        self.session.active.load(Ordering::Acquire)
    }

    pub fn pending(&self) -> Option<PendingPeer> {
        if !self.is_active() {
            return None;
        }
        let now = Instant::now();
        self.session
            .state()
            .peers
            .values()
            .filter(|peer| peer.status == Status::Pending && peer.expires > now)
            .min_by_key(|peer| peer.expires)
            .map(|peer| PendingPeer {
                code: peer.code.clone(),
            })
    }

    pub fn approve(&self, code: &str) -> bool {
        self.session.answer(code, Status::Allowed)
    }

    pub fn deny(&self, code: &str) -> bool {
        self.session.answer(code, Status::Denied)
    }

    pub fn stop(&self) {
        self.session.end();
    }

    pub fn drain_received(&self) -> Vec<PathBuf> { std::mem::take(&mut self.session.state().received) }

    // Port of isle share/server.rs::land: a whole upload lands even as sharing closes.
    pub fn stop_and_drain(&self) -> Vec<PathBuf> {
        self.stop();
        self.drain_received()
    }
}

impl Drop for ShelfShare {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Pending,
    Allowed,
    Denied,
}

struct Peer {
    secret: String,
    code: String,
    status: Status,
    expires: Instant,
}

struct State {
    last_activity: Instant,
    peers: HashMap<Ipv4Addr, Peer>,
    received: Vec<PathBuf>,
    received_bytes: u64,
    reserved_bytes: u64,
    uploading: usize,
    landed: usize,
}

struct Session {
    address: String,
    token: String,
    idle: Duration,
    on_change: Box<dyn Fn() + Send + Sync>,
    active: AtomicBool,
    changed: Notify,
    state: Mutex<State>,
    options: ShareOptions,
}

impl Session {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn end(&self) {
        let state = self.state();
        if self.active.swap(false, Ordering::AcqRel) {
            drop(state);
            self.changed.notify_one();
            (self.on_change)();
        }
    }

    fn touch(&self) {
        self.state().last_activity = Instant::now();
        self.changed.notify_one();
    }

    fn answer(&self, code: &str, answer: Status) -> bool {
        let mut state = self.state();
        let now = Instant::now();
        if !self.active.load(Ordering::Acquire)
            || now.duration_since(state.last_activity) >= self.idle
        {
            return false;
        }
        let Some(peer) = state
            .peers
            .values_mut()
            .find(|peer| peer.status == Status::Pending && peer.code == code && peer.expires > now)
        else {
            return false;
        };
        peer.status = answer;
        if answer == Status::Allowed {
            state.last_activity = now;
        }
        drop(state);
        self.changed.notify_one();
        (self.on_change)();
        true
    }

    fn peer(&self, ip: Ipv4Addr, cookie: Option<&str>, join: bool) -> Access {
        let mut state = self.state();
        let now = Instant::now();
        if !self.active.load(Ordering::Acquire)
            || now.duration_since(state.last_activity) >= self.idle
        {
            return Access::Denied;
        }
        if let Some(peer) = state.peers.get(&ip) {
            if !cookie
                .is_some_and(|value| bool::from(value.as_bytes().ct_eq(peer.secret.as_bytes())))
            {
                return Access::Denied;
            }
            return match peer.status {
                Status::Pending if peer.expires > now => Access::Pending(peer.code.clone(), None),
                Status::Allowed => Access::Allowed,
                _ => Access::Denied,
            };
        }
        if !join || cookie.is_some() {
            return Access::Denied;
        }
        if state.peers.len() >= MAX_PEERS
            || state
                .peers
                .values()
                .filter(|peer| {
                    peer.status == Status::Allowed
                        || (peer.status == Status::Pending && peer.expires > now)
                })
                .count()
                >= MAX_ACTIVE_PEERS
        {
            return Access::Busy;
        }
        let Ok(secret) = random_token() else {
            return Access::Busy;
        };
        let code = loop {
            let mut bytes = [0; 2];
            if getrandom::fill(&mut bytes).is_err() {
                return Access::Busy;
            }
            let code = format!("{:04}", u16::from_ne_bytes(bytes) % 10_000);
            if !state.peers.values().any(|peer| peer.code == code) {
                break code;
            }
        };
        state.peers.insert(
            ip,
            Peer {
                secret: secret.clone(),
                code: code.clone(),
                status: Status::Pending,
                expires: now + APPROVAL_TIMEOUT,
            },
        );
        drop(state);
        self.changed.notify_one();
        (self.on_change)();
        Access::Pending(code, Some(secret))
    }

    fn expire(&self) -> Duration {
        let mut state = self.state();
        let now = Instant::now();
        let mut remaining = self
            .idle
            .saturating_sub(now.duration_since(state.last_activity));
        // Port of Session::busy/idle: a live transfer keeps the session open.
        if state.uploading > 0 { remaining = self.idle; }
        let mut updated = false;
        for peer in state
            .peers
            .values_mut()
            .filter(|peer| peer.status == Status::Pending)
        {
            if peer.expires <= now {
                peer.status = Status::Denied;
                updated = true;
            } else {
                remaining = remaining.min(peer.expires - now);
            }
        }
        drop(state);
        if updated {
            (self.on_change)();
        }
        remaining
    }
}

enum Access {
    Pending(String, Option<String>),
    Allowed,
    Denied,
    Busy,
}

async fn serve(
    session: &Arc<Session>,
    lan: Lan,
    listener: TcpListener,
    files: Arc<Vec<SharedFile>>,
) {
    let mut connections = JoinSet::new();
    let mut network_check = tokio::time::interval(NETWORK_CHECK);
    network_check.tick().await;
    while session.active.load(Ordering::Acquire) {
        let remaining = session.expire();
        if remaining.is_zero() {
            break;
        }
        tokio::select! {
            _ = session.changed.notified() => {},
            _ = tokio::time::sleep(remaining) => {},
            _ = network_check.tick() => {
                if !lan.is_current() {
                    break;
                }
            },
            _ = connections.join_next(), if !connections.is_empty() => {},
            incoming = listener.accept(), if connections.len() < MAX_CONNECTIONS => {
                match incoming {
                    Ok((socket, SocketAddr::V4(peer))) if lan.contains(*peer.ip()) => {
                        connections.spawn(connection(session.clone(), files.clone(), socket, *peer.ip()));
                    },
                    Ok(_) => {},
                    Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
                }
            },
        }
    }
    drop(listener);
    connections.shutdown().await;
}

async fn connection(
    session: Arc<Session>,
    files: Arc<Vec<SharedFile>>,
    mut stream: TcpStream,
    peer: Ipv4Addr,
) {
    let Ok(head) = http::read_head(&mut stream).await else {
        return;
    };
    let prefix = session.token.as_bytes();
    let route = head
        .path
        .strip_prefix('/')
        .and_then(|path| path.split_once('/'));
    let Some((_, route)) = route.filter(|(token, _)| {
        head.host == session.address && bool::from(token.as_bytes().ct_eq(prefix))
    }) else {
        let _ = http::send_text(&mut stream, 404, "Não encontrado.").await;
        return;
    };
    let (route, query) = route.split_once('?').unwrap_or((route, ""));
    if head.method == "PUT" && route == "api/upload" {
        match session.peer(peer, head.cookie.as_deref(), false) {
            Access::Allowed if session.options.receive.is_some() => {
                let _ = upload::receive(&session, &mut stream, &head, query, peer).await;
            }
            _ => { let _ = http::send_text(&mut stream, 403, "Envio não autorizado.").await; }
        }
        http::drain_refused(&mut stream, head.length.unwrap_or(0).saturating_sub(head.body.len() as u64)).await;
        return;
    }
    if head.method == "POST" && route == "api/hello" {
        match session.peer(peer, head.cookie.as_deref(), true) {
            Access::Pending(code, cookie) => { let _ = http::send_pending(&mut stream, &session.token, &code, cookie.as_deref()).await; }
            Access::Allowed => { let _ = http::send_text(&mut stream, 200, "Autorizado").await; }
            _ => { let _ = http::send_text(&mut stream, 403, "Acesso negado").await; }
        }
        return;
    }
    if head.method == "POST" && matches!(route, "api/bye" | "api/picking") {
        let allowed = matches!(session.peer(peer, head.cookie.as_deref(), false), Access::Allowed);
        if allowed && route == "api/picking" { session.touch(); }
        let _ = http::send_text(&mut stream, if allowed { 200 } else { 403 }, "").await;
        return;
    }
    if head.method != "GET" || head.length.unwrap_or(0) != 0 || !head.body.is_empty() {
        let _ = http::send_text(&mut stream, 405, "Método indisponível.").await;
        return;
    }
    let _ = match session.peer(peer, head.cookie.as_deref(), route.is_empty()) {
        Access::Pending(code, cookie) if route.is_empty() => {
            http::send_pending(&mut stream, &session.token, &code, cookie.as_deref()).await
        }
        Access::Allowed if route.is_empty() => http::send_files(&mut stream, &files, session.options.receive.is_some()).await,
        Access::Allowed => {
            let item = route
                .strip_prefix("file/")
                .and_then(|slot| slot.parse::<usize>().ok())
                .and_then(|slot| files.get(slot));
            match item {
                Some(file) => http::send_download(&mut stream, &session, file).await,
                None => http::send_text(&mut stream, 404, "Não encontrado.").await,
            }
        }
        Access::Busy => http::send_text(&mut stream, 503, "Tente novamente em instantes.").await,
        _ => http::send_text(&mut stream, 403, "Acesso não autorizado na Cimori.").await,
    };
}

struct SharedFile {
    name: String,
    file: Arc<Mutex<File>>,
    size: u64,
}

impl SharedFile {
    fn open(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_string_lossy();
        let name: String = name.chars().filter(|c| !c.is_control()).collect();
        let file = File::open(path).ok()?;
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows::Win32::Foundation::HANDLE;
            use windows::Win32::Storage::FileSystem::{GetFileType, FILE_TYPE_DISK};

            // The handle is live for the call and must refer to a regular disk file.
            if unsafe { GetFileType(HANDLE(file.as_raw_handle())) } != FILE_TYPE_DISK {
                return None;
            }
        }
        let meta = file.metadata().ok()?;
        meta.is_file().then_some(Self {
            name,
            file: Arc::new(Mutex::new(file)),
            size: meta.len(),
        })
    }
}

/// A fresh 128-bit secret encoded in the URL-safe Base64 alphabet, without padding.
fn random_token() -> Result<String> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|err| anyhow!("Não foi possível criar o código: {err}"))?;
    let mut token = String::with_capacity(22);
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = chunk.get(1).copied().unwrap_or_default();
        let c = chunk.get(2).copied().unwrap_or_default();
        token.push(ALPHABET[(a >> 2) as usize] as char);
        token.push(ALPHABET[(((a & 3) << 4) | (b >> 4)) as usize] as char);
        if chunk.len() > 1 {
            token.push(ALPHABET[(((b & 15) << 2) | (c >> 6)) as usize] as char);
            token.push(ALPHABET[(c & 63) as usize] as char);
        }
    }
    Ok(token)
}
