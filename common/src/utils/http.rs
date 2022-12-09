use std::{
    io::Read
};

use reqwest::blocking::get;
use serde_json::Value;

// make a GET request to the target URL and return the response body as JSON
pub fn get_json_from_url(url: String, attempts_remaining: u8) -> Option<Value> {

    let mut res = match get(url.clone()) {
        Ok(res) => res,
        Err(_) => {

            // retry if we have attempts remaining
            if attempts_remaining == 1 {
                return None
            }

            std::thread::sleep(std::time::Duration::from_millis(250));
            return get_json_from_url(url, attempts_remaining - 1)
        }
    };
    let mut body = String::new();
    
    match res.read_to_string(&mut body) {
        Ok(_) => {
            Some(match serde_json::from_str(&body) {
                Ok(json) => json,
                Err(_) => {
                    return None
                }
            })
        },
        Err(_) => {
            return None
        }
    }
}