use async_trait::async_trait;
use ethers::abi::Token;
use heimdall_cache::{read_cache, store_cache};

use crate::{
    debug_max,
    utils::{http::get_json_from_url, strings::replace_last},
};
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

#[async_trait]
pub trait ResolveSelector {
    async fn resolve(selector: &str) -> Option<Vec<Self>>
    where
        Self: Sized;
}

#[async_trait]
impl ResolveSelector for ResolvedError {
    async fn resolve(selector: &str) -> Option<Vec<Self>> {
        // normalize selector
        let selector = match selector.strip_prefix("0x") {
            Some(selector) => selector,
            None => selector,
        };

        debug_max!("resolving error selector {}", &selector);

        // get cached results
        if let Some(cached_results) =
            read_cache::<Vec<ResolvedError>>(&format!("selector.{selector}"))
        {
            match cached_results.len() {
                0 => return None,
                _ => {
                    debug_max!("found cached results for selector: {}", &selector);
                    return Some(cached_results)
                }
            }
        }

        // get function possibilities from openchain
        let signatures = match get_json_from_url(
            &format!(
                "https://api.openchain.xyz/signature-database/v1/lookup?filter=true&function=0x{}",
                &selector
            ),
            10,
        )
        .await
        .unwrap()
        {
            Some(signatures) => signatures,
            None => return None,
        };

        // convert the serde value into a vec of possible functions
        let results = signatures
            .get("result")?
            .get("function")?
            .get(&format!("0x{selector}"))?
            .as_array()?
            .to_vec();

        debug_max!("found {} possible functions for selector: {}", &results.len(), &selector);

        let mut signature_list: Vec<ResolvedError> = Vec::new();

        for signature in results {
            // get the function text signature and unwrap it into a string
            let text_signature = match signature.get("name") {
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
                inputs: replace_last(function_parts.1, ")", "")
                    .split(',')
                    .map(|input| input.to_string())
                    .collect(),
            });
        }

        // cache the results
        let _ = store_cache(&format!("selector.{selector}"), signature_list.clone(), None)
            .map_err(|e| debug_max!("error storing signatures in cache: {}", e));

        match signature_list.len() {
            0 => None,
            _ => Some(signature_list),
        }
    }
}

#[async_trait]
impl ResolveSelector for ResolvedLog {
    async fn resolve(selector: &str) -> Option<Vec<Self>> {
        // normalize selector
        let selector = match selector.strip_prefix("0x") {
            Some(selector) => selector,
            None => selector,
        };

        debug_max!("resolving event selector {}", &selector);

        // get cached results
        if let Some(cached_results) =
            read_cache::<Vec<ResolvedLog>>(&format!("selector.{selector}"))
        {
            match cached_results.len() {
                0 => return None,
                _ => {
                    debug_max!("found cached results for selector: {}", &selector);
                    return Some(cached_results)
                }
            }
        }

        // get function possibilities from openchain
        let signatures = match get_json_from_url(
            &format!(
                "https://api.openchain.xyz/signature-database/v1/lookup?filter=true&event=0x{}",
                &selector
            ),
            10,
        )
        .await
        .unwrap()
        {
            Some(signatures) => signatures,
            None => return None,
        };

        // convert the serde value into a vec of possible functions
        let results = signatures
            .get("result")?
            .get("event")?
            .get(&format!("0x{selector}"))?
            .as_array()?
            .to_vec();

        debug_max!("found {} possible functions for selector: {}", &results.len(), &selector);

        let mut signature_list: Vec<ResolvedLog> = Vec::new();

        for signature in results {
            // get the function text signature and unwrap it into a string
            let text_signature = match signature.get("name") {
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
                inputs: replace_last(function_parts.1, ")", "")
                    .split(',')
                    .map(|input| input.to_string())
                    .collect(),
            });
        }

        // cache the results
        let _ = store_cache(&format!("selector.{selector}"), signature_list.clone(), None)
            .map_err(|e| debug_max!("error storing signatures in cache: {}", e));

        match signature_list.len() {
            0 => None,
            _ => Some(signature_list),
        }
    }
}

#[async_trait]
impl ResolveSelector for ResolvedFunction {
    async fn resolve(selector: &str) -> Option<Vec<Self>> {
        // normalize selector
        let selector = match selector.strip_prefix("0x") {
            Some(selector) => selector,
            None => selector,
        };

        debug_max!("resolving event selector {}", &selector);

        // get cached results
        if let Some(cached_results) =
            read_cache::<Vec<ResolvedFunction>>(&format!("selector.{selector}"))
        {
            match cached_results.len() {
                0 => return None,
                _ => {
                    debug_max!("found cached results for selector: {}", &selector);
                    return Some(cached_results)
                }
            }
        }

        // get function possibilities from openchain
        let signatures = match get_json_from_url(
            &format!(
                "https://api.openchain.xyz/signature-database/v1/lookup?filter=true&function=0x{}",
                &selector
            ),
            10,
        )
        .await
        .unwrap()
        {
            Some(signatures) => signatures,
            None => return None,
        };

        // convert the serde value into a vec of possible functions
        let results = signatures
            .get("result")?
            .get("function")?
            .get(&format!("0x{selector}"))?
            .as_array()?
            .to_vec();

        debug_max!("found {} possible functions for selector: {}", &results.len(), &selector);

        let mut signature_list: Vec<ResolvedFunction> = Vec::new();

        for signature in results {
            // get the function text signature and unwrap it into a string
            let text_signature = match signature.get("name") {
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
                inputs: replace_last(function_parts.1, ")", "")
                    .split(',')
                    .map(|input| input.to_string())
                    .collect(),
                decoded_inputs: None,
            });
        }

        // cache the results
        let _ = store_cache(&format!("selector.{selector}"), signature_list.clone(), None)
            .map_err(|e| debug_max!("error storing signatures in cache: {}", e));

        match signature_list.len() {
            0 => None,
            _ => Some(signature_list),
        }
    }
}

pub fn score_signature(signature: &str) -> u32 {
    // the score starts at 1000
    let mut score = 1000;

    // remove the length of the signature from the score
    // this will prioritize shorter signatures, which are typically less spammy
    score -= signature.len() as u32;

    // prioritize signatures with less numbers
    score -= (signature.matches(|c: char| c.is_numeric()).count() as u32) * 3;

    score
}

#[cfg(test)]
mod tests {
    use heimdall_cache::delete_cache;

    use crate::ether::signatures::{
        score_signature, ResolveSelector, ResolvedError, ResolvedFunction, ResolvedLog,
    };

    #[tokio::test]
    async fn resolve_function_signature_nominal() {
        let signature = String::from("095ea7b3");
        delete_cache(&format!("selector.{}", &signature));
        let result = ResolvedFunction::resolve(&signature).await;
        assert!(result.is_some());
        assert!(!result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn resolve_error_signature_nominal() {
        let signature = String::from("30cd7471");
        delete_cache(&format!("selector.{}", &signature));
        let result = ResolvedError::resolve(&signature).await;
        assert!(result.is_some());
        assert!(!result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn resolve_event_signature_nominal() {
        let signature =
            String::from("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");
        delete_cache(&format!("selector.{}", &signature));
        let result = ResolvedLog::resolve(&signature).await;
        assert!(result.is_some());
        assert!(!result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn resolve_function_signature_should_return_none_when_cached_results_not_found() {
        let signature = String::from("test_signature_nocache");
        let result = ResolvedFunction::resolve(&signature).await;

        assert_eq!(result, None,)
    }

    #[tokio::test]
    async fn resolve_function_signature_should_return_none_when_json_url_returns_empty_signatures()
    {
        delete_cache(&format!("selector.{}", "test_signature"));
        let signature = String::from("test_signature");
        let result = ResolvedFunction::resolve(&signature).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn resolve_error_signature_should_return_none_when_cached_results_not_found() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedError::resolve(&signature).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn resolve_error_signature_should_return_none_when_json_url_returns_none() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedError::resolve(&signature).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn resolve_error_signature_should_return_none_when_json_url_returns_empty_signatures() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedError::resolve(&signature).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn resolve_event_signature_should_return_none_when_cached_results_not_found() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedLog::resolve(&signature).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn resolve_event_signature_should_return_none_when_json_url_returns_none() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedLog::resolve(&signature).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn resolve_event_signature_should_return_none_when_json_url_returns_empty_signatures() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedLog::resolve(&signature).await;
        assert_eq!(result, None);
    }

    #[test]
    fn score_signature_should_return_correct_score() {
        let signature = String::from("test_signature");
        let score = score_signature(&signature);
        let expected_score = 1000 -
            (signature.len() as u32) -
            (signature.matches(|c: char| c.is_numeric()).count() as u32) * 3;
        assert_eq!(score, expected_score);
    }
}
