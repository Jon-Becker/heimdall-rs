use ethers::abi::Token;
use heimdall_cache::{read_cache, store_cache};

use crate::utils::{http::get_json_from_url, strings::replace_last};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedFunction {
    pub name: String,
    pub signature: String,
    pub inputs: Vec<String>,
    pub decoded_inputs: Option<Vec<Token>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedError {
    pub name: String,
    pub signature: String,
    pub inputs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedLog {
    pub name: String,
    pub signature: String,
    pub inputs: Vec<String>,
}

pub trait ResolveSelector {
    fn resolve(selector: &str) -> Option<Vec<Self>>
    where
        Self: Sized;
}

impl ResolveSelector for ResolvedError {
    fn resolve(selector: &str) -> Option<Vec<Self>> {
        // get cached results
        if let Some(cached_results) =
            read_cache::<Vec<ResolvedError>>(&format!("selector.{selector}"))
        {
            match cached_results.len() {
                0 => return None,
                _ => return Some(cached_results),
            }
        }

        // get function possibilities from etherface
        let signatures = match get_json_from_url(format!(
            "https://api.etherface.io/v1/signatures/hash/error/{}/1",
            &selector
        )) {
            Some(signatures) => signatures,
            None => return None,
        };

        // convert the serde value into a vec of possible functions
        let results = match signatures.get("items") {
            Some(items) => match items.as_array() {
                Some(items) => items.to_vec(),
                None => return None,
            },
            None => return None,
        };

        let mut signature_list: Vec<ResolvedError> = Vec::new();

        for signature in results {
            // get the function text signature and unwrap it into a string
            let text_signature = match signature.get("text") {
                Some(text_signature) => text_signature.to_string().replace('"', ""),
                None => continue,
            };

            // safely split the text signature into name and inputs
            let function_parts = match text_signature.split_once('(') {
                Some(function_parts) => function_parts,
                None => continue,
            };

            signature_list.push(ResolvedError {
                name: function_parts.0.to_string(),
                signature: text_signature.to_string(),
                inputs: replace_last(function_parts.1.to_string(), ")", "")
                    .split(',')
                    .map(|input| input.to_string())
                    .collect(),
            });
        }

        // cache the results
        store_cache(&format!("selector.{selector}"), signature_list.clone(), None);

        match signature_list.len() {
            0 => None,
            _ => Some(signature_list),
        }
    }
}

impl ResolveSelector for ResolvedLog {
    fn resolve(selector: &str) -> Option<Vec<Self>> {
        // get cached results
        if let Some(cached_results) =
            read_cache::<Vec<ResolvedLog>>(&format!("selector.{selector}"))
        {
            match cached_results.len() {
                0 => return None,
                _ => return Some(cached_results),
            }
        }

        // get function possibilities from etherface
        let signatures = match get_json_from_url(format!(
            "https://api.etherface.io/v1/signatures/hash/event/{}/1",
            &selector
        )) {
            Some(signatures) => signatures,
            None => return None,
        };

        // convert the serde value into a vec of possible functions
        let results = match signatures.get("items") {
            Some(items) => match items.as_array() {
                Some(items) => items.to_vec(),
                None => return None,
            },
            None => return None,
        };

        let mut signature_list: Vec<ResolvedLog> = Vec::new();

        for signature in results {
            // get the function text signature and unwrap it into a string
            let text_signature = match signature.get("text") {
                Some(text_signature) => text_signature.to_string().replace('"', ""),
                None => continue,
            };

            // safely split the text signature into name and inputs
            let function_parts = match text_signature.split_once('(') {
                Some(function_parts) => function_parts,
                None => continue,
            };

            signature_list.push(ResolvedLog {
                name: function_parts.0.to_string(),
                signature: text_signature.to_string(),
                inputs: replace_last(function_parts.1.to_string(), ")", "")
                    .split(',')
                    .map(|input| input.to_string())
                    .collect(),
            });
        }

        // cache the results
        store_cache(&format!("selector.{selector}"), signature_list.clone(), None);

        match signature_list.len() {
            0 => None,
            _ => Some(signature_list),
        }
    }
}

impl ResolveSelector for ResolvedFunction {
    fn resolve(selector: &str) -> Option<Vec<Self>> {
        // get cached results
        if let Some(cached_results) =
            read_cache::<Vec<ResolvedFunction>>(&format!("selector.{selector}"))
        {
            match cached_results.len() {
                0 => return None,
                _ => return Some(cached_results),
            }
        }

        // get function possibilities from etherface
        let signatures = match get_json_from_url(format!(
            "https://api.etherface.io/v1/signatures/hash/function/{}/1",
            &selector
        )) {
            Some(signatures) => signatures,
            None => return None,
        };

        // convert the serde value into a vec of possible functions
        let results = match signatures.get("items") {
            Some(items) => match items.as_array() {
                Some(items) => items.to_vec(),
                None => return None,
            },
            None => return None,
        };

        let mut signature_list: Vec<ResolvedFunction> = Vec::new();

        for signature in results {
            // get the function text signature and unwrap it into a string
            let text_signature = match signature.get("text") {
                Some(text_signature) => text_signature.to_string().replace('"', ""),
                None => continue,
            };

            // safely split the text signature into name and inputs
            let function_parts = match text_signature.split_once('(') {
                Some(function_parts) => function_parts,
                None => continue,
            };

            signature_list.push(ResolvedFunction {
                name: function_parts.0.to_string(),
                signature: text_signature.to_string(),
                inputs: replace_last(function_parts.1.to_string(), ")", "")
                    .split(',')
                    .map(|input| input.to_string())
                    .collect(),
                decoded_inputs: None,
            });
        }

        // cache the results
        store_cache(&format!("selector.{selector}"), signature_list.clone(), None);

        match signature_list.len() {
            0 => None,
            _ => Some(signature_list),
        }
    }
}

pub fn score_signature(signature: &String) -> u32 {
    // the score starts at 1000
    let mut score = 1000;

    // remove the length of the signature from the score
    // this will prioritize shorter signatures, which are typically less spammy
    score -= signature.len() as u32;

    // prioritize signatures with less numbers
    score -= (signature.matches(|c: char| c.is_numeric()).count() as u32) * 3;

    score
}
