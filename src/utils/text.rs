//! Text handling helpers.
//!
//! All text that comes from Reddit (or any other untrusted source) must pass
//! through [`sanitize`] before it is handed to the terminal. Ratatui itself is
//! mostly safe, but a stray ANSI escape sequence could still be emitted by a
//! custom widget, and escape sequences are never something we want to forward
//! verbatim.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Remove ANSI escape sequences, C0/C1 control characters and bidirectional
/// override characters from untrusted text. Newlines and tabs are preserved
/// (tabs are expanded to four spaces by callers where needed).
pub fn sanitize(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\x1b' => skip_escape_sequence(&mut chars),
            '\n' => out.push('\n'),
            '\t' => out.push_str("    "),
            '\r' => {}
            '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' => {}
            c if c.is_control() => {}
            c => out.push(c),
        }
    }

    out
}

fn skip_escape_sequence(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    match chars.next() {
        // CSI: ESC [ ... final-byte(@..~)
        Some('[') => {
            for c in chars.by_ref() {
                if ('\x40'..='\x7e').contains(&c) {
                    break;
                }
            }
        }
        // OSC: ESC ] ... BEL or ST
        Some(']') => {
            while let Some(c) = chars.next() {
                if c == '\x07' {
                    break;
                }
                if c == '\x1b' {
                    if chars.peek() == Some(&'\\') {
                        chars.next();
                    }
                    break;
                }
            }
        }
        // DCS / SOS / PM / APC: ESC P|X|^|_ ... ST
        Some('P' | 'X' | '^' | '_') => {
            while let Some(c) = chars.next() {
                if c == '\x1b' {
                    if chars.peek() == Some(&'\\') {
                        chars.next();
                    }
                    break;
                }
            }
        }
        _ => {}
    }
}

/// Display width of a string in terminal cells.
pub fn width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Truncate a string to at most `max` terminal cells, appending an ellipsis
/// when it had to be shortened.
pub fn ellipsize(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if width(s) <= max {
        return s.to_string();
    }
    let mut out = String::new();
    let mut used = 0usize;
    let budget = max.saturating_sub(1);
    for g in s.graphemes(true) {
        let w = width(g);
        if used + w > budget {
            break;
        }
        out.push_str(g);
        used += w;
    }
    out.push('…');
    out
}

/// Greedy word wrap that works on terminal display width and never splits a
/// grapheme cluster. Long words are hard-wrapped.
pub fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for raw_line in text.split('\n') {
        let line = raw_line.trim_end();
        wrap_one_line(line, max_width, &mut out);
    }
    out
}

fn wrap_one_line(line: &str, max_width: usize, out: &mut Vec<String>) {
    if line.is_empty() {
        out.push(String::new());
        return;
    }

    let mut current = String::new();
    let mut current_width = 0usize;

    for word in line.split_whitespace() {
        let word_width = width(word);

        if word_width > max_width {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
                current_width = 0;
            }
            let mut chunk = String::new();
            let mut chunk_width = 0usize;
            for g in word.graphemes(true) {
                let gw = width(g);
                if chunk_width + gw > max_width && !chunk.is_empty() {
                    out.push(std::mem::take(&mut chunk));
                    chunk_width = 0;
                }
                chunk.push_str(g);
                chunk_width += gw;
            }
            if !chunk.is_empty() {
                current = chunk;
                current_width = chunk_width;
            }
            continue;
        }

        if current.is_empty() {
            current.push_str(word);
            current_width = word_width;
        } else if current_width + 1 + word_width <= max_width {
            current.push(' ');
            current.push_str(word);
            current_width += 1 + word_width;
        } else {
            out.push(std::mem::take(&mut current));
            current.push_str(word);
            current_width = word_width;
        }
    }

    if !current.is_empty() {
        out.push(current);
    }
}

/// Number of wrapped lines for `text` at the given width.
pub fn wrapped_height(text: &str, max_width: usize) -> usize {
    wrap_text(text, max_width).len()
}

/// Validate and normalise a subreddit name.
///
/// Accepts `rust`, `r/rust`, `/r/rust/`, `https://reddit.com/r/rust`, etc.
pub fn normalize_subreddit(input: &str) -> Option<String> {
    let mut value = input.trim();

    for prefix in [
        "https://www.reddit.com/",
        "http://www.reddit.com/",
        "https://reddit.com/",
        "http://reddit.com/",
        "https://old.reddit.com/",
        "https://oauth.reddit.com/",
    ] {
        if let Some(rest) = value.strip_prefix(prefix) {
            value = rest;
            break;
        }
    }

    let value = value.trim_matches('/');
    let value = value.strip_prefix("r/").unwrap_or(value);
    let value = value.trim_matches('/');
    let value = value.strip_suffix(".json").unwrap_or(value);

    if !(2..=21).contains(&value.len()) {
        return None;
    }
    if !value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some(value.to_ascii_lowercase())
}

/// Strip the most common Markdown markers, producing readable plain text.
pub fn strip_markdown(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '*' | '_' | '~' | '`' => {}
            '[' => {
                // [label](url) -> label (url)
                let mut label = String::new();
                let mut found_end = false;
                for lc in chars.by_ref() {
                    if lc == ']' {
                        found_end = true;
                        break;
                    }
                    label.push(lc);
                }
                if found_end && chars.peek() == Some(&'(') {
                    chars.next();
                    let mut url = String::new();
                    for uc in chars.by_ref() {
                        if uc == ')' {
                            break;
                        }
                        url.push(uc);
                    }
                    if label.eq_ignore_ascii_case(&url) {
                        out.push_str(&label);
                    } else {
                        out.push_str(&label);
                        out.push_str(" (");
                        out.push_str(url.trim());
                        out.push(')');
                    }
                } else {
                    out.push('[');
                    out.push_str(&label);
                    if found_end {
                        out.push(']');
                    }
                }
            }
            '\\' => {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// Format a score/comment count the way Reddit does: 1200 -> 1.2k.
pub fn format_count(value: i64) -> String {
    let negative = value < 0;
    let n = value.unsigned_abs();
    let text = if n < 1_000 {
        n.to_string()
    } else if n < 1_000_000 {
        let whole = n / 1000;
        let tenth = (n % 1000) / 100;
        if tenth == 0 {
            format!("{whole}k")
        } else {
            format!("{whole}.{tenth}k")
        }
    } else {
        let whole = n / 1_000_000;
        let tenth = (n % 1_000_000) / 100_000;
        if tenth == 0 {
            format!("{whole}m")
        } else {
            format!("{whole}.{tenth}m")
        }
    };
    if negative { format!("-{text}") } else { text }
}

/// A compact, optionally center-aligned string that fits a cell width.
pub fn pad_center(s: &str, cells: u16) -> String {
    let w = width(s);
    if w >= cells as usize {
        return ellipsize(s, cells as usize);
    }
    let left = (cells as usize - w) / 2;
    let right = cells as usize - w - left;
    format!("{}{}{}", " ".repeat(left), s, " ".repeat(right))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_removes_escapes_and_controls() {
        let dirty = "hello \x1b[31mred\x1b[0m world\u{0007}\u{202E}";
        assert_eq!(sanitize(dirty), "hello red world");
    }

    #[test]
    fn sanitize_keeps_newlines_and_expands_tabs() {
        assert_eq!(sanitize("a\tb\nc"), "a    b\nc");
    }

    #[test]
    fn wrap_respects_display_width() {
        let lines = wrap_text("one two three four", 7);
        assert_eq!(lines, vec!["one two", "three", "four"]);
    }

    #[test]
    fn wrap_handles_long_words() {
        let lines = wrap_text("abcdefghij", 4);
        assert_eq!(lines, vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn wrap_preserves_blank_lines() {
        assert_eq!(wrap_text("a\n\nb", 10), vec!["a", "", "b"]);
    }

    #[test]
    fn ellipsize_uses_one_cell_for_ellipsis() {
        assert_eq!(ellipsize("abcdef", 3), "ab…");
        assert_eq!(ellipsize("abc", 3), "abc");
    }

    #[test]
    fn counts_are_humanized() {
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1200), "1.2k");
        assert_eq!(format_count(15_600), "15.6k");
        assert_eq!(format_count(1_000_000), "1m");
        assert_eq!(format_count(-1200), "-1.2k");
    }

    #[test]
    fn subreddit_normalisation() {
        assert_eq!(normalize_subreddit("rust").as_deref(), Some("rust"));
        assert_eq!(normalize_subreddit("r/rust").as_deref(), Some("rust"));
        assert_eq!(normalize_subreddit("/r/Rust/").as_deref(), Some("rust"));
        assert_eq!(
            normalize_subreddit("https://www.reddit.com/r/rust/").as_deref(),
            Some("rust")
        );
        assert_eq!(normalize_subreddit("rust.json").as_deref(), Some("rust"));
        assert!(normalize_subreddit("not a subreddit").is_none());
        assert!(normalize_subreddit("r/").is_none());
    }

    #[test]
    fn markdown_strip() {
        assert_eq!(
            strip_markdown("**bold** and [link](https://x.y) and `code`"),
            "bold and link (https://x.y) and code"
        );
    }

    #[test]
    fn cjk_width_used() {
        assert_eq!(width("日本語"), 6);
        assert_eq!(ellipsize("日本語", 4), "日…");
    }
}
