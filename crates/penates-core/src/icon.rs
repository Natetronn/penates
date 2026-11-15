//! App icons: found on the site, chosen from a file or URL, stored as our own 256 px PNG.

use crate::{http, site::Page, store};
use anyhow::{bail, Context, Result};
use image::{imageops, RgbaImage};
use std::path::{Path, PathBuf};
use url::Url;

/// Biggest dimension in a `sizes` string like "192x192 512x512"; 0 if absent or "any".
/// Clamped, so a hostile value cannot overflow the scores built from it.
pub(crate) fn max_size(sizes: &str) -> i64 {
    sizes
        .split_whitespace()
        .filter_map(|t| t.split(['x', 'X']).next()?.parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        .clamp(0, 10_000)
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

/// Largest icon file Penates accepts.
pub const MAX_ICON_BYTES: usize = 8 * 1024 * 1024;

fn download(src: &str) -> Result<Vec<u8>> {
    let u = Url::parse(src)?;
    anyhow::ensure!(matches!(u.scheme(), "http" | "https"), "only http(s) icons");
    Ok(http::get(src, MAX_ICON_BYTES as u64)?.body)
}

/// Decode any supported image into a 256x256 transparent-padded square PNG. Decoding is capped
/// (4096 px a side, 64 MB), so a hostile file cannot exhaust memory.
pub fn normalize(bytes: &[u8]) -> Result<Vec<u8>> {
    anyhow::ensure!(bytes.len() <= MAX_ICON_BYTES, "image is larger than 8 MB");
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let img = reader.decode().context("unsupported or oversized image (use PNG, JPG, ICO, GIF or WebP up to 4096 px)")?;
    anyhow::ensure!(img.width().min(img.height()) >= 16, "icon too small");
    let img = img.resize(256, 256, imageops::FilterType::Lanczos3).to_rgba8();
    let mut canvas = RgbaImage::new(256, 256);
    imageops::overlay(&mut canvas, &img, ((256 - img.width()) / 2) as i64, ((256 - img.height()) / 2) as i64);
    let mut out = vec![];
    image::DynamicImage::ImageRgba8(canvas).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)?;
    Ok(out)
}

/// Where an app's icon is stored.
pub fn path_for(id: &str) -> PathBuf {
    store::icon_dir().join(format!("{id}.png"))
}

/// Store a normalized PNG as `<icons>/<id>.png`. Returns the path and whether the file changed.
pub fn save_png(id: &str, png: &[u8]) -> Result<(PathBuf, bool)> {
    store::check_id(id)?;
    let dst = path_for(id);
    if std::fs::read(&dst).is_ok_and(|old| old == png) {
        return Ok((dst, false));
    }
    store::write_atomic(&dst, png)?;
    Ok((dst, true))
}

/// Download an icon URL and normalize it (not stored).
pub fn fetch_png(src: &str) -> Result<Vec<u8>> {
    normalize(&download(src)?)
}

/// Normalize and store icon bytes from any source.
pub fn set_from_bytes(bytes: &[u8], id: &str) -> Result<PathBuf> {
    Ok(save_png(id, &normalize(bytes)?)?.0)
}

/// Download one icon URL (http/https only) and store it.
pub fn set_from_src(src: &str, id: &str) -> Result<PathBuf> {
    set_from_bytes(&download(src)?, id)
}

/// Icon from either an http(s) URL or a local file path.
pub fn import_source(src: &str, id: &str) -> Result<PathBuf> {
    if src.starts_with("http://") || src.starts_with("https://") {
        set_from_src(src, id)
    } else {
        let bytes = std::fs::read(Path::new(src)).with_context(|| format!("reading {src}"))?;
        set_from_bytes(&bytes, id)
    }
}

/// The best usable icon on a page, normalized (not stored).
pub fn best_from(page: &Page) -> Result<Vec<u8>> {
    for c in page.icon_candidates() {
        if let Ok(png) = download(&c).and_then(|b| normalize(&b)) {
            return Ok(png);
        }
    }
    bail!("could not find a usable icon for {}", page.final_url)
}

/// Fetch the site's best icon and store it (the default icon).
pub fn fetch_for(page_url: &str, id: &str) -> Result<PathBuf> {
    Ok(save_png(id, &best_from(&Page::fetch(page_url)?)?)?.0)
}

pub struct Candidate {
    pub src: String,
    pub png: Vec<u8>,
}

/// Up to 8 usable icons from the site, normalized for preview. Downloads run in parallel.
pub fn preview_candidates(page_url: &str) -> Vec<Candidate> {
    let Ok(page) = Page::fetch(page_url) else { return vec![] };
    let srcs: Vec<String> = page.icon_candidates().into_iter().take(12).collect();
    let got: Vec<Option<Candidate>> = std::thread::scope(|s| {
        let jobs: Vec<_> = srcs
            .into_iter()
            .map(|src| s.spawn(move || download(&src).and_then(|b| normalize(&b)).ok().map(|png| Candidate { src, png })))
            .collect();
        jobs.into_iter().map(|j| j.join().ok().flatten()).collect()
    });
    got.into_iter().flatten().take(8).collect()
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
        assert_eq!(max_size(&format!("{}x1", i64::MAX)), 10_000); // no overflow
    }

    #[test]
    fn oversized_images_are_refused() {
        // A PNG header claiming 100000 x 100000 pixels.
        let mut png = vec![];
        image::DynamicImage::ImageRgba8(RgbaImage::new(32, 32)).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        assert!(normalize(&png).is_ok());
        png[16..20].copy_from_slice(&100_000u32.to_be_bytes());
        png[20..24].copy_from_slice(&100_000u32.to_be_bytes());
        // Keep the header's checksum valid, so the size limit (not a CRC error) is what refuses it.
        let crc = png[12..29].iter().fold(!0u32, |mut c, &b| {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
            }
            c
        });
        png[29..33].copy_from_slice(&(!crc).to_be_bytes());
        let err = format!("{:#}", normalize(&png).unwrap_err());
        assert!(err.contains("oversized") || err.contains("limit"), "{err}");
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
