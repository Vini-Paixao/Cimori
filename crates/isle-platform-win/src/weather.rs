// The weather itself is read in isle_core::weather; this only fetches text (the forecast, the
// agenda's .ics, the holidays) over HTTPS.

pub async fn fetch_url_text(url: &str) -> anyhow::Result<String> {
    Ok(reqwest::get(url).await?.error_for_status()?.text().await?)
}

/// Text from GitHub's API, which wants a User-Agent; 15 s at most.
pub async fn fetch_github(url: &str, agent: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::builder().user_agent(agent).timeout(std::time::Duration::from_secs(15)).build()?;
    Ok(client.get(url).header("Accept", "application/vnd.github+json").send().await?.error_for_status()?.text().await?)
}

/// Like `fetch_github`, but None when GitHub has nothing there (404: no release published yet).
pub async fn fetch_github_opt(url: &str, agent: &str) -> anyhow::Result<Option<String>> {
    let client = reqwest::Client::builder().user_agent(agent).timeout(std::time::Duration::from_secs(15)).build()?;
    let reply = client.get(url).header("Accept", "application/vnd.github+json").send().await?;
    if reply.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    Ok(Some(reply.error_for_status()?.text().await?))
}

/// A file's bytes (a release's installer: redirects followed), 5 minutes at most.
pub async fn download(url: &str, agent: &str) -> anyhow::Result<Vec<u8>> {
    let client = reqwest::Client::builder().user_agent(agent).timeout(std::time::Duration::from_secs(300)).build()?;
    Ok(client.get(url).send().await?.error_for_status()?.bytes().await?.to_vec())
}
