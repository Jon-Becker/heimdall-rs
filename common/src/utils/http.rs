use crate::utils::io::logging::Logger;
use async_recursion::async_recursion;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep as async_sleep;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

/// Make a GET request to the target URL and return the response body as JSON
///
/// # Arguments
/// `url` - the URL to make the GET request to
/// `timeout` - timeout duration in seconds
///
/// # Returns
/// `Result<Option<Value>, reqwest::Error>` - the response body as JSON
pub async fn get_json_from_url(url: &str, timeout: u64) -> Result<Option<Value>, reqwest::Error> {
    _get_json_from_url(url, 0, 5, timeout).await
}

#[async_recursion]
async fn _get_json_from_url(
    url: &str,
    retry_count: u8,
    retries_remaining: u8,
    timeout: u64,
) -> Result<Option<Value>, reqwest::Error> {
    // get a new logger
    let logger = Logger::default();

    logger.debug_max(&format!("GET {}", &url));

    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .user_agent(APP_USER_AGENT)
        .timeout(Duration::from_secs(timeout))
        .build()?;

    let res = match client.get(url).send().await {
        Ok(res) => {
            logger.debug_max(&format!("GET {}: {:?}", &url, &res));
            res
        }
        Err(e) => {
            logger.debug_max(&format!("GET {}: {:?}", &url, &e));
            if retries_remaining == 0 {
                return Ok(None)
            }

            // exponential backoff
            let retry_count = retry_count + 1;
            let retries_remaining = retries_remaining - 1;
            let sleep_time = 2u64.pow(retry_count as u32) * 250;
            async_sleep(Duration::from_millis(sleep_time)).await;
            return _get_json_from_url(url, retry_count, retries_remaining, timeout).await
        }
    };
    let body = res.text().await?;

    match serde_json::from_str(&body) {
        Ok(json) => Ok(Some(json)),
        Err(_) => Ok(None),
    }
}
