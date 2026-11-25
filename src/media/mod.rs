pub mod cache;
pub mod downloader;
pub mod renderer;

pub use cache::{ImageCache, ImageKey};
pub use renderer::{ImageManager, ImageStatus};

/// Detect iTerm2 independently of the image-protocol query. iTerm2 answers the
/// Kitty capability probe, but its inline-image protocols are unreliable in a
/// full-screen Ratatui repaint loop, so `Auto` uses half-blocks there.
pub fn is_iterm2_terminal(
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
        assert!(is_iterm2_terminal(Some("iTerm.app"), None, false));
        assert!(is_iterm2_terminal(None, Some("iTerm2"), false));
        assert!(is_iterm2_terminal(None, None, true));
        assert!(!is_iterm2_terminal(Some("ghostty"), Some("Ghostty"), false));
        assert!(!is_iterm2_terminal(None, None, false));
    }
}
