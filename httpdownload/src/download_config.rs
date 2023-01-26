use std::time::Duration;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::header;
use crate::constants::DEFAULT_USER_AGENT;

/**
Holds the http configuration for the Download
 */
#[derive(Debug, Clone)]
pub struct DownloadConfig {
    /**
     * Timeout parameter for requests
     */
    pub timeout: Duration,
    /**
     * Request headers for the Download
     */
    pub headers: HeaderMap,
    pub chunk_size: Option<usize>,
}

impl DownloadConfig {
    /**
    Creates a default set of settings:
    * headers: { user-agent: "ludownloader" }
    * timeout: 30s
     */
    pub fn default() -> Self {
        let mut config = DownloadConfig {
            timeout: Duration::from_secs(60),
            headers: HeaderMap::new(),
            chunk_size: None,
        };
        config.headers.insert(
            header::USER_AGENT,
            HeaderValue::from_str(DEFAULT_USER_AGENT).unwrap(),
        );
        config
    }
}
