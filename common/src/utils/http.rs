use reqwest::blocking::Client;
use serde_json::Value;
use std::io::Read;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

/// make a GET request to the target URL and return the response body as JSON
///
/// # Arguments
/// `url` - the URL to make the GET request to
///
/// # Returns
/// `Option<Value>` - the response body as JSON
pub fn get_json_from_url(url: String) -> Option<Value> {
    _get_json_from_url(url, 0, 5)
}

fn _get_json_from_url(url: String, retry_count: u8, retries_remaining: u8) -> Option<Value> {
    let client = match Client::builder()
        .danger_accept_invalid_certs(true)
        .user_agent(APP_USER_AGENT)
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(_) => Client::default(),
    };

    let mut res = match client.get(url.clone()).send() {
        Ok(res) => res,
        Err(_) => {
            if retries_remaining == 0 {
                return None
            }

            // exponential backoff
            let retry_count = retry_count + 1;
            let retries_remaining = retries_remaining - 1;

            let sleep_time = 2u64.pow(retry_count as u32) * 250;
            std::thread::sleep(std::time::Duration::from_millis(sleep_time));
            return _get_json_from_url(url, retry_count, retries_remaining)
        }
    };
    let mut body = String::new();

    match res.read_to_string(&mut body) {
        Ok(_) => Some(match serde_json::from_str(&body) {
            Ok(json) => json,
            Err(_) => return None,
        }),
        Err(_) => None,
    }
}
