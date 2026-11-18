use super::*;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};

fn request(share: &ShelfShare, method: &str, path: &str, host: &str, cookie: &str) -> String {
    let (_, address, _) = target(share);
    let mut socket = TcpStream::connect(address).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    write!(
        socket,
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nCookie: {cookie}\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).unwrap();
    response
}

fn target(share: &ShelfShare) -> (&str, SocketAddr, String) {
    let (host, path) = share
        .url()
        .strip_prefix("http://")
        .unwrap()
        .split_once('/')
        .unwrap();
    (host, host.parse().unwrap(), format!("/{path}"))
}

fn cookie(response: &str) -> &str {
    response
        .lines()
        .find_map(|line| line.strip_prefix("Set-Cookie: "))
        .unwrap()
        .split(';')
        .next()
        .unwrap()
}

fn loopback(
    paths: Vec<PathBuf>,
    idle: Duration,
    wake: impl Fn() + Send + Sync + 'static,
) -> ShelfShare {
    ShelfShare::start_on(Lan::loopback(), paths, idle, wake).unwrap()
}

#[test]
fn qr_session_exposes_only_shelved_files_after_local_approval() {
    let directory = std::env::temp_dir().join(format!(
        "cimori-share-test-{}-{}",
        std::process::id(),
        random_token().unwrap()
    ));
    fs::create_dir(&directory).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(directory.clone());
    let shared = directory.join("relatório & amigo.txt");
    fs::write(&shared, b"test content").unwrap();
    fs::write(directory.join("other.txt"), b"unshared").unwrap();

    let (wake, events) = std::sync::mpsc::channel();
    let share = loopback(vec![shared], Duration::from_secs(30), move || {
        let _ = wake.send(());
    });
    let (host, _, path) = target(&share);
    assert!(share.qr().width() >= 21);
    assert!(share.qr().dark(0, 0));
    assert!(request(&share, "GET", "/unknown/", host, "").starts_with("HTTP/1.1 404"));
    assert!(request(&share, "GET", &path, "unrelated.test", "").starts_with("HTTP/1.1 404"));
    assert!(share.pending().is_none());

    let landing = request(&share, "GET", &path, host, "");
    let cookie = cookie(&landing);
    assert!(landing.contains("HttpOnly; SameSite=Strict"));
    assert!(!landing.contains("relatório"));
    events.recv_timeout(Duration::from_secs(1)).unwrap();
    let pending = share.pending().unwrap();
    assert!(landing.contains(&pending.code));
    assert!(!share.approve("xyz"));
    assert!(
        request(&share, "GET", &format!("{path}file/0"), host, cookie).starts_with("HTTP/1.1 403")
    );
    assert!(share.approve(&pending.code));

    let page = request(&share, "GET", &path, host, cookie);
    assert!(page.contains("relatório &amp; amigo.txt"));
    assert!(!page.contains("other.txt"));
    let download = request(&share, "GET", &format!("{path}file/0"), host, cookie);
    assert!(download.starts_with("HTTP/1.1 200"));
    assert!(download.contains("Content-Disposition: attachment"));
    assert!(download.contains("relat%C3%B3rio%20%26%20amigo.txt"));
    assert!(download.ends_with("test content"));
    assert!(
        request(&share, "GET", &format!("{path}file/1"), host, cookie).starts_with("HTTP/1.1 404")
    );
    assert!(request(
        &share,
        "GET",
        &format!("{path}file/../other.txt"),
        host,
        cookie
    )
    .starts_with("HTTP/1.1 404"));
    assert!(request(&share, "POST", &path, host, cookie).starts_with("HTTP/1.1 405"));
}

#[test]
fn denial_blocks_the_phone_until_a_new_session() {
    let share = loopback(Vec::new(), Duration::from_secs(30), || {});
    let (host, _, path) = target(&share);
    let response = request(&share, "GET", &path, host, "");
    let pending = share.pending().unwrap();
    assert!(share.deny(&pending.code));
    assert!(!share.approve(&pending.code));
    assert!(request(&share, "GET", &path, host, cookie(&response)).starts_with("HTTP/1.1 403"));
    assert!(request(&share, "GET", &path, host, "").starts_with("HTTP/1.1 403"));
    share.stop();
    assert!(!share.is_active());
    let replacement = loopback(Vec::new(), Duration::from_secs(30), || {});
    let (replacement_host, _, replacement_path) = target(&replacement);
    assert_ne!(path, replacement_path);
    assert!(request(&replacement, "GET", &path, replacement_host, "").starts_with("HTTP/1.1 404"));
}

#[test]
fn unapproved_phone_does_not_extend_idle_deadline() {
    let share = loopback(Vec::new(), Duration::from_millis(300), || {});
    let (host, address, path) = target(&share);
    let response = request(&share, "GET", &path, host, "");
    std::thread::sleep(Duration::from_millis(150));
    assert!(request(&share, "GET", &path, host, cookie(&response)).starts_with("HTTP/1.1 200"));
    let until = Instant::now() + Duration::from_secs(1);
    while Instant::now() < until && TcpStream::connect(address).is_ok() {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!share.is_active());
    assert!(TcpStream::connect(address).is_err());
}

#[test]
fn public_wildcard_and_loopback_addresses_are_rejected_by_production_entry_point() {
    for address in [
        Ipv4Addr::UNSPECIFIED,
        Ipv4Addr::LOCALHOST,
        Ipv4Addr::new(203, 0, 113, 10),
    ] {
        assert!(ShelfShare::start(address, Vec::new(), Duration::from_secs(30), || {}).is_err());
    }
}

fn upload(share: &ShelfShare, cookie: &str, name: &str, data: &[u8]) -> String {
    let (host, address, path) = target(share);
    let mut socket = TcpStream::connect(address).unwrap();
    socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    write!(socket, "PUT {path}api/upload?name={name} HTTP/1.1\r\nHost: {host}\r\nCookie: {cookie}\r\nContent-Length: {}\r\n\r\n", data.len()).unwrap();
    socket.write_all(data).unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn approved_upload_lands_once_without_replacing_an_existing_file() {
    let dir = std::env::temp_dir().join(format!("cimori-upload-{}", random_token().unwrap()));
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("foto.txt"), b"original").unwrap();
    let share = ShelfShare::start_with_options(Lan::loopback(), Vec::new(), Duration::from_secs(30),
        ShareOptions { receive: Some(dir.clone()), ..Default::default() }, || {}).unwrap();
    let (host, _, path) = target(&share);
    let landing = request(&share, "POST", &format!("{path}api/hello"), host, "");
    let cookie = cookie(&landing);
    assert!(upload(&share, cookie, "foto.txt", b"new").starts_with("HTTP/1.1 403"));
    assert!(share.approve(&share.pending().unwrap().code));
    assert!(upload(&share, cookie, "..%2Ffoto.txt", b"new").starts_with("HTTP/1.1 201"));
    let received = share.stop_and_drain();
    assert!(!share.is_active());
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].file_name().unwrap(), "foto 2.txt");
    assert_eq!(fs::read(&received[0]).unwrap(), b"new");
    #[cfg(windows)]
    assert!(fs::read_to_string(format!("{}:Zone.Identifier", received[0].display())).unwrap().contains("ZoneId=3"));
    assert_eq!(fs::read(dir.join("foto.txt")).unwrap(), b"original");
    assert!(share.drain_received().is_empty());
    share.stop();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn uploads_reserve_bytes_and_shelf_places_until_they_end() {
    let share = loopback(Vec::new(), Duration::from_secs(30), || {});
    let first = share.session.reserve(4_000_000_000).unwrap();
    assert!(share.session.reserve(4_000_000_001).is_err());
    let second = share.session.reserve(4_000_000_000).unwrap();
    assert!(share.session.reserve(1).is_err());
    drop(first);
    drop(second);
    assert!(share.session.reserve(1).is_ok());
}

#[test]
fn an_interrupted_upload_removes_its_part_and_releases_its_slot() {
    let dir = std::env::temp_dir().join(format!("cimori-short-{}", random_token().unwrap()));
    let share = ShelfShare::start_with_options(Lan::loopback(), Vec::new(), Duration::from_secs(30),
        ShareOptions { receive: Some(dir.clone()), capacity: 1, ..Default::default() }, || {}).unwrap();
    let (host, address, path) = target(&share);
    let landing = request(&share, "GET", &path, host, "");
    let cookie = cookie(&landing);
    assert!(share.approve(&share.pending().unwrap().code));
    let mut socket = TcpStream::connect(address).unwrap();
    write!(socket, "PUT {path}api/upload?name=short.txt HTTP/1.1\r\nHost: {host}\r\nCookie: {cookie}\r\nContent-Length: 100\r\n\r\nshort").unwrap();
    socket.shutdown(std::net::Shutdown::Write).unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 400"));
    // Cleanup follows sending the response on the same worker; wait for its guard.
    let until = Instant::now() + Duration::from_secs(2);
    while fs::read_dir(&dir).unwrap().count() > 0 && Instant::now() < until { std::thread::yield_now(); }
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
    assert!(upload(&share, cookie, "complete.txt", b"ok").starts_with("HTTP/1.1 201"));
    share.stop();
    fs::remove_dir_all(dir).unwrap();
}
