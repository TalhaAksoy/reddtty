//! Relative time formatting.

use chrono::{DateTime, Utc};

/// Format a Unix timestamp as a compact relative time ("5m ago", "3d ago").
pub fn relative(created_utc: i64, now: i64) -> String {
    let delta = now.saturating_sub(created_utc);
    if delta <= 5 {
        return "just now".to_string();
    }
    if delta < 60 {
        return format!("{delta}s ago");
    }
    if delta < 3_600 {
        return format!("{}m ago", delta / 60);
    }
    if delta < 86_400 {
        return format!("{}h ago", delta / 3_600);
    }
    if delta < 2_592_000 {
        return format!("{}d ago", delta / 86_400);
    }
    if delta < 31_536_000 {
        return format!("{}mo ago", delta / 2_592_000);
    }
    format!("{}y ago", delta / 31_536_000)
}

pub fn relative_now(created_utc: i64) -> String {
    relative(created_utc, Utc::now().timestamp())
}

/// Absolute date used in the detail screen, e.g. "Nov 23, 2026 • 14:05 UTC".
pub fn absolute(created_utc: i64) -> String {
    match DateTime::<Utc>::from_timestamp(created_utc, 0) {
        Some(dt) => dt.format("%b %d, %Y • %H:%M UTC").to_string(),
        None => "unknown date".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_boundaries() {
        let now = 1_700_000_000;
        assert_eq!(relative(now - 2, now), "just now");
        assert_eq!(relative(now - 42, now), "42s ago");
        assert_eq!(relative(now - 120, now), "2m ago");
        assert_eq!(relative(now - 7_200, now), "2h ago");
        assert_eq!(relative(now - 3 * 86_400, now), "3d ago");
        assert_eq!(relative(now - 60 * 86_400, now), "2mo ago");
        assert_eq!(relative(now - 800 * 86_400, now), "2y ago");
    }

    #[test]
    fn future_timestamps_are_clamped() {
        assert_eq!(relative(2_000, 1_000), "just now");
    }
}
