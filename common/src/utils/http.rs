use std::sync::Arc;
use lazy_static::lazy_static;
use crate::debug_max;
use async_recursion::async_recursion;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep as async_sleep;
use tokio::sync::Mutex;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

//Use a single client in case Heimdall is used across threads
lazy_static! {
    static ref HTTP_CLIENT: Arc<Mutex<Client>> = Arc::new(Mutex::new(Client::builder()
        .user_agent(APP_USER_AGENT)
        // .danger_accept_invalid_certs(true) // Be cautious with this setting
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap()));
}
/// Make a GET request to the target URL and return the response body as JSON
///
/// ```no_run
/// use heimdall_common::utils::http::get_json_from_url;
///
/// let url = "https://example.com";
/// let timeout = 5;
/// // get_json_from_url(url, timeout).await;
/// ```
pub async fn get_json_from_url(url: &str, timeout: u64) -> Result<Option<Value>, reqwest::Error> {
    _get_json_from_url(url, 0, 5, timeout).await
}

#[async_recursion]
/// Internal function for making a GET request to the target URL and returning the response body as
/// JSON
async fn _get_json_from_url(
    url: &str,
    retry_count: u8,
    retries_remaining: u8,
    timeout: u64,
) -> Result<Option<Value>, reqwest::Error> {
    debug_max!("GET {}", &url);

    let client = HTTP_CLIENT.lock().await;
    let res = match client.get(url).send().await {
        Ok(res) => {
            debug_max!("GET {}: {:?}", &url, &res);
            res
        }
        Err(e) => {
            debug_max!("GET {}: {:?}", &url, &e);
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
