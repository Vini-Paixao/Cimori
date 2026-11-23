//! A newer Cimori: GitHub's latest release, looked up at start and once a day (port of isle's
//! isle-daemon/src/updates.rs). Nothing is sent but the request itself; it is installed only when
//! asked ("Baixar e instalar"), from the release's own installer checked against its SHA256SUMS.txt.

pub const LATEST: &str = "https://api.github.com/repos/Vini-Paixao/Cimori/releases/latest";
/// Release pages the "Baixar" button may open (the answer's `html_url` must start with it).
pub const RELEASES: &str = "https://github.com/Vini-Paixao/Cimori/releases";
pub const EVERY_SECS: u64 = 24 * 3600;
/// Where the release's files may be downloaded from (an asset's URL must start with it).
pub const DOWNLOADS: &str = "https://github.com/Vini-Paixao/Cimori/releases/download/";

/// The latest release, as GitHub describes it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Latest {
    /// Without the "v": "1.0.1".
    pub version: String,
    /// Its page (under `RELEASES`).
    pub page: String,
    /// The installer (`Cimori-<version>-setup.exe`): its name and URL (under `DOWNLOADS`).
    pub installer: Option<(String, String)>,
    /// `SHA256SUMS.txt`'s URL (under `DOWNLOADS`).
    pub sums: Option<String>,
}

/// The latest release from GitHub's answer.
pub fn parse_latest(json: &str) -> Option<Latest> {
    let release: serde_json::Value = serde_json::from_str(json).ok()?;
    let version = release["tag_name"].as_str()?.trim().trim_start_matches('v').to_owned();
    if version.is_empty() {
        return None;
    }
    let page = release["html_url"].as_str().filter(|url| url.starts_with(RELEASES)).unwrap_or(RELEASES).to_owned();
    let assets = release["assets"].as_array().map(Vec::as_slice).unwrap_or_default();
    let asset = |wanted: &dyn Fn(&str) -> bool| {
        assets.iter().find_map(|a| {
            let name = a["name"].as_str().filter(|n| wanted(n))?;
            let url = a["browser_download_url"].as_str().filter(|u| u.starts_with(DOWNLOADS))?;
            Some((name.to_owned(), url.to_owned()))
        })
    };
    let installer = asset(&|n| n.eq_ignore_ascii_case(&format!("Cimori-{version}-setup.exe")));
    let sums = asset(&|n| n.eq_ignore_ascii_case("SHA256SUMS.txt")).map(|(_, url)| url);
    Some(Latest { version, page, installer, sums })
}

/// The SHA-256 (lowercase hex) `SHA256SUMS.txt` gives for `file` ("<hash>  <name>" lines, `*name` too).
pub fn expected_sha256(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.trim().split_once(char::is_whitespace)?;
        let name = name.trim().trim_start_matches('*');
        (name.eq_ignore_ascii_case(file) && hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
    })
}

/// Whether version `a` ("1.2.0") is newer than `b`, by each number (isle's is_newer); a pre-release
/// suffix ("1.2.0-beta") is read up to its first non-digit.
pub fn is_newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.split('.').map(|p| p.chars().take_while(char::is_ascii_digit).collect::<String>().parse().unwrap_or(0)).collect()
    };
    !a.is_empty() && parts(a) > parts(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_by_number() {
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(is_newer("1.10.0", "1.9.3"));
        assert!(is_newer("2.0.0", "1.99.0"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("0.9.9", "1.0.0"));
        assert!(!is_newer("", "1.0.0"));
        assert!(is_newer("1.1.0-beta", "1.0.0"));
    }

    #[test]
    fn reads_the_tag_the_page_and_the_installer() {
        let json = r#"{"tag_name":"v1.2.0","html_url":"https://github.com/Vini-Paixao/Cimori/releases/tag/v1.2.0","assets":[
            {"name":"Cimori-1.2.0-win-x64.zip","browser_download_url":"https://github.com/Vini-Paixao/Cimori/releases/download/v1.2.0/Cimori-1.2.0-win-x64.zip"},
            {"name":"Cimori-1.2.0-setup.exe","browser_download_url":"https://github.com/Vini-Paixao/Cimori/releases/download/v1.2.0/Cimori-1.2.0-setup.exe"},
            {"name":"SHA256SUMS.txt","browser_download_url":"https://github.com/Vini-Paixao/Cimori/releases/download/v1.2.0/SHA256SUMS.txt"}]}"#;
        let latest = parse_latest(json).unwrap();
        assert_eq!(latest.version, "1.2.0");
        assert_eq!(latest.page, "https://github.com/Vini-Paixao/Cimori/releases/tag/v1.2.0");
        assert_eq!(latest.installer.unwrap().0, "Cimori-1.2.0-setup.exe");
        assert!(latest.sums.unwrap().ends_with("/SHA256SUMS.txt"));
        // A page or a file elsewhere is not used: the list of releases, and no installer.
        let odd = r#"{"tag_name":"1.3.0","html_url":"https://example.com/x","assets":[
            {"name":"Cimori-1.3.0-setup.exe","browser_download_url":"https://example.com/Cimori-1.3.0-setup.exe"}]}"#;
        let odd = parse_latest(odd).unwrap();
        assert_eq!((odd.page.as_str(), odd.installer), (RELEASES, None));
        assert_eq!(parse_latest(r#"{"message":"Not Found"}"#), None);
        assert_eq!(parse_latest("<html>"), None);
    }

    #[test]
    fn finds_the_installers_hash() {
        let hash = "ab".repeat(32);
        let sums = format!("{}  Cimori-1.2.0-win-x64.zip
{hash}  Cimori-1.2.0-setup.exe
", "cd".repeat(32));
        assert_eq!(expected_sha256(&sums, "Cimori-1.2.0-setup.exe"), Some(hash.clone()));
        assert_eq!(expected_sha256(&format!("{} *Cimori-1.2.0-setup.exe", hash.to_uppercase()), "Cimori-1.2.0-setup.exe"), Some(hash));
        assert_eq!(expected_sha256(&sums, "outro.exe"), None);
        assert_eq!(expected_sha256("xyz  Cimori-1.2.0-setup.exe", "Cimori-1.2.0-setup.exe"), None);
    }
}
