use serde_json::Value;
use std::io::Read;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

// make a GET request to the target URL and return the response body as JSON
pub fn get_json_from_url(url: String, attempts_remaining: u8) -> Option<Value> {
    let client = reqwest::blocking::Client::builder().user_agent(APP_USER_AGENT).build().unwrap();

    let mut res = match client.get(url.clone()).send() {
        Ok(res) => res,
        Err(_) => {
            // retry if we have attempts remaining
            let attempts_remaining = attempts_remaining - 1;
            if attempts_remaining == 0 {
                return None;
            }

            std::thread::sleep(std::time::Duration::from_millis(500));
            return get_json_from_url(url, attempts_remaining);
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
