pub mod cache;
pub mod downloader;
pub mod renderer;

pub use cache::{ImageCache, ImageKey};
pub use renderer::{ImageManager, ImageStatus};

/// iTerm2 answers the Kitty capability query, but its native inline-image
/// protocol is the reliable path for Kitty-style placeholder rendering. Detect
/// it independently of the query so `Auto` can pick iTerm2.
pub fn prefers_iterm2_native(
    term_program: Option<&str>,
    lc_terminal: Option<&str>,
    has_iterm_session: bool,
) -> bool {
    has_iterm_session
        || term_program.is_some_and(|value| value.eq_ignore_ascii_case("iTerm.app"))
        || lc_terminal.is_some_and(|value| value.eq_ignore_ascii_case("iTerm2"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_iterm2_through_each_environment_marker() {
        assert!(prefers_iterm2_native(Some("iTerm.app"), None, false));
        assert!(prefers_iterm2_native(None, Some("iTerm2"), false));
        assert!(prefers_iterm2_native(None, None, true));
        assert!(!prefers_iterm2_native(
            Some("ghostty"),
            Some("Ghostty"),
            false
        ));
        assert!(!prefers_iterm2_native(None, None, false));
    }
}
