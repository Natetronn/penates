//! What Penates reads from a website: its name and its icons. The page (and its PWA manifest, if it
//! links one) is downloaded once, and relative links resolve against the address the site finally
//! answered from, after redirects.

use crate::icon;
use scraper::{Html, Selector};
use url::Url;

/// The parts of a page Penates uses.
pub struct Page {
    /// Where the page came from after redirects.
    pub final_url: Url,
    meta_name: Option<String>,
    title: Option<String>,
    /// `<link rel=…icon…>` entries: (score, absolute URL).
    icon_links: Vec<(i64, String)>,
    /// The PWA manifest's address and text.
    manifest: Option<(Url, String)>,
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn short(s: String) -> Option<String> {
    (!s.is_empty() && s.chars().count() <= 40).then_some(s)
}

fn meta(doc: &Html, css: &str) -> Option<String> {
    let sel = Selector::parse(css).ok()?;
    doc.select(&sel).next()?.value().attr("content").map(clean).filter(|s| !s.is_empty())
}

/// "Inbox - Proton Mail" -> the segment that names the site: the one matching the site's host label
/// ("proton"), else the last one (titles usually read "Page - Site").
fn from_title(title: &str, host: Option<&str>) -> Option<String> {
    let t = clean(title);
    let segs: Vec<&str> = t
        .split(['|', '·', '—', '–', '•'])
        .flat_map(|seg| seg.split(" - "))
        .map(str::trim)
        .filter(|s| s.chars().count() >= 2)
        .collect();
    let label = host.map(crate::site_label).unwrap_or_default();
    let by_host = (!label.is_empty())
        .then(|| segs.iter().find(|s| crate::slugify(s).replace('-', "").contains(&label)))
        .flatten();
    by_host.or(segs.last()).map(|s| s.to_string())
}

impl Page {
    /// Parse a page that is already downloaded. `fetch_manifest` also downloads its PWA manifest.
    pub fn from_html(final_url: Url, html: &str, fetch_manifest: bool) -> Page {
        let doc = Html::parse_document(html);
        let meta_name = meta(&doc, r#"meta[property="og:site_name"]"#)
            .or_else(|| meta(&doc, r#"meta[name="application-name"]"#))
            .or_else(|| meta(&doc, r#"meta[name="apple-mobile-web-app-title"]"#))
            .and_then(short);
        let title = Selector::parse("title").ok().and_then(|s| doc.select(&s).next().map(|t| t.text().collect::<String>()));
        let mut icon_links = vec![];
        let mut manifest_href = None;
        let links = Selector::parse("link[rel][href]").expect("valid selector");
        for el in doc.select(&links) {
            let rel = el.value().attr("rel").unwrap_or("").to_ascii_lowercase();
            let Some(href) = el.value().attr("href").and_then(|h| final_url.join(h).ok()) else { continue };
            if rel.split_whitespace().any(|t| t == "manifest") {
                manifest_href.get_or_insert(href);
            } else if rel.contains("icon") {
                let bonus = if rel.contains("apple-touch") { 1000 } else { 0 };
                icon_links.push((icon::max_size(el.value().attr("sizes").unwrap_or("")) + bonus, href.to_string()));
            }
        }
        let manifest = manifest_href
            .filter(|u| fetch_manifest && matches!(u.scheme(), "http" | "https"))
            .and_then(|u| crate::http::get_text(u.as_str()).ok().map(|body| (u, body)));
        Page { final_url, meta_name, title, icon_links, manifest }
    }

    /// Download `url` (following redirects) and its PWA manifest.
    pub fn fetch(url: &str) -> anyhow::Result<Page> {
        let got = crate::http::get(url, crate::http::MAX_BYTES)?;
        let final_url = Url::parse(&got.final_url)?;
        Ok(Page::from_html(final_url, &String::from_utf8_lossy(&got.body), true))
    }

    /// The site's own name: meta tags, then the PWA manifest, then the page title.
    pub fn name(&self) -> Option<String> {
        self.meta_name
            .clone()
            .or_else(|| self.manifest.as_ref().and_then(|(_, body)| icon::parse_manifest_name(body)))
            .or_else(|| self.title.as_deref().and_then(|t| from_title(t, self.final_url.host_str())).and_then(short))
    }

    /// Icon URLs, best first: PWA manifest icons, apple-touch icons, `<link rel=icon>` by size,
    /// then `/favicon.ico` as a last resort.
    pub fn icon_candidates(&self) -> Vec<String> {
        let mut found = self.icon_links.clone();
        if let Some((murl, body)) = &self.manifest {
            found.extend(icon::parse_manifest_icons(murl, body));
        }
        found.sort_by_key(|f| std::cmp::Reverse(f.0));
        let mut urls: Vec<String> = vec![];
        let favicon = self.final_url.join("/favicon.ico").ok().map(|u| u.to_string());
        for u in found.into_iter().map(|(_, u)| u).chain(favicon) {
            if !urls.contains(&u) {
                urls.push(u);
            }
        }
        urls
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(url: &str, html: &str) -> Page {
        Page::from_html(Url::parse(url).unwrap(), html, false)
    }

    #[test]
    fn prefers_og_site_name() {
        let p = page("https://github.com/", r#"<head><meta property="og:site_name" content="GitHub"><title>Home · Something else</title></head>"#);
        assert_eq!(p.name().as_deref(), Some("GitHub"));
    }

    #[test]
    fn title_segment_matching_the_site_wins() {
        let gh = page("https://github.com/", "<title>GitHub · Build and ship software on a single platform</title>");
        assert_eq!(gh.name().as_deref(), Some("GitHub"));
        let pm = page("https://mail.proton.me/", "<title>Inbox - Proton Mail</title>");
        assert_eq!(pm.name().as_deref(), Some("Proton Mail"));
        let other = page("https://example.org/", "<title>Dashboard | Acme</title>");
        assert_eq!(other.name().as_deref(), Some("Acme"));
        assert!(page("https://example.org/", "<p>nothing</p>").name().is_none());
    }

    #[test]
    fn relative_icons_resolve_against_the_final_address() {
        let p = page(
            "https://www.example.com/app/",
            r#"<link rel="icon" href="i/16.png" sizes="16x16"><link rel="apple-touch-icon" href="/touch.png">"#,
        );
        assert_eq!(
            p.icon_candidates(),
            ["https://www.example.com/touch.png", "https://www.example.com/app/i/16.png", "https://www.example.com/favicon.ico"]
        );
    }
}
