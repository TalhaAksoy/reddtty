use thiserror::Error;

/// Provider failures, already mapped to a user-facing category.
#[derive(Clone, Debug, Error)]
pub enum ProviderError {
    #[error("Reddit authentication is required or expired. {0}")]
    AuthRequired(String),
    #[error("Reddit refused this request: {0}")]
    Forbidden(String),
    #[error("Content not found: {0}")]
    NotFound(String),
    #[error("Rate limited{}", .retry_after.map(|s| format!(" (retry in {s}s)")).unwrap_or_default())]
    RateLimited { retry_after: Option<u64> },
    #[error("Reddit server error ({0}): {1}")]
    Server(u16, String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Request timed out after {0}s")]
    Timeout(u64),
    #[error("Could not parse the provider response: {0}")]
    Parse(String),
    #[error("Login required: {0}")]
    LoginRequired(String),
    #[error("The HTML structure changed: {0}")]
    HtmlStructureChanged(String),
    #[error("{0}")]
    Disabled(String),
    #[error("This provider does not support {0}")]
    Unsupported(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("{0}")]
    Other(String),
}

impl ProviderError {
    pub fn from_status(status: u16, body: &str, retry_after: Option<u64>) -> Self {
        let snippet = truncate(body, 240);
        match status {
            401 => Self::AuthRequired(
                "Check your Reddit app credentials / refresh token, then restart.".into(),
            ),
            403 => Self::Forbidden(snippet),
            404 => Self::NotFound(snippet),
            429 => Self::RateLimited { retry_after },
            s if (500..=599).contains(&s) => Self::Server(s, snippet),
            s => Self::Other(format!("Unexpected HTTP {s}: {snippet}")),
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Network(_) | Self::Timeout(_) | Self::RateLimited { .. } | Self::Server(..)
        )
    }

    /// A short title suitable for an error screen/toast.
    pub fn title(&self) -> &'static str {
        match self {
            Self::AuthRequired(_) => "Authentication required",
            Self::Forbidden(_) => "Access denied",
            Self::NotFound(_) => "Not found",
            Self::RateLimited { .. } => "Rate limited",
            Self::Server(..) => "Reddit is having trouble",
            Self::Network(_) | Self::Timeout(_) => "Network problem",
            Self::Parse(_) => "Invalid response",
            Self::LoginRequired(_) => "Login required",
            Self::HtmlStructureChanged(_) => "HTML structure changed",
            Self::Disabled(_) => "Provider disabled",
            Self::Unsupported(_) => "Not supported",
            Self::InvalidInput(_) => "Invalid input",
            Self::Other(_) => "Something went wrong",
        }
    }
}

impl From<url::ParseError> for ProviderError {
    fn from(error: url::ParseError) -> Self {
        Self::InvalidInput(format!("invalid URL: {error}"))
    }
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_http_statuses() {
        assert!(matches!(
            ProviderError::from_status(401, "no", None),
            ProviderError::AuthRequired(_)
        ));
        assert!(matches!(
            ProviderError::from_status(429, "slow down", Some(30)),
            ProviderError::RateLimited {
                retry_after: Some(30)
            }
        ));
        assert!(ProviderError::from_status(503, "down", None).is_retryable());
        assert!(!ProviderError::from_status(403, "no", None).is_retryable());
    }

    #[test]
    fn html_errors_are_not_retryable() {
        let login = ProviderError::LoginRequired("sign in".into());
        let changed = ProviderError::HtmlStructureChanged("no listing".into());
        assert!(!login.is_retryable());
        assert!(!changed.is_retryable());
        assert_eq!(login.title(), "Login required");
        assert_eq!(changed.title(), "HTML structure changed");
    }

    #[test]
    fn rate_limit_message_contains_retry() {
        let err = ProviderError::RateLimited {
            retry_after: Some(12),
        };
        assert!(err.to_string().contains("12s"));
    }
}
