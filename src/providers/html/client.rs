//! Shared HTTP transport for HTML providers.

use std::time::Duration;

use reqwest::header::{ACCEPT, CONTENT_LENGTH, CONTENT_TYPE, RETRY_AFTER};
use url::Url;

use crate::error::ProviderError;

const MAX_HTML_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct HtmlClientConfig {
    pub base_url: String,
    pub user_agent: String,
    /// Explicit opt-in from configuration. When false, every fetch fails with
    /// an explanatory error instead of touching the network.
    pub allow_network: bool,
    pub timeout: Duration,
}

impl HtmlClientConfig {
    pub fn new(base_url: impl Into<String>, user_agent: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            user_agent: user_agent.into(),
            allow_network: false,
            timeout: Duration::from_secs(20),
        }
    }
}

pub struct HtmlClient {
    base: Url,
    http: reqwest::Client,
    allow_network: bool,
}

impl HtmlClient {
    pub fn new(config: &HtmlClientConfig) -> Result<Self, ProviderError> {
        let base = Url::parse(
            &crate::config::settings::validate_html_base(&config.base_url)
                .map_err(ProviderError::InvalidInput)?,
        )
        .map_err(|e| ProviderError::InvalidInput(format!("invalid source URL: {e}")))?;
        // The client is constructible even with the network disabled so the
        // app can report a precise "network access is disabled" error. Every
        // fetch goes through `ensure_allowed`.
        let http = reqwest::Client::builder()
            .user_agent(config.user_agent.clone())
            .timeout(config.timeout)
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                // Do not follow a redirect to a different host. This prevents a
                // compromised instance from bouncing a request to an internal
                // address or an unrelated site.
                if attempt.previous().len() >= 3 {
                    return attempt.stop();
                }
                let same_host = match (attempt.url().host_str(), attempt.previous().last()) {
                    (Some(host), Some(previous)) => previous
                        .host_str()
                        .is_some_and(|p| p.eq_ignore_ascii_case(host)),
                    (Some(_), None) => true,
                    _ => false,
                };
                if same_host {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .build()
            .map_err(|e| ProviderError::Network(format!("could not create HTTP client: {e}")))?;
        Ok(Self {
            base,
            http,
            allow_network: config.allow_network,
        })
    }

    pub fn base(&self) -> &Url {
        &self.base
    }

    pub fn ensure_allowed(&self) -> Result<(), ProviderError> {
        if self.allow_network {
            Ok(())
        } else {
            Err(ProviderError::Disabled(
                "HTML network access is disabled. Set provider.html.allow_network = true only after confirming the source's terms and permissions.".into(),
            ))
        }
    }

    pub fn same_origin(&self, url: &Url) -> bool {
        url.scheme() == self.base.scheme()
            && url.host_str().is_some_and(|host| {
                self.base
                    .host_str()
                    .is_some_and(|base| base.eq_ignore_ascii_case(host))
            })
            && url.port_or_known_default() == self.base.port_or_known_default()
    }

    pub async fn fetch(&self, url: &Url) -> Result<FetchedPage, ProviderError> {
        self.ensure_allowed()?;
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(ProviderError::InvalidInput(
                "only http(s) URLs are supported".into(),
            ));
        }
        if !self.same_origin(url) {
            return Err(ProviderError::InvalidInput(
                "refusing to fetch a URL outside the configured source origin".into(),
            ));
        }

        let response = self
            .http
            .get(url.clone())
            .header(ACCEPT, "text/html,application/xhtml+xml;q=0.9,*/*;q=0.1")
            .send()
            .await
            .map_err(map_reqwest_error)?;

        let status = response.status();
        if !status.is_success() {
            let retry = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            let raw_body =
                String::from_utf8_lossy(&response.bytes().await.unwrap_or_default()).into_owned();
            let lower_body = raw_body.to_ascii_lowercase();
            let body = super::parser::plain_text(&raw_body, 240);
            let page_blocked = lower_body.contains("reddit blocked this request")
                || lower_body.contains("access denied")
                || lower_body.contains("captcha")
                || lower_body.contains("verify you are human");
            if page_blocked {
                return Err(ProviderError::Forbidden(format!(
                    "the source reported an access block instead of content: {body}"
                )));
            }
            if lower_body.contains("too many requests") || lower_body.contains("rate limit") {
                return Err(ProviderError::RateLimited { retry_after: retry });
            }
            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(ProviderError::LoginRequired(
                    "the source rejected the request as unauthenticated. HTML mode cannot sign in."
                        .into(),
                ));
            }
            if status == reqwest::StatusCode::FORBIDDEN {
                return Err(ProviderError::Forbidden(
                    "the source denied access. ReddTTY does not bypass access controls or rotate identities."
                        .into(),
                ));
            }
            return Err(ProviderError::from_status(status.as_u16(), &body, retry));
        }

        if let Some(length) = response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<usize>().ok())
            && length > MAX_HTML_BYTES
        {
            return Err(ProviderError::Parse(format!(
                "HTML response is too large ({length} bytes; limit {MAX_HTML_BYTES})"
            )));
        }

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        if content_type.is_empty() {
            return Err(ProviderError::Parse(
                "the source did not declare a content-type; expected text/html".into(),
            ));
        }
        if !content_type.starts_with("text/html")
            && !content_type.starts_with("application/xhtml+xml")
        {
            return Err(ProviderError::Parse(format!(
                "expected an HTML response, got content-type `{content_type}`"
            )));
        }

        let final_url = response.url().clone();
        if !self.same_origin(&final_url) {
            return Err(ProviderError::InvalidInput(
                "the source redirected outside its configured origin".into(),
            ));
        }
        let bytes = response.bytes().await.map_err(map_reqwest_error)?;
        if bytes.len() > MAX_HTML_BYTES {
            return Err(ProviderError::Parse(format!(
                "HTML response is too large ({} bytes; limit {MAX_HTML_BYTES})",
                bytes.len()
            )));
        }
        let body = String::from_utf8_lossy(&bytes).into_owned();
        if body.trim().is_empty() {
            return Err(ProviderError::Parse(
                "the source returned an empty response".into(),
            ));
        }
        Ok(FetchedPage {
            url: final_url,
            body,
        })
    }
}

pub struct FetchedPage {
    pub url: Url,
    pub body: String,
}

fn map_reqwest_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Timeout(20)
    } else if error.is_connect() || error.is_request() || error.is_body() {
        ProviderError::Network(error.to_string())
    } else if error.is_redirect() {
        ProviderError::Network("the source redirected outside its configured origin".into())
    } else {
        ProviderError::Network(error.to_string())
    }
}
