//! HTML utilities shared by the Old Reddit and Redlib parsers.
//!
//! These helpers never panic on missing or unexpected markup: selectors are
//! static and valid, while every document lookup returns an `Option`.

use scraper::node::Node;
use scraper::{ElementRef, Html, Selector};
use url::Url;

use crate::error::ProviderError;

pub(crate) fn selector(css: &str) -> Selector {
    Selector::parse(css).expect("static CSS selector must be valid")
}

pub(crate) fn first_in<'a>(scope: ElementRef<'a>, css: &str) -> Option<ElementRef<'a>> {
    scope.select(&selector(css)).next()
}

pub(crate) fn all_in<'a>(scope: ElementRef<'a>, css: &str) -> Vec<ElementRef<'a>> {
    scope.select(&selector(css)).collect()
}

pub(crate) fn attr<'a>(element: ElementRef<'a>, name: &str) -> Option<&'a str> {
    element
        .value()
        .attr(name)
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

pub(crate) fn has_class(element: ElementRef<'_>, class: &str) -> bool {
    element
        .value()
        .has_class(class, scraper::CaseSensitivity::AsciiCaseInsensitive)
}

pub(crate) fn has_any_class(element: ElementRef<'_>, classes: &[&str]) -> bool {
    classes.iter().any(|class| has_class(element, class))
}

/// Direct element child with a tag and CSS class.
pub(crate) fn direct_child<'a>(
    element: ElementRef<'a>,
    tag: &str,
    class: &str,
) -> Option<ElementRef<'a>> {
    element
        .children()
        .filter_map(ElementRef::wrap)
        .find(|child| child.value().name() == tag && has_class(*child, class))
}

/// All direct element children with a tag and CSS class.
pub(crate) fn direct_children<'a>(
    element: ElementRef<'a>,
    tag: &str,
    class: &str,
) -> Vec<ElementRef<'a>> {
    element
        .children()
        .filter_map(ElementRef::wrap)
        .filter(|child| child.value().name() == tag && has_class(*child, class))
        .collect()
}

pub(crate) fn text(element: ElementRef<'_>) -> String {
    collapse_whitespace(&element.text().collect::<String>())
}

pub(crate) fn meta_content(doc: &Html, name: &str) -> Option<String> {
    let meta = doc
        .select(&selector(&format!("meta[name=\"{name}\"]")))
        .next()?;
    attr(meta, "content").map(str::to_string)
}

pub(crate) fn page_text(doc: &Html) -> String {
    doc.root_element()
        .text()
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Resolve a possibly relative URL and reject non-HTTP schemes, credentials and
/// fragments. Media URLs and next-page links both go through this gate.
pub(crate) fn absolute_url(base: &Url, raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.starts_with('#') {
        return None;
    }
    let lower = raw.to_ascii_lowercase();
    if lower.starts_with("javascript:")
        || lower.starts_with("data:")
        || lower.starts_with("mailto:")
        || lower.starts_with("file:")
    {
        return None;
    }
    let joined = if raw.starts_with("//") {
        Url::parse(&format!("{}:{}", base.scheme(), raw)).ok()?
    } else {
        base.join(raw).ok()?
    };
    if joined.scheme() != "http" && joined.scheme() != "https" {
        return None;
    }
    if !joined.username().is_empty() || joined.password().is_some() {
        return None;
    }
    let mut clean = joined;
    clean.set_fragment(None);
    Some(clean.to_string())
}

pub(crate) fn is_image_url(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
    let path = parsed.path().to_ascii_lowercase();
    let ext = path.rsplit('.').next().unwrap_or("");
    matches!(ext, "png" | "jpg" | "jpeg" | "gif" | "webp")
        || matches!(
            host.as_str(),
            "i.redd.it" | "preview.redd.it" | "i.imgur.com" | "pbs.twimg.com"
        )
}

pub(crate) fn is_video_url(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
    let path = parsed.path().to_ascii_lowercase();
    let ext = path.rsplit('.').next().unwrap_or("");
    matches!(ext, "mp4" | "webm" | "mov")
        || host == "youtu.be"
        || host.ends_with("youtube.com")
        || host.ends_with("vimeo.com")
        || host.ends_with("streamable.com")
}

pub(crate) fn is_gallery_url(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    let path = parsed.path().to_ascii_lowercase();
    path.contains("/gallery/") || path.contains("/a/")
}

/// Parse human counts such as `1.2k points`, `12,345`, `-4` or `3 comments`.
pub(crate) fn parse_count(raw: &str) -> i64 {
    let lower = raw.trim().to_ascii_lowercase();
    if lower.is_empty() || lower.contains("hidden") || lower.contains("vote") || lower == "score" {
        return 0;
    }
    let chars: Vec<char> = lower.chars().collect();
    let mut start = None;
    let mut end = 0;
    for (index, ch) in chars.iter().copied().enumerate() {
        let numeric = ch.is_ascii_digit() || ch == '.' || ch == ',';
        let signed = ch == '-' && start.is_none();
        if numeric || (signed && chars.get(index + 1).is_some_and(char::is_ascii_digit)) {
            if start.is_none() {
                start = Some(index);
            }
            end = index + 1;
        } else if start.is_some() {
            break;
        }
    }
    let Some(start) = start else {
        return 0;
    };
    let number: String = chars[start..end].iter().filter(|c| **c != ',').collect();
    let Ok(mut value) = number.parse::<f64>() else {
        return 0;
    };
    let suffix = chars[end..].iter().find(|c| !c.is_whitespace()).copied();
    match suffix {
        Some('k') => value *= 1_000.0,
        Some('m') => value *= 1_000_000.0,
        Some('b') => value *= 1_000_000_000.0,
        _ => {}
    }
    value.round() as i64
}

/// Parse ISO-8601, Unix seconds/milliseconds or a short relative phrase.
pub(crate) fn parse_timestamp(raw: &str, now: i64) -> Option<i64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(number) = raw.parse::<i64>() {
        return Some(if number > 10_000_000_000 {
            number / 1_000
        } else {
            number
        });
    }
    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(raw) {
        return Some(parsed.timestamp());
    }
    let without_zone = raw
        .strip_suffix(" UTC")
        .or_else(|| raw.strip_suffix(" GMT"))
        .unwrap_or(raw);
    for format in [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%b %d %Y, %H:%M:%S",
        "%b %d %Y %H:%M:%S",
        "%d %b %Y, %H:%M:%S",
    ] {
        if let Ok(parsed) = chrono::NaiveDateTime::parse_from_str(without_zone, format) {
            return Some(parsed.and_utc().timestamp());
        }
    }
    let lower = raw.to_ascii_lowercase();
    if lower.contains("just now") {
        return Some(now);
    }
    let words: Vec<&str> = lower.split_whitespace().collect();
    // Redlib uses compact ages such as `2h ago`, `3d ago`, `56y ago`.
    for word in &words {
        if let Some((amount, unit)) = compact_age(word)
            && let Some(seconds) = unit_seconds(unit)
        {
            return Some(now.saturating_sub(amount.saturating_mul(seconds)));
        }
    }
    if words.len() < 2 {
        return None;
    }
    let amount = words
        .iter()
        .find_map(|word| word.parse::<i64>().ok())
        .or_else(|| {
            // "a minute ago", "an hour ago"
            words
                .first()
                .filter(|word| **word == "a" || **word == "an")
                .map(|_| 1)
        })?;
    let unit = words
        .iter()
        .find(|word| {
            word.starts_with("sec")
                || word.starts_with("min")
                || word.starts_with("hour")
                || word.starts_with("day")
                || word.starts_with("week")
                || word.starts_with("month")
                || word.starts_with("year")
                || word.starts_with("yr")
                || word.starts_with("mo")
        })
        .copied()?;
    let seconds = unit_seconds(unit)?;
    Some(now.saturating_sub(amount.saturating_mul(seconds)))
}

fn compact_age(word: &str) -> Option<(i64, &str)> {
    let split = word.find(|c: char| !c.is_ascii_digit())?;
    if split == 0 {
        return None;
    }
    let amount = word[..split].parse::<i64>().ok()?;
    let unit = &word[split..];
    if unit.is_empty() {
        return None;
    }
    Some((amount, unit))
}

fn unit_seconds(unit: &str) -> Option<i64> {
    let unit = unit.trim_end_matches('s');
    if unit == "s" || unit.starts_with("sec") {
        Some(1)
    } else if unit == "m" || unit.starts_with("min") {
        Some(60)
    } else if unit == "h" || unit.starts_with("hour") || unit == "hr" {
        Some(3_600)
    } else if unit == "d" || unit.starts_with("day") {
        Some(86_400)
    } else if unit == "w" || unit.starts_with("week") {
        Some(604_800)
    } else if unit == "mo" || unit.starts_with("month") {
        Some(2_592_000)
    } else if unit == "y" || unit.starts_with("year") || unit == "yr" {
        Some(31_536_000)
    } else {
        None
    }
}

/// Convert a Markdown-ish HTML body into plain text, preserving paragraph and
/// list breaks that the existing renderer understands.
pub(crate) fn html_to_text(element: ElementRef<'_>) -> String {
    let mut out = String::new();
    walk_html(element, &mut out);
    normalise_blocks(&out)
}

fn walk_html(element: ElementRef<'_>, out: &mut String) {
    for child in element.children() {
        match child.value() {
            Node::Text(text) => out.push_str(&text.text),
            Node::Element(_) => {
                let Some(child) = ElementRef::wrap(child) else {
                    continue;
                };
                if has_class(child, "hidden") || child.value().attr("hidden").is_some() {
                    continue;
                }
                let tag = child.value().name();
                let block = matches!(
                    tag,
                    "p" | "div"
                        | "section"
                        | "article"
                        | "blockquote"
                        | "pre"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                        | "ul"
                        | "ol"
                        | "table"
                );
                if block && !out.ends_with('\n') {
                    out.push('\n');
                }
                if tag == "li" {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str("• ");
                }
                if tag == "br" {
                    out.push('\n');
                } else {
                    walk_html(child, out);
                }
                if (block || tag == "li") && !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            _ => {}
        }
    }
}

fn normalise_blocks(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut pending_space = false;
    let mut newlines = 0;
    for ch in raw.chars() {
        if ch == '\r' {
            continue;
        }
        if ch == '\n' {
            while out.ends_with(' ') {
                out.pop();
            }
            newlines += 1;
            if newlines <= 2 {
                out.push('\n');
            }
            pending_space = false;
            continue;
        }
        if ch.is_whitespace() {
            pending_space = true;
            continue;
        }
        if pending_space && !out.is_empty() && !out.ends_with('\n') {
            out.push(' ');
        }
        pending_space = false;
        newlines = 0;
        out.push(ch);
    }
    out.trim().to_string()
}

pub(crate) fn collapse_whitespace(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract a short plain-text snippet from an error page so the UI does not
/// dump raw HTML into a toast or error screen.
pub(crate) fn plain_text(body: &str, max: usize) -> String {
    let doc = Html::parse_document(body);
    let text = collapse_whitespace(&doc.root_element().text().collect::<String>());
    if text.chars().count() <= max {
        text
    } else {
        let mut out: String = text.chars().take(max).collect();
        out.push('…');
        out
    }
}

/// Detect a login wall, CAPTCHA or explicit block page. This must only be used
/// when the expected content container is missing, because Old Reddit includes
/// a login form in the header of ordinary pages.
pub(crate) fn detect_gate(doc: &Html) -> Option<ProviderError> {
    let lower = page_text(doc);
    let compact = collapse_whitespace(&lower).to_ascii_lowercase();
    let title = first_in(doc.root_element(), "title")
        .map(text)
        .unwrap_or_default()
        .to_ascii_lowercase();

    let login_phrase = [
        "you must be logged in",
        "log in to continue",
        "login to continue",
        "please log in",
        "sign in to continue",
        "you need to log in",
        "login required",
        "private community",
        "this community is private",
    ]
    .iter()
    .any(|phrase| compact.contains(phrase));
    if login_phrase || (title.contains("login") && compact.contains("password")) {
        return Some(ProviderError::LoginRequired(
            "the source returned a login page instead of content. HTML mode cannot authenticate; use demo mode or an approved source.".into(),
        ));
    }

    if compact.contains("too many requests") || compact.contains("rate limit exceeded") {
        return Some(ProviderError::RateLimited { retry_after: None });
    }

    let blocked = [
        "access denied",
        "forbidden",
        "captcha",
        "verify you are human",
        "unusual traffic",
        "blocked by the instance",
        "instance is disabled",
        "instance has been disabled",
        "this instance is unavailable",
        "cloudflare",
        "reddit blocked this request",
        "403 forbidden",
    ]
    .iter()
    .any(|phrase| compact.contains(phrase));
    if blocked {
        return Some(ProviderError::Forbidden(
            "the source returned an access/CAPTCHA page. ReddTTY does not bypass access controls."
                .into(),
        ));
    }
    None
}

/// The first next-page link, resolved against the page URL.
pub(crate) fn next_page_url(doc: &Html, page: &Url) -> Option<String> {
    // Redlib emits a bare `<footer><a accesskey="N" href="?sort=...&after=...">NEXT</a></footer>`
    // rather than a rel=next link or a pagination class. Keep the other forms
    // for Old Reddit and older Redlib versions.
    let next_selector = selector(
        "a[rel=next], a[accesskey=N], .nav-buttons a, .pagination a, a.next, footer a[href*=\"after=\"]",
    );
    let anchors = doc.select(&next_selector);
    for anchor in anchors {
        let text = text(anchor).to_ascii_lowercase();
        let rel = attr(anchor, "rel").unwrap_or("");
        if (rel.eq_ignore_ascii_case("next")
            || text.contains("next")
            || text.contains('›')
            || text.contains('→'))
            && let Some(url) = attr(anchor, "href").and_then(|href| absolute_url(page, href))
        {
            return Some(url);
        }
    }
    None
}

pub(crate) fn structure_error(source: &str, detail: &str) -> ProviderError {
    ProviderError::HtmlStructureChanged(format!(
        "{source} did not contain the expected HTML ({detail}). The source may have changed or returned a different page."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_redlib_absolute_and_compact_timestamps() {
        let now = 1_800_000_000;
        assert_eq!(
            parse_timestamp("Nov 23 2026, 08:13:22 UTC", now),
            Some(1_795_421_602)
        );
        assert_eq!(parse_timestamp("2h ago", now), Some(now - 2 * 3_600));
        assert_eq!(parse_timestamp("56y ago", now), Some(now - 56 * 31_536_000));
        assert_eq!(parse_timestamp("just now", now), Some(now));
        assert_eq!(
            parse_timestamp("2024-01-01T00:00:00Z", now),
            Some(1_704_067_200)
        );
    }

    #[test]
    fn counts_and_urls_are_safe() {
        assert_eq!(parse_count("1.2k points"), 1_200);
        assert_eq!(parse_count("52.6k"), 52_600);
        assert_eq!(parse_count("12,345"), 12_345);
        assert_eq!(parse_count("-4"), -4);
        assert_eq!(parse_count("score hidden"), 0);
        let base = Url::parse("https://redlib.example/r/rust/").unwrap();
        assert_eq!(
            absolute_url(&base, "//i.redd.it/a.jpg").as_deref(),
            Some("https://i.redd.it/a.jpg")
        );
        assert_eq!(absolute_url(&base, "javascript:alert(1)"), None);
        assert!(is_image_url("https://i.redd.it/a.jpg"));
        assert!(!is_image_url("https://redlib.example/r/rust/"));
        assert!(is_video_url("https://youtu.be/abc"));
    }
}
