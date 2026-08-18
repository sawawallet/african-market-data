//! Shared HTTP client: timeouts, bounded retry, and a user agent that
//! identifies us honestly to the sources we depend on.

use std::time::Duration;

use serde::de::DeserializeOwned;

const USER_AGENT: &str = concat!(
    "african-market-data/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/sawawallet/african-market-data)"
);

#[derive(Debug, Clone)]
pub struct HttpClient {
    inner: reqwest::Client,
    retries: u32,
}

impl HttpClient {
    pub fn new() -> Result<Self, reqwest::Error> {
        Ok(HttpClient {
            inner: reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .timeout(Duration::from_secs(10))
                .connect_timeout(Duration::from_secs(5))
                .build()?,
            retries: 2,
        })
    }

    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    /// GET and deserialize, retrying only what is worth retrying.
    ///
    /// 4xx other than 429 will not improve on a second attempt, so they fail
    /// fast. This matters for sources like kwayisi that rate-limit: hammering a
    /// free service we depend on is both rude and counterproductive.
    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, HttpError> {
        let mut last: Option<HttpError> = None;
        for attempt in 0..=self.retries {
            if attempt > 0 {
                let backoff = Duration::from_millis(250 * (1 << (attempt - 1)).min(8));
                tokio::time::sleep(backoff).await;
            }
            match self.inner.get(url).send().await {
                Ok(res) => {
                    let status = res.status();
                    if status.is_success() {
                        return res
                            .json::<T>()
                            .await
                            .map_err(|e| HttpError::Decode(e.to_string()));
                    }
                    let retryable = status.as_u16() == 429 || status.is_server_error();
                    let err = HttpError::Status {
                        status: status.as_u16(),
                        url: url.to_owned(),
                    };
                    if !retryable {
                        return Err(err);
                    }
                    tracing::warn!(%url, status = status.as_u16(), attempt, "retrying");
                    last = Some(err);
                }
                Err(e) => {
                    if e.is_builder() {
                        return Err(HttpError::Transport(e.to_string()));
                    }
                    tracing::warn!(%url, attempt, error = %e, "retrying");
                    last = Some(HttpError::Transport(e.to_string()));
                }
            }
        }
        Err(last.unwrap_or_else(|| HttpError::Transport("no attempts made".into())))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("HTTP {status} for {url}")]
    Status { status: u16, url: String },
    #[error("transport error: {0}")]
    Transport(String),
    #[error("could not decode response: {0}")]
    Decode(String),
}
