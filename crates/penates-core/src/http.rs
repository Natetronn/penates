//! One HTTP client for everything Penates downloads (pages, icons, catalogs, release info), so
//! connections are reused and every request has the same timeout, user agent and size cap.

use anyhow::Result;
use std::io::Read;
use std::sync::LazyLock;

static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
    ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent(concat!("Mozilla/5.0 (X11; Linux x86_64) Penates/", env!("CARGO_PKG_VERSION")))
        .build()
});

/// Largest page, catalog or icon Penates reads.
pub const MAX_BYTES: u64 = 10 * 1024 * 1024;

/// A downloaded body and the address it finally came from (after redirects).
pub struct Fetched {
    pub final_url: String,
    pub body: Vec<u8>,
}

/// GET `url`, following redirects, reading at most `limit` bytes.
pub fn get(url: &str, limit: u64) -> Result<Fetched> {
    // ureq's errors already name the URL ("https://…: status code 404").
    let resp = AGENT.get(url).call()?;
    let final_url = resp.get_url().to_string();
    let mut body = vec![];
    resp.into_reader().take(limit + 1).read_to_end(&mut body)?;
    anyhow::ensure!(body.len() as u64 <= limit, "{url} is larger than {} MB", limit / (1024 * 1024));
    Ok(Fetched { final_url, body })
}

/// GET `url` as text (lossy UTF-8), capped at `MAX_BYTES`.
pub fn get_text(url: &str) -> Result<String> {
    Ok(String::from_utf8_lossy(&get(url, MAX_BYTES)?.body).into_owned())
}
