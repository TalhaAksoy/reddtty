//! A small, dependency-free Markdown renderer for Ratatui.
//!
//! This is intentionally not a full CommonMark implementation. It covers the
//! constructs that are common in Reddit posts: headings, bold/italic text,
//! inline code, fenced code blocks, links, block quotes, lists and horizontal
//! rules. Everything is sanitised first, so rendered content can never emit
//! terminal control sequences.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::utils::text::{ellipsize, sanitize, width};

#[derive(Clone, Copy, Debug)]
pub struct MdStyles {
    pub text: Style,
    pub heading: Style,
    pub strong: Style,
    pub emphasis: Style,
    pub code: Style,
    pub link: Style,
    pub quote: Style,
    pub bullet: Style,
}

/// Render Markdown into styled, width-wrapped lines.
pub fn render(md: &str, max_width: usize, styles: MdStyles) -> Vec<Line<'static>> {
    render_limited(md, max_width, styles, None)
}

pub fn render_limited(
    md: &str,
    max_width: usize,
    styles: MdStyles,
    max_lines: Option<usize>,
) -> Vec<Line<'static>> {
    if max_width == 0 {
        return Vec::new();
    }
    let clean = sanitize(md);
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut in_code = false;

    for raw in clean.split('\n') {
        let line = raw.trim_end();
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code = !in_code;
            if in_code {
                out.push(Line::from(Span::styled(
                    "  code".to_string(),
                    styles
                        .code
                        .patch(Style::default().add_modifier(Modifier::DIM)),
                )));
            }
            continue;
        }

        if in_code {
            let content = format!("  {line}");
            out.push(Line::from(Span::styled(
                ellipsize(&content, max_width),
                styles.code,
            )));
            continue;
        }

        if line.trim().is_empty() {
            out.push(Line::default());
            continue;
        }

        // Horizontal rule
        let bare = trimmed.trim_end();
        if (bare.len() >= 3
            && (bare.chars().all(|c| c == '-')
                || bare.chars().all(|c| c == '*')
                || bare.chars().all(|c| c == '_')))
            && !bare.contains(' ')
        {
            out.push(Line::from(Span::styled(
                "─".repeat(max_width),
                styles.quote,
            )));
            continue;
        }

        // Headings
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ') {
            let heading_text = trimmed[hashes..].trim();
            let style = styles.heading;
            let spans = parse_inline(heading_text, style, styles);
            out.extend(wrap_spans(spans, max_width));
            continue;
        }

        // Block quote
        if let Some(rest) = trimmed.strip_prefix('>') {
            let rest = rest.trim_start();
            let spans = parse_inline(rest, styles.text.patch(styles.quote), styles);
            out.extend(wrap_with_prefix(
                spans,
                max_width,
                Span::styled("▌ ".to_string(), styles.quote),
                Span::styled("  ".to_string(), styles.quote),
            ));
            continue;
        }

        // Bullet list
        if let Some(rest) = strip_bullet(trimmed) {
            let spans = parse_inline(rest, styles.text, styles);
            out.extend(wrap_with_prefix(
                spans,
                max_width,
                Span::styled("• ".to_string(), styles.bullet),
                Span::raw("  "),
            ));
            continue;
        }

        // Ordered list
        if let Some((marker, rest)) = strip_ordered(trimmed) {
            let prefix = format!("{marker} ");
            let indent = " ".repeat(width(&prefix));
            let spans = parse_inline(rest, styles.text, styles);
            out.extend(wrap_with_prefix(
                spans,
                max_width,
                Span::styled(prefix, styles.bullet),
                Span::raw(indent),
            ));
            continue;
        }

        let spans = parse_inline(trimmed, styles.text, styles);
        out.extend(wrap_spans(spans, max_width));

        if let Some(limit) = max_lines
            && out.len() >= limit
        {
            out.truncate(limit);
            break;
        }
    }

    if let Some(limit) = max_lines
        && out.len() > limit
    {
        out.truncate(limit);
    }

    out
}

fn strip_bullet(s: &str) -> Option<&str> {
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = s.strip_prefix(marker) {
            return Some(rest.trim_start());
        }
    }
    None
}

fn strip_ordered(s: &str) -> Option<(String, &str)> {
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    let rest = &s[digits.len()..];
    let rest = rest
        .strip_prefix(". ")
        .or_else(|| rest.strip_prefix(") "))?;
    Some((format!("{digits}."), rest))
}

/// Parse inline Markdown into styled spans.
pub fn parse_inline(text: &str, base: Style, styles: MdStyles) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    parse_inline_into(text, base, styles, &mut spans);
    spans
}

fn parse_inline_into(text: &str, base: Style, styles: MdStyles, out: &mut Vec<Span<'static>>) {
    let mut plain = String::new();
    let mut chars = text.char_indices().peekable();

    while let Some((idx, ch)) = chars.next() {
        match ch {
            '\\' => {
                if let Some((_, next)) = chars.next() {
                    plain.push(next);
                }
            }
            '`' => {
                if let Some(close) = find_closing(text, idx + 1, "`") {
                    flush_plain(&mut plain, base, out);
                    let inner = &text[idx + 1..close];
                    out.push(Span::styled(inner.to_string(), base.patch(styles.code)));
                    // Skip to after the closing backtick.
                    while let Some((i, _)) = chars.peek() {
                        if *i <= close {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                } else {
                    plain.push(ch);
                }
            }
            '*' | '_' | '~' => {
                let double = format!("{ch}{ch}");
                if text[idx..].starts_with(&double)
                    && let Some(close) = find_closing(text, idx + 2, &double)
                {
                    flush_plain(&mut plain, base, out);
                    let inner = &text[idx + 2..close];
                    let style = if ch == '~' {
                        base.add_modifier(Modifier::CROSSED_OUT)
                    } else {
                        base.patch(styles.strong)
                    };
                    parse_inline_into(inner, style, styles, out);
                    skip_to(&mut chars, close + double.len());
                    continue;
                }
                let single = ch.to_string();
                if let Some(close) = find_closing(text, idx + 1, &single) {
                    flush_plain(&mut plain, base, out);
                    let inner = &text[idx + 1..close];
                    parse_inline_into(inner, base.patch(styles.emphasis), styles, out);
                    skip_to(&mut chars, close + 1);
                } else {
                    plain.push(ch);
                }
            }
            '[' => {
                if let Some((label, url, end)) = parse_link(text, idx) {
                    flush_plain(&mut plain, base, out);
                    let link_style = base.patch(styles.link);
                    out.push(Span::styled(label, link_style));
                    if !url.is_empty() {
                        out.push(Span::styled(
                            " ↗".to_string(),
                            link_style.add_modifier(Modifier::DIM),
                        ));
                    }
                    skip_to(&mut chars, end);
                } else {
                    plain.push(ch);
                }
            }
            _ => plain.push(ch),
        }
    }

    flush_plain(&mut plain, base, out);
}

fn flush_plain(plain: &mut String, style: Style, out: &mut Vec<Span<'static>>) {
    if !plain.is_empty() {
        out.push(Span::styled(std::mem::take(plain), style));
    }
}

fn skip_to(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>, target: usize) {
    while let Some((i, _)) = chars.peek() {
        if *i < target {
            chars.next();
        } else {
            break;
        }
    }
}

fn find_closing(text: &str, from: usize, needle: &str) -> Option<usize> {
    text[from..].find(needle).map(|i| from + i)
}

fn parse_link(text: &str, start: usize) -> Option<(String, String, usize)> {
    let after = &text[start + 1..];
    let label_end = after.find(']')?;
    let label = &after[..label_end];
    let url_start = start + 1 + label_end + 1;
    let rest = text.get(url_start..)?;
    if !rest.starts_with('(') {
        return Some((label.to_string(), String::new(), url_start));
    }
    let inside = &rest[1..];
    let url_end = inside.find(')')?;
    let url = inside[..url_end].trim().to_string();
    Some((label.to_string(), url, url_start + 1 + url_end + 1))
}

struct Word {
    text: String,
    style: Style,
}

fn collect_words(spans: &[Span<'static>]) -> Vec<Word> {
    let mut words = Vec::new();
    for span in spans {
        for word in span.content.split_whitespace() {
            words.push(Word {
                text: word.to_string(),
                style: span.style,
            });
        }
    }
    words
}

/// Greedy-wrap spans to `max_width` cells.
pub fn wrap_spans(spans: Vec<Span<'static>>, max_width: usize) -> Vec<Line<'static>> {
    let words = collect_words(&spans);
    if words.is_empty() {
        return vec![Line::default()];
    }
    if max_width == 0 {
        return Vec::new();
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut current_width = 0usize;

    for word in words {
        let word_width = width(&word.text);
        if word_width > max_width {
            if !current.is_empty() {
                lines.push(Line::from(std::mem::take(&mut current)));
                current_width = 0;
            }
            let mut chunk = String::new();
            let mut chunk_width = 0;
            for g in unicode_segmentation::UnicodeSegmentation::graphemes(word.text.as_str(), true)
            {
                let gw = width(g);
                if chunk_width + gw > max_width && !chunk.is_empty() {
                    lines.push(Line::from(Span::styled(
                        std::mem::take(&mut chunk),
                        word.style,
                    )));
                    chunk_width = 0;
                }
                chunk.push_str(g);
                chunk_width += gw;
            }
            if !chunk.is_empty() {
                current.push(Span::styled(chunk, word.style));
                current_width = chunk_width;
            }
            continue;
        }

        if current.is_empty() {
            current.push(Span::styled(word.text, word.style));
            current_width = word_width;
        } else if current_width + 1 + word_width <= max_width {
            current.push(Span::raw(" "));
            current.push(Span::styled(word.text, word.style));
            current_width += 1 + word_width;
        } else {
            lines.push(Line::from(std::mem::take(&mut current)));
            current.push(Span::styled(word.text, word.style));
            current_width = word_width;
        }
    }

    if !current.is_empty() {
        lines.push(Line::from(current));
    }

    lines
}

fn wrap_with_prefix(
    spans: Vec<Span<'static>>,
    max_width: usize,
    prefix: Span<'static>,
    indent: Span<'static>,
) -> Vec<Line<'static>> {
    let prefix_width = width(&prefix.content);
    let indent_width = width(&indent.content);
    if max_width <= prefix_width {
        return wrap_spans(spans, max_width);
    }

    let first_width = (max_width - prefix_width).max(1);
    let cont_width = max_width.saturating_sub(indent_width).max(1);
    let words = collect_words(&spans);
    if words.is_empty() {
        return vec![Line::from(prefix)];
    }

    let mut rows: Vec<(bool, Vec<Word>)> = Vec::new();
    let mut current: Vec<Word> = Vec::new();
    let mut current_width = 0usize;
    let mut is_first = true;

    for word in words {
        let budget = if is_first { first_width } else { cont_width };
        let word_width = width(&word.text);

        if word_width > budget {
            if !current.is_empty() {
                rows.push((is_first, std::mem::take(&mut current)));
                is_first = false;
                current_width = 0;
            }
            let mut chunk = String::new();
            let mut chunk_width = 0;
            for g in unicode_segmentation::UnicodeSegmentation::graphemes(word.text.as_str(), true)
            {
                let gw = width(g);
                if chunk_width + gw > budget && !chunk.is_empty() {
                    rows.push((
                        is_first,
                        vec![Word {
                            text: std::mem::take(&mut chunk),
                            style: word.style,
                        }],
                    ));
                    is_first = false;
                    chunk_width = 0;
                }
                chunk.push_str(g);
                chunk_width += gw;
            }
            if !chunk.is_empty() {
                current.push(Word {
                    text: chunk,
                    style: word.style,
                });
                current_width = chunk_width;
            }
            continue;
        }

        if current.is_empty() {
            current.push(word);
            current_width = word_width;
        } else if current_width + 1 + word_width <= budget {
            current.push(word);
            current_width += 1 + word_width;
        } else {
            rows.push((is_first, std::mem::take(&mut current)));
            is_first = false;
            current.push(word);
            current_width = word_width;
        }
    }

    if !current.is_empty() {
        rows.push((is_first, current));
    }

    let mut out = Vec::new();
    for (first, words) in rows {
        let lead = if first {
            prefix.clone()
        } else {
            indent.clone()
        };
        let mut spans = vec![lead];
        for (i, word) in words.into_iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw(" "));
            }
            spans.push(Span::styled(word.text, word.style));
        }
        out.push(Line::from(spans));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn styles() -> MdStyles {
        let base = Style::default();
        MdStyles {
            text: base,
            heading: base.add_modifier(Modifier::BOLD),
            strong: Style::default().add_modifier(Modifier::BOLD),
            emphasis: Style::default().add_modifier(Modifier::ITALIC),
            code: Style::default().add_modifier(Modifier::DIM),
            link: Style::default().add_modifier(Modifier::UNDERLINED),
            quote: Style::default().add_modifier(Modifier::ITALIC),
            bullet: Style::default(),
        }
    }

    fn text_of(lines: &[Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn headings_and_bullets_render() {
        let lines = render("# Title\n\n- one\n- two", 30, styles());
        let text = text_of(&lines);
        assert_eq!(text[0], "Title");
        assert!(text.contains(&"• one".to_string()));
        assert!(text.contains(&"• two".to_string()));
    }

    #[test]
    fn code_blocks_are_preserved() {
        let lines = render("```\nfn main() {}\n```", 30, styles());
        let text = text_of(&lines);
        assert!(text.iter().any(|l| l.contains("fn main() {}")));
    }

    #[test]
    fn inline_styles_are_applied() {
        let lines = render("This is **bold** and *italic*", 60, styles());
        let spans = &lines[0].spans;
        assert!(spans.iter().any(|s| s.content == "bold"));
        assert!(spans.iter().any(|s| s.content == "italic"));
    }

    #[test]
    fn dangerous_escapes_are_removed() {
        let lines = render("\x1b[31mred\x1b[0m", 30, styles());
        assert_eq!(text_of(&lines), vec!["red"]);
    }

    #[test]
    fn width_is_respected() {
        let lines = render("one two three four five six", 9, styles());
        for line in text_of(&lines) {
            assert!(width(&line) <= 9);
        }
    }

    #[test]
    fn link_rendering() {
        let lines = render("[docs](https://example.com)", 40, styles());
        assert_eq!(text_of(&lines), vec!["docs ↗"]);
    }
}
