// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 zickrian

//! The read-only part of Isle's phone-sharing HTTP server, using one request per connection.

use std::fmt::Write as _;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::{Session, SharedFile};

const MAX_HEAD: usize = 8 * 1024;
const MAX_HEADERS: usize = 32;
const CHUNK: usize = 64 * 1024;
const FIRST_BYTE_WITHIN: Duration = Duration::from_secs(3);
const HEAD_WITHIN: Duration = Duration::from_secs(10);
const STEP_WITHIN: Duration = Duration::from_secs(15);

pub(super) struct Head {
    pub method: String,
    pub path: String,
    pub host: String,
    pub cookie: Option<String>,
    pub length: Option<u64>,
    pub body: Vec<u8>,
}

pub(super) async fn read_head(stream: &mut TcpStream) -> io::Result<Head> {
    let mut buffer = [0; MAX_HEAD];
    let mut count = tokio::time::timeout(FIRST_BYTE_WITHIN, stream.read(&mut buffer)).await??;
    tokio::time::timeout(HEAD_WITHIN, async {
        loop {
            let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
            let mut request = httparse::Request::new(&mut headers);
            if let httparse::Status::Complete(end) =
                request.parse(&buffer[..count]).map_err(|_| bad_head())?
            {
                // Every supported request has an empty body. Ignore no trailing request or bytes.
                if request.version != Some(1) {
                    return Err(bad_head());
                }
                let mut head = parse_head(&request)?;
                head.body = buffer[end..count].to_vec();
                if head.body.len() as u64 > head.length.unwrap_or(0) { return Err(bad_head()); }
                return Ok(head);
            }
            let n = stream.read(&mut buffer[count..]).await?;
            if n == 0 {
                return Err(bad_head());
            }
            count += n;
        }
    })
    .await?
}

fn parse_head(request: &httparse::Request<'_, '_>) -> io::Result<Head> {
    let mut host = None;
    let mut cookie = None;
    let mut length = None;
    for header in request.headers.iter() {
        if header.name.eq_ignore_ascii_case("Host") {
            if host
                .replace(std::str::from_utf8(header.value).map_err(|_| bad_head())?)
                .is_some()
            {
                return Err(bad_head());
            }
        } else if header.name.eq_ignore_ascii_case("Content-Length") {
            let value = std::str::from_utf8(header.value).map_err(|_| bad_head())?;
            if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) || length.replace(value.parse().map_err(|_| bad_head())?).is_some() { return Err(bad_head()); }
        } else if header.name.eq_ignore_ascii_case("Transfer-Encoding") {
            return Err(bad_head());
        } else if header.name.eq_ignore_ascii_case("Cookie") {
            let value = std::str::from_utf8(header.value).map_err(|_| bad_head())?;
            if let Some(secret) = value
                .split(';')
                .find_map(|pair| pair.trim().strip_prefix("cimori="))
            {
                if cookie.replace(secret.to_owned()).is_some() {
                    return Err(bad_head());
                }
            }
        }
    }
    Ok(Head {
        method: request.method.ok_or_else(bad_head)?.to_owned(),
        path: request.path.ok_or_else(bad_head)?.to_owned(),
        host: host.ok_or_else(bad_head)?.to_owned(),
        cookie,
        length, body: Vec::new(),
    })
}

fn bad_head() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid HTTP request head")
}

fn response_head(status: u16, content_type: &str, length: u64, headers: &str) -> String {
    let reason = match status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Content Too Large",
        507 => "Insufficient Storage",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Service Unavailable",
    };
    format!(
        "HTTP/1.1 {status} {reason}\r\nConnection: close\r\nContent-Type: {content_type}\r\n\
         Content-Length: {length}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\n\
         X-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\n\
         Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'\r\n\
         {headers}\r\n"
    )
}

async fn send_body(
    stream: &mut TcpStream,
    status: u16,
    kind: &str,
    headers: &str,
    body: &str,
) -> io::Result<()> {
    let mut response = response_head(status, kind, body.len() as u64, headers);
    response.push_str(body);
    tokio::time::timeout(STEP_WITHIN, stream.write_all(response.as_bytes())).await?
}

pub(super) async fn send_text(stream: &mut TcpStream, status: u16, text: &str) -> io::Result<()> {
    let headers = if status == 405 { "Allow: GET\r\n" } else { "" };
    send_body(stream, status, "text/plain; charset=utf-8", headers, text).await
}

// Port of isle share/server.rs::drain: let a refusal reach the phone even if it
// already sent body bytes (closing an unread TCP receive buffer causes a reset).
pub(super) async fn drain_refused(stream: &mut TcpStream, remaining: u64) {
    let _ = stream.shutdown().await;
    let _ = tokio::time::timeout(Duration::from_secs(1), async {
        let mut left = remaining.min(64 * 1024);
        let mut chunk = [0u8; 4096];
        while left > 0 {
            let want = chunk.len().min(left as usize);
            let n = stream.read(&mut chunk[..want]).await?;
            if n == 0 { break; }
            left -= n as u64;
        }
        Ok::<_, io::Error>(())
    }).await;
}

const HTML_START: &str = r#"<!doctype html><html lang="pt-BR"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Cimori · Prateleira</title><style>
body{font:1rem/1.5 system-ui,sans-serif;color:#f5f5f5;background:#18181b;margin:0}main{max-width:32rem;margin:0 auto;padding:2rem 1.25rem}h1{font-size:1.65rem;margin:0 0 1rem}a{display:block;color:inherit;background:#303036;border-radius:.65rem;padding:.85rem;margin:.55rem 0;overflow-wrap:anywhere}strong{font-size:2rem;letter-spacing:.18em}</style>"#;

pub(super) async fn send_pending(
    stream: &mut TcpStream,
    token: &str,
    code: &str,
    cookie: Option<&str>,
) -> io::Result<()> {
    let headers = cookie.map_or_else(String::new, |value| {
        format!("Set-Cookie: cimori={value}; Path=/{token}/; HttpOnly; SameSite=Strict\r\n")
    });
    let page = format!(
        "{HTML_START}<meta http-equiv=\"refresh\" content=\"2\"></head><body><main>\
         <h1>Autorizar este celular</h1><strong>{code}</strong>\
         <p>Confira o código na Cimori e toque em Permitir.</p></main></body></html>"
    );
    send_body(stream, 200, "text/html; charset=utf-8", &headers, &page).await
}

pub(super) async fn send_files(stream: &mut TcpStream, files: &[SharedFile], receive: bool) -> io::Result<()> {
    let mut page = format!("{HTML_START}</head><body><main><h1>Arquivos da Cimori</h1>");
    if files.is_empty() {
        page.push_str("<p>A prateleira está vazia.</p>");
    }
    for (index, file) in files.iter().enumerate() {
        let name = html_text(&file.name);
        let _ = write!(page, "<a href=\"file/{index}\" download>{name}</a>");
    }
    if receive { page.push_str(include_str!("upload.html")); }
    page.push_str("</main></body></html>");
    send_body(stream, 200, "text/html; charset=utf-8", "", &page).await
}

pub(super) async fn send_download(
    stream: &mut TcpStream,
    session: &Session,
    item: &SharedFile,
) -> io::Result<()> {
    let name = percent_encode(&item.name);
    let headers = format!("Content-Disposition: attachment; filename*=UTF-8''{name}\r\n");
    let response = response_head(200, "application/octet-stream", item.size, &headers);
    tokio::time::timeout(STEP_WITHIN, stream.write_all(response.as_bytes())).await??;

    let mut offset = 0;
    let started = Instant::now();
    while offset < item.size && session.active.load(Ordering::Acquire) {
        let size = (item.size - offset).min(CHUNK as u64) as usize;
        let file = item.file.clone();
        let chunk = tokio::task::spawn_blocking(move || {
            let mut file = file
                .lock()
                .map_err(|_| io::Error::other("file reader stopped"))?;
            file.seek(SeekFrom::Start(offset))?;
            let mut chunk = vec![0; size];
            file.read_exact(&mut chunk)?;
            Ok::<_, io::Error>(chunk)
        });
        let chunk = tokio::time::timeout(STEP_WITHIN, chunk).await???;
        if !session.active.load(Ordering::Acquire) {
            break;
        }
        tokio::time::timeout(STEP_WITHIN, stream.write_all(&chunk)).await??;
        offset += chunk.len() as u64;
        let seconds = started.elapsed().as_secs();
        if seconds >= 30 && offset / seconds < 16 * 1024 {
            return Err(io::ErrorKind::TimedOut.into());
        }
        session.touch();
    }
    if item.size == 0 {
        session.touch();
    }
    Ok(())
}

fn percent_encode(text: &str) -> String {
    let mut encoded = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

fn html_text(text: &str) -> String {
    let mut escaped = String::new();
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(character),
        }
    }
    escaped
}
