//! App icons: found on the site, chosen from a file or URL, stored as our own 256 px PNG.

use crate::{http, site::Page, store};
use anyhow::{Context, Result};
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

/// Icons listed in a PWA web manifest, scored (bigger is better). Monochrome icons (a single-color
/// silhouette meant to be tinted by the system) are left out. Maskable-only icons are cropped to a
/// full square with padding, so they rank below the site's ordinary icons of 64 px or more.
pub fn parse_manifest_icons(manifest_url: &Url, json: &str) -> Vec<(i64, String)> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return vec![] };
    let Some(icons) = v.get("icons").and_then(|i| i.as_array()) else { return vec![] };
    icons
        .iter()
        .filter_map(|i| {
            let src = manifest_url.join(i.get("src")?.as_str()?).ok()?;
            let size = max_size(i.get("sizes").and_then(|s| s.as_str()).unwrap_or(""));
            let purpose = i.get("purpose").and_then(|p| p.as_str()).unwrap_or("any").to_ascii_lowercase();
            let purposes: Vec<&str> = purpose.split_whitespace().collect();
            if purposes.is_empty() || purposes.contains(&"any") {
                Some((size + 1500, src.to_string()))
            } else if purposes.contains(&"maskable") {
                Some((size / 8, src.to_string()))
            } else {
                None // monochrome only
            }
        })
        .collect()
}

/// Whether a normalized icon is a one-tone glyph: every visible pixel about the same gray, white or
/// black (like a monochrome silhouette), which looks flat and can vanish against a light or dark
/// panel. Such icons are used only when the site has nothing better.
pub fn is_flat(png: &[u8]) -> bool {
    let Ok(img) = image::load_from_memory(png) else { return false };
    let img = img.to_rgba8();
    let mut lum: Vec<u8> = vec![];
    let mut colored = 0usize;
    for p in img.pixels().filter(|p| p[3] >= 128) {
        let (max, min) = (p[0].max(p[1]).max(p[2]), p[0].min(p[1]).min(p[2]));
        if max - min > 40 {
            colored += 1;
        }
        lum.push(((299 * p[0] as u32 + 587 * p[1] as u32 + 114 * p[2] as u32) / 1000) as u8);
    }
    if lum.len() < 64 {
        return true; // next to nothing visible
    }
    if colored * 50 > lum.len() {
        return false; // more than 2% colored pixels
    }
    lum.sort_unstable();
    let at = |q: usize| lum[(lum.len() - 1) * q / 100];
    at(95) - at(5) < 48
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

/// Decode one image with size limits (4096 px a side, 64 MB), so a hostile file cannot exhaust memory.
fn decode_limited(bytes: &[u8]) -> Result<image::DynamicImage> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode()?)
}

/// The images inside an ICO file, largest first, each as a standalone file: embedded PNGs as they are,
/// bitmap entries wrapped in a one-entry ICO. Used when the ICO decoder refuses the file as a whole
/// (it only accepts full-color PNG entries, but many favicons embed grayscale or palette PNGs).
fn ico_entries(bytes: &[u8]) -> Vec<Vec<u8>> {
    let u16_at = |i: usize| bytes.get(i..i + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |i: usize| bytes.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    if u16_at(0) != Some(0) || u16_at(2) != Some(1) {
        return vec![];
    }
    let count = u16_at(4).unwrap_or(0) as usize;
    let mut entries: Vec<(u32, Vec<u8>)> = (0..count.min(64))
        .filter_map(|n| {
            let dir = 6 + n * 16;
            let side = |b: u8| if b == 0 { 256 } else { b as u32 };
            let size = side(*bytes.get(dir)?) * side(*bytes.get(dir + 1)?);
            let (len, off) = (u32_at(dir + 8)? as usize, u32_at(dir + 12)? as usize);
            let data = bytes.get(off..off.checked_add(len)?)?;
            let file = if data.starts_with(b"\x89PNG") {
                data.to_vec()
            } else {
                // A one-entry ICO around this bitmap: header, the directory entry (data now at offset 22), data.
                let mut one = vec![0, 0, 1, 0, 1, 0];
                one.extend_from_slice(&bytes[dir..dir + 12]);
                one.extend_from_slice(&22u32.to_le_bytes());
                one.extend_from_slice(data);
                one
            };
            Some((size, file))
        })
        .collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.0));
    entries.into_iter().map(|e| e.1).collect()
}

/// Decode any supported image, trying an ICO's entries one by one when the file as a whole fails.
fn decode(bytes: &[u8]) -> Result<image::DynamicImage> {
    match decode_limited(bytes) {
        Ok(img) => Ok(img),
        Err(e) => ico_entries(bytes).iter().find_map(|entry| decode_limited(entry).ok()).ok_or(e),
    }
}

/// Decode any supported image into a 256x256 transparent-padded square PNG.
pub fn normalize(bytes: &[u8]) -> Result<Vec<u8>> {
    anyhow::ensure!(bytes.len() <= MAX_ICON_BYTES, "image is larger than 8 MB");
    let img = decode(bytes).context("unsupported or oversized image (use PNG, JPG, ICO, GIF or WebP up to 4096 px)")?;
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

/// The best usable icon on a page, normalized (not stored): the highest-ranked one that is not
/// a one-tone glyph, else the highest-ranked usable one.
pub fn best_from(page: &Page) -> Result<Vec<u8>> {
    let mut flat = None;
    for c in page.icon_candidates() {
        if let Ok(png) = download(&c).and_then(|b| normalize(&b)) {
            if !is_flat(&png) {
                return Ok(png);
            }
            flat.get_or_insert(png);
        }
    }
    flat.ok_or_else(|| anyhow::anyhow!("could not find a usable icon for {}", page.final_url))
}

/// Fetch the site's best icon and store it (the default icon).
pub fn fetch_for(page_url: &str, id: &str) -> Result<PathBuf> {
    Ok(save_png(id, &best_from(&Page::fetch(page_url)?)?)?.0)
}

/// How long an icon shown in the catalog listing is reused before it is fetched again.
const CATALOG_ICON_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(30 * 24 * 3600);

fn catalog_icon_dir() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("penates").join("catalog-icons")
}

/// The cached icon's file name: the app id and a hash of where the icon comes from (the entry's icon address,
/// else the site) and this Penates version, so a catalog entry that changes its icon, or a Penates that chooses
/// site icons differently, fetches it again instead of showing the old one for up to 30 days.
fn catalog_icon_name(id: &str, url: &str, icon_url: Option<&str>) -> String {
    let source = format!("{}\0{}", icon_url.unwrap_or(url), env!("CARGO_PKG_VERSION"));
    let hash = source.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
    format!("{id}-{hash:016x}.png")
}

/// Whether `name` is a cached icon of app `id` (from any source, or the older `<id>.png`).
fn is_catalog_icon_of(name: &str, id: &str) -> bool {
    name == format!("{id}.png")
        || name
            .strip_prefix(id)
            .and_then(|r| r.strip_prefix('-'))
            .and_then(|r| r.strip_suffix(".png"))
            .is_some_and(|h| h.len() == 16 && h.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// The icon for a catalog app (for browsing, before it is added): the cached copy while it is fresh,
/// else the entry's own icon address, else the site's best icon. A normalized 256 px PNG.
pub fn catalog_icon(id: &str, url: &str, icon_url: Option<&str>) -> Result<Vec<u8>> {
    store::check_id(id)?;
    let site = Url::parse(url)?;
    anyhow::ensure!(matches!(site.scheme(), "http" | "https"), "only http(s) sites");
    let cached = catalog_icon_dir().join(catalog_icon_name(id, url, icon_url));
    let fresh = std::fs::metadata(&cached)
        .and_then(|m| m.modified())
        .is_ok_and(|t| t.elapsed().is_ok_and(|age| age < CATALOG_ICON_MAX_AGE));
    if fresh {
        if let Ok(png) = std::fs::read(&cached) {
            return Ok(png);
        }
    }
    let png = match icon_url.and_then(|u| fetch_png(u).ok()) {
        Some(png) => png,
        None => best_from(&Page::fetch(url)?)?,
    };
    // A cache: failing to write it is not an error. Older copies of this app's icon go.
    if let Ok(entries) = std::fs::read_dir(catalog_icon_dir()) {
        for e in entries.flatten() {
            if is_catalog_icon_of(&e.file_name().to_string_lossy(), id) && e.path() != cached {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let _ = store::write_atomic(&cached, &png);
    Ok(png)
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
    // One-tone glyphs last, as with the default choice.
    let mut got: Vec<Candidate> = got.into_iter().flatten().collect();
    got.sort_by_key(|c| is_flat(&c.png));
    got.into_iter().take(8).collect()
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
        // As on youtube.com: a monochrome silhouette listed first, then a maskable icon.
        let json = r#"{"name":"My App","short_name":"MyApp","icons":[
            {"src":"/i/mono.png","sizes":"512x512","purpose":"monochrome"},
            {"src":"/i/512m.png","sizes":"512x512","purpose":"maskable"},
            {"src":"/i/192.png","sizes":"192x192"},
            {"src":"/i/512.png","sizes":"512x512","purpose":"any"},
            {"src":"/i/both.png","sizes":"256x256","purpose":"any maskable"}]}"#;
        let mut v = parse_manifest_icons(&base, json);
        v.sort_by_key(|f| std::cmp::Reverse(f.0));
        let order: Vec<&str> = v.iter().map(|f| f.1.trim_start_matches("https://app.example/i/")).collect();
        assert_eq!(order, ["512.png", "both.png", "192.png", "512m.png"]);
        assert!(v.iter().find(|f| f.1.ends_with("512m.png")).unwrap().0 < 100, "maskable ranks below ordinary 64 px+ icons");
        assert_eq!(parse_manifest_name(json).as_deref(), Some("MyApp"));
        assert_eq!(max_size("48x48 96x96"), 96);
        assert_eq!(max_size("any"), 0);
        assert_eq!(max_size(&format!("{}x1", i64::MAX)), 10_000); // no overflow
    }

    /// An ICO whose largest entry is a grayscale PNG (as on login.tailscale.com), plus a 16 px one.
    #[test]
    fn ico_with_a_grayscale_png_entry_decodes() {
        let mut gray = vec![];
        image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(64, 64, image::Luma([200])))
            .write_to(&mut std::io::Cursor::new(&mut gray), image::ImageFormat::Png)
            .unwrap();
        let mut small = vec![];
        image::DynamicImage::ImageRgba8(RgbaImage::new(16, 16))
            .write_to(&mut std::io::Cursor::new(&mut small), image::ImageFormat::Png)
            .unwrap();
        let mut ico = vec![0, 0, 1, 0, 2, 0];
        let first = 6 + 2 * 16;
        for (side, data, off) in [(64u8, &gray, first), (16u8, &small, first + gray.len())] {
            ico.extend_from_slice(&[side, side, 0, 0, 1, 0, 32, 0]);
            ico.extend_from_slice(&(data.len() as u32).to_le_bytes());
            ico.extend_from_slice(&(off as u32).to_le_bytes());
        }
        ico.extend_from_slice(&gray);
        ico.extend_from_slice(&small);
        assert!(decode_limited(&ico).is_err(), "the library still refuses it; this test guards the fallback");
        let png = normalize(&ico).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (256, 256));
        assert_eq!(img.get_pixel(128, 128).0, [200, 200, 200, 255], "the 64 px grayscale entry was used");
        assert!(ico_entries(b"not an ico").is_empty());
    }

    fn png_of(img: RgbaImage) -> Vec<u8> {
        let mut out = vec![];
        image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        out
    }

    #[test]
    fn one_tone_glyphs_are_flat() {
        // A white play-button silhouette on transparent (YouTube's monochrome icon), and dark-gray dots
        // on transparent (login.tailscale.com's favicon).
        let mut white = RgbaImage::new(64, 64);
        let mut gray = RgbaImage::new(64, 64);
        for (x, y, p) in white.enumerate_pixels_mut() {
            if (8..56).contains(&x) && (16..48).contains(&y) {
                *p = image::Rgba([255, 255, 255, 255]);
            }
        }
        for (x, y, p) in gray.enumerate_pixels_mut() {
            if (x / 8) % 2 == 0 && (y / 8) % 2 == 0 {
                *p = image::Rgba([70, 70, 70, 255]);
            }
        }
        assert!(is_flat(&png_of(white.clone())));
        assert!(is_flat(&png_of(gray)));
        assert!(is_flat(&png_of(RgbaImage::new(64, 64))), "nothing visible");
        // A red button with a white triangle, and white dots on a dark square (tailscale.com), are not.
        let mut red = white.clone();
        for p in red.pixels_mut().filter(|p| p[3] > 0) {
            *p = image::Rgba([255, 0, 51, 255]);
        }
        red.put_pixel(30, 30, image::Rgba([255, 255, 255, 255]));
        assert!(!is_flat(&png_of(red)));
        let mut dark = RgbaImage::from_pixel(64, 64, image::Rgba([30, 30, 30, 255]));
        for (x, y, p) in dark.enumerate_pixels_mut() {
            if (x / 8) % 2 == 1 && (y / 8) % 2 == 1 {
                *p = image::Rgba([255, 255, 255, 255]);
            }
        }
        assert!(!is_flat(&png_of(dark)));
    }

    #[test]
    fn a_new_catalog_icon_source_gets_a_new_cache_file() {
        let site = catalog_icon_name("gmail", "https://mail.google.com/", None);
        let own = catalog_icon_name("gmail", "https://mail.google.com/", Some("https://www.gstatic.com/gmail.png"));
        assert_ne!(site, own);
        assert_eq!(own, catalog_icon_name("gmail", "https://mail.google.com/", Some("https://www.gstatic.com/gmail.png")));
        assert!(is_catalog_icon_of(&site, "gmail") && is_catalog_icon_of("gmail.png", "gmail"));
        // Another app whose id starts the same is not touched.
        let calendar = catalog_icon_name("google-calendar", "https://calendar.google.com/", None);
        assert!(!is_catalog_icon_of(&calendar, "google"));
        assert!(!is_catalog_icon_of("gmail-old.png", "gmail"));
    }

    #[test]
    fn catalog_icons_check_their_inputs_before_any_download() {
        assert!(catalog_icon("../x", "https://example.com/", None).is_err());
        assert!(catalog_icon("ok", "file:///etc/passwd", None).is_err());
        assert!(catalog_icon("ok", "not a url", None).is_err());
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
