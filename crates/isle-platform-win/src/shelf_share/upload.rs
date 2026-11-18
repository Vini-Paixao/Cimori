// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 zickrian
//! Port of isle-daemon/src/share/{server,session,files}.rs at a29a7c4.

use super::*;
use std::io::Write;
use tokio::io::AsyncReadExt;

const MAX_FILE: u64 = 4_000_000_000;
const MAX_SESSION: u64 = 8_000_000_000;
const KEEP_FREE: u64 = 1_000_000_000;

pub(super) struct Reservation<'a> { session: &'a Session, bytes: u64 }
impl Session {
    // Port of Session::reserve (isle share/session.rs): promised bytes and shelf slots count.
    pub(super) fn reserve(&self, bytes: u64) -> Result<Reservation<'_>, u16> {
        let mut state = self.state();
        if bytes > MAX_FILE || state.received_bytes + state.reserved_bytes + bytes > MAX_SESSION { return Err(413); }
        if state.landed + state.uploading >= self.options.capacity { return Err(409); }
        if self.options.receive.as_deref().and_then(free_space).is_some_and(|free| free < state.reserved_bytes + bytes + KEEP_FREE) { return Err(507); }
        state.uploading += 1;
        state.reserved_bytes += bytes;
        Ok(Reservation { session: self, bytes })
    }
}
impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        let mut state = self.session.state();
        state.uploading -= 1;
        state.reserved_bytes -= self.bytes;
        state.last_activity = Instant::now();
    }
}

struct Part(PathBuf);
impl Drop for Part { fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); } }

// Port of share/files.rs::sanitize, extended for Windows' reserved names and characters.
fn sanitize(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = base.chars().filter(|c| !c.is_control() && !matches!(*c,
        '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\u{200b}' | '\u{200e}' | '\u{200f}' |
        '\u{2028}'..='\u{202e}' | '\u{2060}' | '\u{2066}'..='\u{2069}' | '\u{061c}' | '\u{feff}')).collect();
    let mut name = cleaned.trim().trim_matches('.').trim().to_owned();
    if name.is_empty() { return "Arquivo do celular".into(); }
    let stem = name.split('.').next().unwrap_or_default().to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes()[3].is_ascii_digit()) { name.insert(0, '_'); }
    if name.len() > 200 {
        let ext = name.rsplit_once('.').filter(|(_, ext)| ext.len() <= 16).map(|(_, ext)| format!(".{ext}")).unwrap_or_default();
        let mut cut = 200 - ext.len();
        while !name.is_char_boundary(cut) { cut -= 1; }
        name = format!("{}{}", name[..cut].trim_end_matches([' ', '.']), ext);
    }
    name
}

fn decode(text: &str) -> Option<String> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < text.len() {
        let b = text.as_bytes()[at];
        if b == b'%' { out.push(u8::from_str_radix(text.get(at + 1..at + 3)?, 16).ok()?); at += 3; }
        else { out.push(if b == b'+' { b' ' } else { b }); at += 1; }
    }
    String::from_utf8(out).ok()
}

// Port of share/server.rs::upload/receive/land: a private part is published only whole.
pub(super) async fn receive(session: &Arc<Session>, stream: &mut TcpStream, head: &http::Head, query: &str, peer: Ipv4Addr) -> io::Result<()> {
    let Some(length) = head.length else { return http::send_text(stream, 411, "Tamanho obrigatório.").await; };
    let dir = session.options.receive.as_ref().unwrap();
    if std::fs::create_dir_all(dir).is_err() { return http::send_text(stream, 507, "Não foi possível criar a pasta.").await; }
    let _reservation = match session.reserve(length) {
        Ok(hold) => hold,
        Err(status) => return http::send_text(stream, status, "Limite de arquivo, sessão, prateleira ou disco atingido.").await,
    };
    let name = sanitize(&query.split('&').find_map(|p| p.strip_prefix("name=").and_then(decode)).unwrap_or_default());
    let part = Part(dir.join(format!(".phone-{}.part", random_token().map_err(io::Error::other)?)));
    let file = std::fs::OpenOptions::new().write(true).create_new(true).open(&part.0)?;
    let file = Arc::new(Mutex::new(file));
    let started = Instant::now();
    let mut done = 0u64;
    let result: io::Result<()> = async {
        let mut chunk = head.body.clone();
        loop {
            if !session.active.load(Ordering::Acquire) { return Err(io::ErrorKind::Interrupted.into()); }
            if !chunk.is_empty() {
                let writer = file.clone();
                let count = chunk.len();
                tokio::task::spawn_blocking(move || writer.lock().unwrap_or_else(std::sync::PoisonError::into_inner).write_all(&chunk)).await??;
                done += count as u64;
                session.touch();
            }
            if done == length { break; }
            if started.elapsed() >= Duration::from_secs(30) && done / started.elapsed().as_secs().max(1) < 16 * 1024 { return Err(io::ErrorKind::TimedOut.into()); }
            if free_space(dir).is_some_and(|free| free < KEEP_FREE / 2) { return Err(io::Error::other("Disco cheio")); }
            chunk = vec![0; (length - done).min(64 * 1024) as usize];
            let n = tokio::time::timeout(Duration::from_secs(30), stream.read(&mut chunk)).await??;
            if n == 0 { return Err(io::ErrorKind::UnexpectedEof.into()); }
            chunk.truncate(n);
        }
        let writer = file.clone();
        tokio::task::spawn_blocking(move || writer.lock().unwrap_or_else(std::sync::PoisonError::into_inner).sync_all()).await??;
        Ok(())
    }.await;
    drop(file);
    if result.is_err() { return http::send_text(stream, 400, "Envio incompleto; tente novamente.").await; }
    // Mark of the Web, so SmartScreen warns when the phone's file is opened. A hard link shares the stream.
    if mark_of_the_web(&part.0, peer).is_err() { return http::send_text(stream, 507, "Não foi possível salvar o arquivo.").await; }
    let saved = match land(session, &part.0, &name, length) {
        Ok(path) => path,
        Err(status) => return http::send_text(stream, status, "Não foi possível salvar o arquivo.").await,
    };
    let name = saved.file_name().unwrap().to_string_lossy().into_owned();
    (session.on_change)();
    http::send_text(stream, 201, &serde_json::json!({"name": name}).to_string()).await
}

fn mark_of_the_web(file: &Path, peer: Ipv4Addr) -> io::Result<()> {
    let mut stream = file.as_os_str().to_owned();
    stream.push(":Zone.Identifier");
    std::fs::write(stream, format!("[ZoneTransfer]
ZoneId=3
HostUrl=http://{peer}/
"))
}

// Port of isle share/server.rs::land; serialized with stopping the session.
fn land(session: &Session, part: &Path, name: &str, length: u64) -> Result<PathBuf, u16> {
    let mut state = session.state();
    if !session.active.load(Ordering::Acquire) { return Err(403); }
    let dir = session.options.receive.as_ref().unwrap();
    let mut saved = None;
    for n in 1..=999 {
        let numbered = if n == 1 { name.to_owned() } else { match name.rsplit_once('.') {
            Some((stem, ext)) => format!("{stem} {n}.{ext}"), None => format!("{name} {n}"),
        }};
        let target = dir.join(numbered);
        // Windows hard_link has CREATE_NEW semantics: never overwrites an existing name.
        match std::fs::hard_link(part, &target) {
            Ok(()) => { saved = Some(target); break; }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(507),
        }
    }
    let Some(saved) = saved else { return Err(409); };
    state.received_bytes += length;
    state.landed += 1;
    state.received.push(saved.clone());
    Ok(saved)
}

#[cfg(windows)]
fn free_space(dir: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    let path: Vec<u16> = dir.as_os_str().encode_wide().chain([0]).collect();
    let mut free = 0;
    unsafe { windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(windows::core::PCWSTR(path.as_ptr()), Some(&mut free), None, None).ok()?; }
    Some(free)
}
#[cfg(not(windows))]
fn free_space(_: &Path) -> Option<u64> { None }

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn received_files_carry_the_zone_identifier() {
        let part = std::env::temp_dir().join(format!("cimori-motw-{}.part", std::process::id()));
        let link = part.with_extension("txt");
        std::fs::write(&part, b"x").unwrap();
        mark_of_the_web(&part, Ipv4Addr::new(192, 168, 0, 7)).unwrap();
        std::fs::hard_link(&part, &link).unwrap();
        let mut zone = link.as_os_str().to_owned();
        zone.push(":Zone.Identifier");
        let text = std::fs::read_to_string(zone).unwrap();
        assert!(text.contains("ZoneId=3") && text.contains("HostUrl=http://192.168.0.7/"));
        let _ = std::fs::remove_file(part);
        let _ = std::fs::remove_file(link);
    }
    #[test]
    fn phone_names_cannot_escape_or_spoof_windows_paths() {
        assert_eq!(sanitize("../../CON.txt"), "_CON.txt");
        assert_eq!(sanitize("a\\b:evil?.txt. "), "bevil.txt");
        assert_eq!(sanitize(".."), "Arquivo do celular");
        assert_eq!(sanitize("a\u{202e}cod.exe"), "acod.exe");
        assert!(sanitize(&"á".repeat(200)).len() <= 200);
    }
}
