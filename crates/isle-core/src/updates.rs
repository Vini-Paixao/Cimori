//! A newer Cimori: GitHub's latest release, looked up at start and once a day (port of isle's
//! isle-daemon/src/updates.rs). Nothing is sent but the request itself; nothing is installed.

pub const LATEST: &str = "https://api.github.com/repos/Vini-Paixao/Cimori/releases/latest";
/// Release pages the "Baixar" button may open (the answer's `html_url` must start with it).
pub const RELEASES: &str = "https://github.com/Vini-Paixao/Cimori/releases";
pub const EVERY_SECS: u64 = 24 * 3600;

/// The latest release's version (without the "v") and its page, from GitHub's answer.
pub fn parse_latest(json: &str) -> Option<(String, String)> {
    let release: serde_json::Value = serde_json::from_str(json).ok()?;
    let version = release["tag_name"].as_str()?.trim().trim_start_matches('v').to_owned();
    let page = release["html_url"].as_str().filter(|url| url.starts_with(RELEASES)).unwrap_or(RELEASES).to_owned();
    (!version.is_empty()).then_some((version, page))
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
    fn reads_the_tag_and_the_page() {
        let json = r#"{"tag_name":"v1.2.0","html_url":"https://github.com/Vini-Paixao/Cimori/releases/tag/v1.2.0","assets":[]}"#;
        assert_eq!(parse_latest(json), Some(("1.2.0".into(), "https://github.com/Vini-Paixao/Cimori/releases/tag/v1.2.0".into())));
        // A page elsewhere is not opened: the list of releases instead.
        let odd = r#"{"tag_name":"1.3.0","html_url":"https://example.com/x"}"#;
        assert_eq!(parse_latest(odd), Some(("1.3.0".into(), RELEASES.into())));
        assert_eq!(parse_latest(r#"{"message":"Not Found"}"#), None);
        assert_eq!(parse_latest("<html>"), None);
    }
}
