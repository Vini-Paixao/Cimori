//! The island's HTTP side (`:47830`): AI hooks (`Cimori --island-hook`, on this machine or over
//! Tailscale/LAN) report events and ask permissions; `quota_sync` takes limits read elsewhere.
//!
//! - `POST /agents/decide`  a `Request`; answers "allow", "always", "deny", or "" (the AI asks in its terminal)
//! - `POST /agents/report`  an `Event`
//! - `POST /api/hook`       `{"event":"quota_sync", ...}`

use crate::agents::{ANSWER_WITHIN, Event, Limiter, Request};
use anyhow::Result;
use isle_core::state::{AiProviderKind, PermissionDecision};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};

/// Bodies are small JSON; anything bigger is refused.
const MAX_REQUEST: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum HookPayload {
    QuotaSync {
        provider: AiProviderKind,
        used_pct: f32,
        reset_at_unix: Option<u64>,
    },
}

pub enum HookEvent {
    /// A permission to show; the reply's receiver is dropped once nobody waits any more.
    Ask {
        request: Request,
        reply: oneshot::Sender<PermissionDecision>,
    },
    /// An AI needs the user ("permission", "waiting") or finished ("done").
    Agent(Event),
    QuotaSync {
        provider: AiProviderKind,
        used_pct: f32,
        reset_at_unix: Option<u64>,
    },
}

pub type HookEventChannel = mpsc::Receiver<HookEvent>;

/// The request line's path and the body, once whole; `None` for anything malformed or too big.
async fn read_http_request(stream: &mut TcpStream) -> Option<(String, String, Option<String>)> {
    let mut buf = Vec::with_capacity(4096);
    let mut temp = [0u8; 4096];
    loop {
        let n = stream.read(&mut temp).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&temp[..n]);
        if buf.len() > MAX_REQUEST {
            return None;
        }
        let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
            continue;
        };
        let head = String::from_utf8_lossy(&buf[..pos]);
        let line = head.lines().next()?.split_whitespace().collect::<Vec<_>>();
        let ["POST", path, "HTTP/1.1"] = line[..] else { return None; };
        let path = path.to_owned();
        let mut length = None;
        let mut bearer = None;
        for (key, value) in head.lines().skip(1).filter_map(|line| line.split_once(':')) {
            let value = value.trim();
            if key.eq_ignore_ascii_case("content-length") {
                if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) || length.replace(value.parse::<usize>().ok()?).is_some() { return None; }
            } else if key.eq_ignore_ascii_case("authorization") {
                if bearer.is_some() { return None; }
                let (scheme, token) = value.split_once(' ')?;
                if !scheme.eq_ignore_ascii_case("bearer") { return None; }
                bearer = Some(token.to_owned());
            } else if key.eq_ignore_ascii_case("transfer-encoding") { return None; }
        }
        let length = length.unwrap_or(0);
        let start = pos + 4;
        let end = start.checked_add(length).filter(|end| *end <= MAX_REQUEST)?;
        if buf.len() >= end {
            let body = String::from_utf8_lossy(&buf[start..start + length]).into_owned();
            return Some((path, body, bearer));
        }
    }
}

async fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes()).await;
}

fn answer_text(decision: PermissionDecision) -> &'static str {
    match decision {
        PermissionDecision::Allow => "allow",
        PermissionDecision::Always => "always",
        PermissionDecision::Deny => "deny",
    }
}

struct Shared {
    tx: mpsc::Sender<HookEvent>,
    dnd: Arc<AtomicBool>,
    limiter: Mutex<Limiter>,
    asked: AtomicU64,
    token: String,
}

/// Waits for the island's answer, or "" once the hook goes (answered in its terminal) or time is up.
async fn decide(shared: &Shared, stream: &mut TcpStream, request: Request) -> String {
    if shared.dnd.load(Ordering::Relaxed) {
        return String::new();
    }
    let mut request = request;
    let number = shared.asked.fetch_add(1, Ordering::Relaxed);
    request.id = format!("{}-{number}", request.provider);
    let (reply, answered) = oneshot::channel();
    if shared.tx.send(HookEvent::Ask { request, reply }).await.is_err() {
        return String::new();
    }
    let mut probe = [0u8; 1];
    tokio::select! {
        answer = answered => answer.map(answer_text).unwrap_or_default().to_owned(),
        // The hook sends nothing more: a read ending means it closed the connection.
        _ = stream.read(&mut probe) => String::new(),
        () = tokio::time::sleep(ANSWER_WITHIN) => String::new(),
    }
}

/// How long a connection may take to send its request before it has proved the token.
const PRE_AUTH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

async fn handle(shared: Arc<Shared>, mut stream: TcpStream, slots: Arc<tokio::sync::Semaphore>, pre_auth: tokio::sync::OwnedSemaphorePermit) {
    let Ok(Some((path, body, bearer))) = tokio::time::timeout(PRE_AUTH_TIMEOUT, read_http_request(&mut stream)).await else {
        let _ = stream.shutdown().await;
        return;
    };
    if !crate::auth::matches(&shared.token, bearer.as_deref()) {
        if bearer.is_none() { tracing::warn!("Hook antigo sem token recusado; execute Cimori --install-hooks novamente."); }
        else { tracing::warn!("Hook recusado: token inválido; confira o token compartilhado e reinstale os hooks."); }
        respond(&mut stream, "401 Unauthorized", "Token dos hooks obrigatório. Reinstale com --install-hooks.").await;
        let _ = stream.shutdown().await;
        return;
    }
    drop(pre_auth);
    // Only an authenticated request takes one of the working slots.
    let Ok(_slot) = slots.try_acquire_owned() else {
        respond(&mut stream, "503 Service Unavailable", "Ocupado.").await;
        let _ = stream.shutdown().await;
        return;
    };
    match path.as_str() {
        "/agents/decide" => {
            let request = serde_json::from_str::<Request>(&body)
                .map_err(|err| err.to_string())
                .and_then(Request::checked);
            match request {
                Ok(request) => {
                    let answer = decide(&shared, &mut stream, request).await;
                    respond(&mut stream, "200 OK", &answer).await;
                }
                Err(err) => respond(&mut stream, "400 Bad Request", &err).await,
            }
        }
        "/agents/report" => {
            let event = serde_json::from_str::<Event>(&body)
                .map_err(|err| err.to_string())
                .and_then(Event::checked);
            match event {
                Ok(event) => {
                    let allowed = shared
                        .limiter
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .allow(&event, Instant::now());
                    if allowed {
                        let _ = shared.tx.send(HookEvent::Agent(event)).await;
                    }
                    respond(&mut stream, "200 OK", "").await;
                }
                Err(err) => respond(&mut stream, "400 Bad Request", &err).await,
            }
        }
        "/api/hook" => match serde_json::from_str::<HookPayload>(&body) {
            Ok(HookPayload::QuotaSync {
                provider,
                used_pct,
                reset_at_unix,
            }) => {
                let _ = shared
                    .tx
                    .send(HookEvent::QuotaSync {
                        provider,
                        used_pct,
                        reset_at_unix,
                    })
                    .await;
                respond(&mut stream, "200 OK", "").await;
            }
            Err(err) => respond(&mut stream, "400 Bad Request", &err.to_string()).await,
        },
        _ => respond(&mut stream, "404 Not Found", "").await,
    }
}

pub struct HookServer {
    pub addr: SocketAddr,
    pub dnd_flag: Arc<AtomicBool>,
}

/// This machine (loopback) and Tailscale's addresses (100.64.0.0/10); anyone when not `local_only`.
pub fn peer_allowed(ip: std::net::IpAddr, local_only: bool) -> bool {
    if !local_only || ip.is_loopback() {
        return true;
    }
    match ip {
        std::net::IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            a == 100 && (64..128).contains(&b)
        }
        std::net::IpAddr::V6(v6) => v6.to_ipv4_mapped().is_some_and(|v4| peer_allowed(v4.into(), true))
            || v6.segments()[..3] == [0xfd7a, 0x115c, 0xa1e0],
    }
}

impl HookServer {
    #[cfg(test)]
    pub async fn spawn_for_test(dnd_active: bool) -> (Self, HookEventChannel) {
        Self::spawn_authenticated("127.0.0.1:0", dnd_active, true, TEST_TOKEN.into())
            .await
            .expect("bind ephemeral port")
    }

    /// `local_only`: connections from other machines than this one and Tailscale's are dropped.
    pub async fn spawn(bind_addr: &str, dnd_active: bool, local_only: bool) -> Result<(Self, HookEventChannel)> {
        Self::spawn_authenticated(bind_addr, dnd_active, local_only, crate::auth::configured_token()?).await
    }

    /// HTTP equivalent of the session-bus trust boundary in upstream agents/hook.rs.
    pub async fn spawn_authenticated(bind_addr: &str, dnd_active: bool, local_only: bool, token: String) -> Result<(Self, HookEventChannel)> {
        anyhow::ensure!(crate::auth::valid_token(&token), "Token dos hooks inválido.");
        let listener = TcpListener::bind(bind_addr).await?;
        let addr = listener.local_addr()?;
        let dnd_flag = Arc::new(AtomicBool::new(dnd_active));
        let (tx, rx) = mpsc::channel::<HookEvent>(64);
        let shared = Arc::new(Shared {
            tx,
            dnd: Arc::clone(&dnd_flag),
            limiter: Mutex::new(Limiter::default()),
            asked: AtomicU64::new(0),
            token,
        });

        tokio::spawn(async move {
            let slots = Arc::new(tokio::sync::Semaphore::new(32));
            // Unauthenticated connections are capped apart, so they can never crowd out the hooks.
            let pre_auth = Arc::new(tokio::sync::Semaphore::new(128));
            while let Ok((stream, peer)) = listener.accept().await {
                if !peer_allowed(peer.ip(), local_only) {
                    continue;
                }
                let Ok(guard) = pre_auth.clone().try_acquire_owned() else { continue; };
                let (shared, slots) = (Arc::clone(&shared), slots.clone());
                tokio::spawn(async move { handle(shared, stream, slots, guard).await; });
            }
        });

        Ok((Self { addr, dnd_flag }, rx))
    }
}

#[cfg(test)]
const TEST_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn unauthenticated_and_wrong_token_requests_never_reach_any_route() {
        let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let (server, mut rx) = HookServer::spawn_authenticated("127.0.0.1:0", false, true, token.into()).await.unwrap();
        for route in ["/agents/decide", "/agents/report", "/api/hook"] {
            for bearer in [None, Some("wrong")] {
                let client = reqwest::Client::new();
                let mut request = client.post(format!("http://{}{route}", server.addr)).body("{}");
                if let Some(token) = bearer { request = request.bearer_auth(token); }
                assert_eq!(request.send().await.unwrap().status(), 401);
                assert!(rx.try_recv().is_err());
            }
        }
        let response = reqwest::Client::new().post(format!("http://{}/api/hook", server.addr))
            .bearer_auth(token).body(r#"{"event":"quota_sync","provider":"ClaudeCode","used_pct":10,"reset_at_unix":null}"#).send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert!(matches!(rx.recv().await, Some(HookEvent::QuotaSync { .. })));
    }
    #[tokio::test]
    async fn silent_connections_cannot_starve_authenticated_hooks() {
        let (server, mut rx) = HookServer::spawn_authenticated("127.0.0.1:0", false, true, TEST_TOKEN.into()).await.unwrap();
        // More silent sockets than the 32 working slots: none has authenticated, none may hold a slot.
        let mut silent = Vec::new();
        for _ in 0..40 { silent.push(TcpStream::connect(server.addr).await.unwrap()); }
        let response = reqwest::Client::new().post(format!("http://{}/api/hook", server.addr))
            .bearer_auth(TEST_TOKEN).body(r#"{"event":"quota_sync","provider":"ClaudeCode","used_pct":10,"reset_at_unix":null}"#).send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert!(matches!(rx.recv().await, Some(HookEvent::QuotaSync { .. })));
        // A silent socket is closed explicitly once the pre-auth window ends.
        let mut buf = [0u8; 1];
        let n = tokio::time::timeout(Duration::from_secs(4), silent[0].read(&mut buf)).await.unwrap().unwrap();
        assert_eq!(n, 0);
    }
    #[test]
    fn only_this_machine_and_tailscale_when_local_only() {
        let ip = |s: &str| s.parse::<std::net::IpAddr>().unwrap();
        assert!(peer_allowed(ip("127.0.0.1"), true));
        assert!(peer_allowed(ip("::1"), true));
        assert!(peer_allowed(ip("100.64.0.1"), true));
        assert!(peer_allowed(ip("fd7a:115c:a1e0::1"), true));
        assert!(!peer_allowed(ip("fd7a:115c:a1e1::1"), true));
        assert!(!peer_allowed(ip("100.128.0.1"), true), "past 100.64.0.0/10");
        assert!(!peer_allowed(ip("192.168.0.5"), true));
        assert!(peer_allowed(ip("192.168.0.5"), false), "off: anyone, as before");
    }
    use std::time::Duration;

    fn ask_json(input: &str) -> String {
        serde_json::to_string(&Request {
            provider: "claude".into(),
            cwd: "/mnt/dados/projetos/isle-win".into(),
            tool: "Bash".into(),
            input: input.into(),
            always: true,
            ..Request::default()
        })
        .unwrap()
    }

    async fn post(addr: SocketAddr, path: &str, body: String) -> (u16, String) {
        let res = reqwest::Client::new()
            .post(format!("http://{addr}{path}"))
            .bearer_auth(TEST_TOKEN)
            .body(body)
            .send()
            .await
            .unwrap();
        (res.status().as_u16(), res.text().await.unwrap())
    }

    #[tokio::test]
    async fn an_ask_is_shown_and_answered_from_the_island() {
        let (server, mut rx) = HookServer::spawn_for_test(false).await;
        tokio::spawn(async move {
            if let Some(HookEvent::Ask { request, reply }) = rx.recv().await {
                assert_eq!(request.project, "isle-win");
                assert_eq!(request.input, "cargo test --workspace");
                assert!(request.id.starts_with("claude-"));
                let _ = reply.send(PermissionDecision::Always);
            }
        });
        let res = post(server.addr, "/agents/decide", ask_json("cargo test --workspace")).await;
        assert_eq!(res, (200, "always".to_string()));
    }

    #[tokio::test]
    async fn dnd_answers_nothing_at_once() {
        let (server, mut rx) = HookServer::spawn_for_test(true).await;
        let started = Instant::now();
        let res = post(server.addr, "/agents/decide", ask_json("ls")).await;
        assert_eq!(res, (200, String::new()));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_hidden_command_is_refused_before_it_reaches_the_island() {
        let (server, mut rx) = HookServer::spawn_for_test(false).await;
        let (status, _) = post(server.addr, "/agents/decide", ask_json("ls\nrm -rf ~")).await;
        assert_eq!(status, 400);
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn the_hook_leaving_withdraws_the_ask() {
        let (server, mut rx) = HookServer::spawn_for_test(false).await;
        let body = ask_json("npm test");
        let mut stream = TcpStream::connect(server.addr).await.unwrap();
        let head = format!(
            "POST /agents/decide HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer {TEST_TOKEN}\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).await.unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        let Some(HookEvent::Ask { mut reply, .. }) = rx.recv().await else {
            panic!("expected an ask");
        };
        assert!(!reply.is_closed());
        drop(stream); // answered in the terminal: Claude ends its hook
        tokio::time::timeout(Duration::from_secs(2), reply.closed())
            .await
            .expect("the island must let the ask go");
    }

    #[tokio::test]
    async fn reports_are_checked_and_passed_on() {
        let (server, mut rx) = HookServer::spawn_for_test(false).await;
        let body = r#"{"provider":"claude","kind":"done","cwd":"C:\\Users\\m\\site","message":"Fixed it.\nMore"}"#;
        assert_eq!(post(server.addr, "/agents/report", body.into()).await.0, 200);
        let Some(HookEvent::Agent(event)) = rx.recv().await else {
            panic!("expected an event");
        };
        assert_eq!((event.project.as_str(), event.message.as_str()), ("site", "Fixed it. More"));
        let bad = r#"{"provider":"someone","kind":"done"}"#;
        assert_eq!(post(server.addr, "/agents/report", bad.into()).await.0, 400);
    }

    #[tokio::test]
    async fn quota_sync_still_reaches_the_island() {
        let (server, mut rx) = HookServer::spawn_for_test(false).await;
        let body = serde_json::to_string(&HookPayload::QuotaSync {
            provider: AiProviderKind::ClaudeCode,
            used_pct: 81.0,
            reset_at_unix: Some(10),
        })
        .unwrap();
        assert_eq!(post(server.addr, "/api/hook", body).await.0, 200);
        assert!(matches!(rx.recv().await, Some(HookEvent::QuotaSync { used_pct, .. }) if used_pct == 81.0));
    }
}
