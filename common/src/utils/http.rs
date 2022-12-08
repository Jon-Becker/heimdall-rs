use std::{
    io::Read
};

use reqwest::blocking::get;
use serde_json::Value;

// make a GET request to the target URL and return the response body as JSON
pub fn get_json_from_url(url: String) -> Option<Value> {

    let mut res = match get(url.clone()) {
        Ok(res) => res,
        Err(_) => {

            // wait 1 second and try again
            std::thread::sleep(std::time::Duration::from_secs(1));
            return get_json_from_url(url)
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