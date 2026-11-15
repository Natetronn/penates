use anyhow::{bail, Context, Result};
use image::{imageops, RgbaImage};
use std::path::{Path, PathBuf};
use url::Url;

pub(crate) fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) penates")
        .build()
}

fn get_bytes(url: &str) -> Result<Vec<u8>> {
    use std::io::Read;
    let resp = agent().get(url).call()?;
    let mut buf = vec![];
    resp.into_reader().take(10 * 1024 * 1024).read_to_end(&mut buf)?;
    Ok(buf)
}

/// Biggest dimension in a `sizes` string like "192x192 512x512"; 0 if absent or "any".
fn max_size(sizes: &str) -> i64 {
    sizes.split_whitespace().filter_map(|t| t.split(['x', 'X']).next()?.parse::<i64>().ok()).max().unwrap_or(0)
}

/// Icons listed in a PWA web manifest, scored (bigger is better; maskable-only icons carry
/// extra padding so they rank lower).
pub fn parse_manifest_icons(manifest_url: &Url, json: &str) -> Vec<(i64, String)> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return vec![] };
    let Some(icons) = v.get("icons").and_then(|i| i.as_array()) else { return vec![] };
    icons
        .iter()
        .filter_map(|i| {
            let src = manifest_url.join(i.get("src")?.as_str()?).ok()?;
            let size = max_size(i.get("sizes").and_then(|s| s.as_str()).unwrap_or(""));
            let purpose = i.get("purpose").and_then(|p| p.as_str()).unwrap_or("any");
            let pad = if purpose.trim() == "maskable" { 300 } else { 0 };
            Some((size + 1500 - pad, src.to_string()))
        })
        .collect()
}

/// `short_name` or `name` from a PWA manifest.
pub fn parse_manifest_name(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    ["short_name", "name"]
        .iter()
        .filter_map(|k| v.get(*k)?.as_str())
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .find(|s| !s.is_empty() && s.chars().count() <= 40)
}

/// Fetch the page's web manifest (if it links one): (manifest URL, JSON text).
pub(crate) fn fetch_manifest(html: &str, base: &Url) -> Option<(Url, String)> {
    let doc = scraper::Html::parse_document(html);
    let sel = scraper::Selector::parse("link[rel]").ok()?;
    let href = doc
        .select(&sel)
        .find(|e| e.value().attr("rel").is_some_and(|r| r.split_whitespace().any(|t| t.eq_ignore_ascii_case("manifest"))))?
        .value()
        .attr("href")?;
    let url = base.join(href).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let body = agent().get(url.as_str()).call().ok()?.into_string().ok()?;
    Some((url, body))
}

/// Icon URLs for a page, best first: PWA manifest icons, apple-touch icons, `<link rel=icon>`
/// by size, then `/favicon.ico` as a last resort.
pub fn candidates(page_url: &str) -> Vec<String> {
    let Ok(base) = Url::parse(page_url) else { return vec![] };
    let mut found: Vec<(i64, String)> = vec![];
    if let Some(html) = agent().get(page_url).call().ok().and_then(|r| r.into_string().ok()) {
        let doc = scraper::Html::parse_document(&html);
        let sel = scraper::Selector::parse("link[rel]").unwrap();
        for el in doc.select(&sel) {
            let rel = el.value().attr("rel").unwrap_or("").to_lowercase();
            if !rel.contains("icon") {
                continue;
            }
            let Some(href) = el.value().attr("href").and_then(|h| base.join(h).ok()) else { continue };
            let size = max_size(el.value().attr("sizes").unwrap_or(""));
            let bonus = if rel.contains("apple-touch") { 1000 } else { 0 };
            found.push((size + bonus, href.to_string()));
        }
        if let Some((murl, body)) = fetch_manifest(&html, &base) {
            found.extend(parse_manifest_icons(&murl, &body));
        }
    }
    found.sort_by_key(|f| std::cmp::Reverse(f.0));
    let mut urls: Vec<String> = vec![];
    for (_, u) in found {
        if !urls.contains(&u) {
            urls.push(u);
        }
    }
    if let Ok(f) = base.join("/favicon.ico") {
        let f = f.to_string();
        if !urls.contains(&f) {
            urls.push(f);
        }
    }
    urls
}

/// Decode any supported image into a 256x256 transparent-padded square PNG.
fn normalize(bytes: &[u8]) -> Result<Vec<u8>> {
    let img = image::load_from_memory(bytes).context("unsupported image (use PNG, JPG, ICO, GIF or WebP)")?;
    anyhow::ensure!(img.width().min(img.height()) >= 16, "icon too small");
    let img = img.resize(256, 256, imageops::FilterType::Lanczos3).to_rgba8();
    let mut canvas = RgbaImage::new(256, 256);
    imageops::overlay(&mut canvas, &img, ((256 - img.width()) / 2) as i64, ((256 - img.height()) / 2) as i64);
    let mut out = vec![];
    image::DynamicImage::ImageRgba8(canvas).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)?;
    Ok(out)
}

/// Store normalized icon bytes as `<icons>/<id>.png` and return the path.
pub fn set_from_bytes(bytes: &[u8], id: &str) -> Result<PathBuf> {
    anyhow::ensure!(crate::store::valid_id(id), "invalid app id '{id}'");
    let png = normalize(bytes)?;
    std::fs::create_dir_all(crate::store::icon_dir())?;
    let dst = crate::store::icon_dir().join(format!("{id}.png"));
    std::fs::write(&dst, png)?;
    Ok(dst)
}

/// Download one icon URL (http/https only) and store it.
pub fn set_from_src(src: &str, id: &str) -> Result<PathBuf> {
    let u = Url::parse(src)?;
    anyhow::ensure!(matches!(u.scheme(), "http" | "https"), "only http(s) icons");
    set_from_bytes(&get_bytes(src)?, id)
}

/// Fetch the site's best icon (the default). Direct from the site only.
pub fn fetch_for(page_url: &str, id: &str) -> Result<PathBuf> {
    for c in candidates(page_url) {
        if let Ok(bytes) = get_bytes(&c) {
            if let Ok(p) = set_from_bytes(&bytes, id) {
                return Ok(p);
            }
        }
    }
    bail!("could not find a usable icon for {page_url}")
}

/// Icon from either an http(s) URL or a local file path.
pub fn import_source(src: &str, id: &str) -> Result<PathBuf> {
    if src.starts_with("http://") || src.starts_with("https://") {
        set_from_src(src, id)
    } else {
        import_file(Path::new(src), id)
    }
}

pub fn import_file(src: &Path, id: &str) -> Result<PathBuf> {
    set_from_bytes(&std::fs::read(src).with_context(|| format!("reading {}", src.display()))?, id)
}

pub struct Candidate {
    pub src: String,
    pub png: Vec<u8>,
}

/// Up to 8 usable icons from the site, normalized for preview.
pub fn preview_candidates(page_url: &str) -> Vec<Candidate> {
    candidates(page_url)
        .into_iter()
        .filter_map(|src| {
            let png = normalize(&get_bytes(&src).ok()?).ok()?;
            Some(Candidate { src, png })
        })
        .take(8)
        .collect()
}

/// Load a stored icon as raw RGBA scaled to `size` px, optionally converted to grayscale
/// (keeps alpha). Used for the system tray.
pub fn rgba_for_tray(path: &Path, grayscale: bool, size: u32) -> Result<(Vec<u8>, u32, u32)> {
    let img = image::open(path)?.to_rgba8();
    let mut img = imageops::resize(&img, size, size, imageops::FilterType::Lanczos3);
    if grayscale {
        for p in img.pixels_mut() {
            let l = (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32).round() as u8;
            p[0] = l;
            p[1] = l;
            p[2] = l;
        }
    }
    let (w, h) = img.dimensions();
    Ok((img.into_raw(), w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_icons_rank_by_size_and_skip_padding() {
        let base = Url::parse("https://app.example/manifest.json").unwrap();
        let json = r#"{"name":"My App","short_name":"MyApp","icons":[
            {"src":"/i/192.png","sizes":"192x192"},
            {"src":"/i/512.png","sizes":"512x512","purpose":"any"},
            {"src":"/i/512m.png","sizes":"512x512","purpose":"maskable"}]}"#;
        let mut v = parse_manifest_icons(&base, json);
        v.sort_by_key(|f| std::cmp::Reverse(f.0));
        assert_eq!(v[0].1, "https://app.example/i/512.png");
        assert_eq!(v[1].1, "https://app.example/i/512m.png");
        assert_eq!(v[2].1, "https://app.example/i/192.png");
        assert_eq!(parse_manifest_name(json).as_deref(), Some("MyApp"));
        assert_eq!(max_size("48x48 96x96"), 96);
        assert_eq!(max_size("any"), 0);
    }

    #[test]
    fn grayscale_has_equal_channels() {
        let path = std::env::temp_dir().join("penates-gray-test.png");
        let mut img = image::RgbaImage::new(32, 32);
        for p in img.pixels_mut() {
            *p = image::Rgba([200, 30, 90, 255]);
        }
        img.save(&path).unwrap();
        let (px, w, h) = super::rgba_for_tray(&path, true, 16).unwrap();
        assert_eq!((w, h), (16, 16));
        assert!(px.chunks(4).all(|c| c[0] == c[1] && c[1] == c[2] && c[3] == 255));
        let (col, _, _) = super::rgba_for_tray(&path, false, 16).unwrap();
        assert!(col[0] != col[1]);
    }
}
