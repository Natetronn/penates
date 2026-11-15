//! Read a site's own name from its HTML meta tags.

use scraper::{Html, Selector};

fn meta(doc: &Html, css: &str) -> Option<String> {
    let sel = Selector::parse(css).ok()?;
    doc.select(&sel).next()?.value().attr("content").map(clean).filter(|s| !s.is_empty())
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "Page title · Site" -> pick the shortest segment, which is almost always the site name.
fn from_title(title: &str) -> Option<String> {
    let t = clean(title);
    t.split(|c| matches!(c, '|' | '·' | '—' | '–' | '•'))
        .flat_map(|seg| seg.split(" - "))
        .map(str::trim)
        .filter(|s| s.chars().count() >= 2)
        .min_by_key(|s| s.chars().count())
        .map(str::to_string)
}

fn short(s: String) -> Option<String> {
    (s.chars().count() <= 40).then_some(s)
}

/// Names declared in the page's meta tags (no network).
fn name_from_meta(html: &str) -> Option<String> {
    let doc = Html::parse_document(html);
    meta(&doc, r#"meta[property="og:site_name"]"#)
        .or_else(|| meta(&doc, r#"meta[name="application-name"]"#))
        .or_else(|| meta(&doc, r#"meta[name="apple-mobile-web-app-title"]"#))
        .and_then(short)
}

fn name_from_title(html: &str) -> Option<String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse("title").ok()?;
    from_title(&doc.select(&sel).next()?.text().collect::<String>()).and_then(short)
}

/// Meta tags, then the page `<title>` (no network).
pub fn name_from_html(html: &str) -> Option<String> {
    name_from_meta(html).or_else(|| name_from_title(html))
}

/// Fetch the page and read its name: meta tags, then the PWA manifest, then the title.
/// None when offline or nothing usable is found.
pub fn name(page_url: &str) -> Option<String> {
    let html = crate::icon::agent().get(page_url).call().ok()?.into_string().ok()?;
    if let Some(n) = name_from_meta(&html) {
        return Some(n);
    }
    if let Ok(base) = url::Url::parse(page_url) {
        if let Some((_, body)) = crate::icon::fetch_manifest(&html, &base) {
            if let Some(n) = crate::icon::parse_manifest_name(&body) {
                return Some(n);
            }
        }
    }
    name_from_title(&html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_og_site_name() {
        let h = r#"<head><meta property="og:site_name" content="GitHub"><title>Home · Something else</title></head>"#;
        assert_eq!(name_from_html(h).as_deref(), Some("GitHub"));
    }

    #[test]
    fn falls_back_to_short_title_segment() {
        assert_eq!(name_from_html("<title>GitHub · Build and ship software on a single platform</title>").as_deref(), Some("GitHub"));
        assert_eq!(name_from_html("<title>Inbox - Proton Mail</title>").as_deref(), Some("Inbox"));
        assert!(name_from_html("<p>nothing</p>").is_none());
    }
}
