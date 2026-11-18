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
