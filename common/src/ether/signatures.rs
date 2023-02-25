use ethers::abi::Token;
use heimdall_cache::{store_cache, read_cache};

use crate::utils::{http::get_json_from_url, strings::replace_last};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedFunction {
    pub name: String,
    pub signature: String,
    pub inputs: Vec<String>,
    pub decoded_inputs: Option<Vec<Token>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedError {
    pub name: String,
    pub signature: String,
    pub inputs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedLog {
    pub name: String,
    pub signature: String,
    pub inputs: Vec<String>,
}

pub fn resolve_function_signature(signature: &String) -> Option<Vec<ResolvedFunction>> {

    // get cached results
    match read_cache::<Vec<ResolvedFunction>>(&format!("selector.{}", signature)) {
        Some(cached_results) => {
            match cached_results.len() {
                0 => return None,
                _ => return Some(cached_results)
            }
        },
        None => {}
    };

    // get function possibilities from 4byte
    let signatures = match get_json_from_url(format!("https://api.openchain.xyz/signature-database/v1/lookup?function=0x{}", &signature), 3) {
        Some(signatures) => signatures,
        None => return None
    };

    // convert the serde value into a vec of possible functions
    // AAAHAHHHHHH IM MATCHING
    let results = match signatures.get("result") {
        Some(result) => match result.get("function") {
            Some(function) => match function.get(format!("0x{signature}")) {
                Some(functions) => match functions.as_array() {
                    Some(functions) => functions.to_vec(),
                    None => return None
                },
                None => return None
            },
            None => return None
        },
        None => return None
    };

    let mut signature_list: Vec<ResolvedFunction> = Vec::new();

    for signature in results {

        // get the function text signature and unwrap it into a string
        let text_signature = match signature.get("name") {
            Some(text_signature) => text_signature.to_string().replace("\"", ""),
            None => continue
        };
        
        // safely split the text signature into name and inputs
        let function_parts = match text_signature.split_once("(") {
            Some(function_parts) => function_parts,
            None => continue
        };

        signature_list.push(ResolvedFunction {
            name: function_parts.0.to_string(),
            signature: text_signature.to_string(),
            inputs: replace_last(function_parts.1.to_string(), ")", "").split(",").map(|input| input.to_string()).collect(),
            decoded_inputs: None
        });

    }

    // cache the results
    store_cache(&format!("selector.{}", signature), signature_list.clone(), None);

    return match signature_list.len() {
        0 => None,
        _ => Some(signature_list)
    }

}

pub fn resolve_error_signature(signature: &String) -> Option<Vec<ResolvedError>> {

    // get cached results
    match read_cache::<Vec<ResolvedError>>(&format!("selector.{}", signature)) {
        Some(cached_results) => {
            match cached_results.len() {
                0 => return None,
                _ => return Some(cached_results)
            }
        },
        None => {}
    };

    // get function possibilities from 4byte
    let signatures = match get_json_from_url(format!("https://api.openchain.xyz/signature-database/v1/lookup?function=0x{}", &signature), 3) {
        Some(signatures) => signatures,
        None => return None
    };

    // convert the serde value into a vec of possible functions
    // AAAHAHHHHHH IM MATCHING
    let results = match signatures.get("result") {
        Some(result) => match result.get("function") {
            Some(function) => match function.get(format!("0x{signature}")) {
                Some(functions) => match functions.as_array() {
                    Some(functions) => functions.to_vec(),
                    None => return None
                },
                None => return None
            },
            None => return None
        },
        None => return None
    };

    let mut signature_list: Vec<ResolvedError> = Vec::new();

    for signature in results {

        // get the function text signature and unwrap it into a string
        let text_signature = match signature.get("name") {
            Some(text_signature) => text_signature.to_string().replace("\"", ""),
            None => continue
        };
        
        // safely split the text signature into name and inputs
        let function_parts = match text_signature.split_once("(") {
            Some(function_parts) => function_parts,
            None => continue
        };

        signature_list.push(ResolvedError {
            name: function_parts.0.to_string(),
            signature: text_signature.to_string(),
            inputs: replace_last(function_parts.1.to_string(), ")", "").split(",").map(|input| input.to_string()).collect()
        });

    }

    // cache the results
    store_cache(&format!("selector.{}", signature), signature_list.clone(), None);

    return match signature_list.len() {
        0 => None,
        _ => Some(signature_list)
    }

}


pub fn resolve_event_signature(signature: &String) -> Option<Vec<ResolvedLog>> {

    // get cached results
    match read_cache::<Vec<ResolvedLog>>(&format!("selector.{}", signature)) {
        Some(cached_results) => {
            match cached_results.len() {
                0 => return None,
                _ => return Some(cached_results)
            }
        },
        None => {}
    };

    // get function possibilities from 4byte
    let signatures = match get_json_from_url(format!("https://api.openchain.xyz/signature-database/v1/lookup?event=0x{}", &signature), 3) {
        Some(signatures) => signatures,
        None => return None
    };

    // convert the serde value into a vec of possible functions
    // AAAHAHHHHHH IM MATCHING
    let results = match signatures.get("result") {
        Some(result) => match result.get("event") {
            Some(event) => match event.get(format!("0x{signature}")) {
                Some(events) => match events.as_array() {
                    Some(events) => events.to_vec(),
                    None => return None
                },
                None => return None
            },
            None => return None
        },
        None => return None
    };

    let mut signature_list: Vec<ResolvedLog> = Vec::new();

    for signature in results {

        // get the function text signature and unwrap it into a string
        let text_signature = match signature.get("name") {
            Some(text_signature) => text_signature.to_string().replace("\"", ""),
            None => continue
        };
        
        // safely split the text signature into name and inputs
        let function_parts = match text_signature.split_once("(") {
            Some(function_parts) => function_parts,
            None => continue
        };

        signature_list.push(ResolvedLog {
            name: function_parts.0.to_string(),
            signature: text_signature.to_string(),
            inputs: replace_last(function_parts.1.to_string(), ")", "").split(",").map(|input| input.to_string()).collect()
        });

    }
    
    // cache the results
    store_cache(&format!("selector.{}", signature), signature_list.clone(), None);

    return match signature_list.len() {
        0 => None,
        _ => Some(signature_list)
    }

}