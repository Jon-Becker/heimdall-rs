use async_trait::async_trait;
use ethers::abi::Token;

use heimdall_cache::{read_cache, store_cache};
use tracing::trace;

use crate::{
    error::Error,
    utils::{
        http::get_json_from_url,
        io::{logging::TraceFactory, types::display},
        strings::replace_last,
    },
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
    async fn resolve(selector: &str) -> Result<Option<Vec<Self>>, Error>
    where
        Self: Sized;
}

#[async_trait]
impl ResolveSelector for ResolvedError {
    async fn resolve(selector: &str) -> Result<Option<Vec<Self>>, Error> {
        // normalize selector
        let selector = match selector.strip_prefix("0x") {
            Some(selector) => selector,
            None => selector,
        };

        trace!("resolving error selector {}", &selector);

        // get cached results
        if let Some(cached_results) =
            read_cache::<Vec<ResolvedError>>(&format!("selector.{selector}"))
                .map_err(|e| Error::Generic(format!("error reading cache: {}", e)))?
        {
            match cached_results.len() {
                0 => return Ok(None),
                _ => {
                    trace!("found cached results for selector: {}", &selector);
                    return Ok(Some(cached_results));
                }
            }
        }

        // get function possibilities from openchain
        let signatures = match get_json_from_url(
            &format!(
                "https://api.openchain.xyz/signature-database/v1/lookup?filter=false&function=0x{}",
                &selector
            ),
            10,
        )
        .await
        .map_err(|e| Error::Generic(format!("error fetching signatures from openchain: {}", e)))?
        {
            Some(signatures) => signatures,
            None => return Ok(None),
        };

        // convert the serde value into a vec of possible functions
        let results = signatures
            .get("result")
            .and_then(|result| result.get("function"))
            .and_then(|function| function.get(format!("0x{selector}")))
            .and_then(|item| item.as_array())
            .map(|array| array.to_vec())
            .ok_or_else(|| Error::Generic("error parsing signatures from openchain".to_string()))?;

        trace!("found {} possible functions for selector: {}", &results.len(), &selector);

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
            .map_err(|e| trace!("error storing signatures in cache: {}", e));

        Ok(match signature_list.len() {
            0 => None,
            _ => Some(signature_list),
        })
    }
}

#[async_trait]
impl ResolveSelector for ResolvedLog {
    async fn resolve(selector: &str) -> Result<Option<Vec<Self>>, Error> {
        // normalize selector
        let selector = match selector.strip_prefix("0x") {
            Some(selector) => selector,
            None => selector,
        };

        trace!("resolving event selector {}", &selector);

        // get cached results
        if let Some(cached_results) =
            read_cache::<Vec<ResolvedLog>>(&format!("selector.{selector}"))
                .map_err(|e| Error::Generic(format!("error reading cache: {}", e)))?
        {
            match cached_results.len() {
                0 => return Ok(None),
                _ => {
                    trace!("found cached results for selector: {}", &selector);
                    return Ok(Some(cached_results));
                }
            }
        }

        // get function possibilities from openchain
        let signatures = match get_json_from_url(
            &format!(
                "https://api.openchain.xyz/signature-database/v1/lookup?filter=false&event=0x{}",
                &selector
            ),
            10,
        )
        .await
        .map_err(|e| Error::Generic(format!("error fetching signatures from openchain: {}", e)))?
        {
            Some(signatures) => signatures,
            None => return Ok(None),
        };

        // convert the serde value into a vec of possible functions
        let results = signatures
            .get("result")
            .and_then(|result| result.get("event"))
            .and_then(|function| function.get(format!("0x{selector}")))
            .and_then(|item| item.as_array())
            .map(|array| array.to_vec())
            .ok_or_else(|| Error::Generic("error parsing signatures from openchain".to_string()))?;

        trace!("found {} possible functions for selector: {}", &results.len(), &selector);

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
            .map_err(|e| trace!("error storing signatures in cache: {}", e));

        Ok(match signature_list.len() {
            0 => None,
            _ => Some(signature_list),
        })
    }
}

#[async_trait]
impl ResolveSelector for ResolvedFunction {
    async fn resolve(selector: &str) -> Result<Option<Vec<Self>>, Error> {
        // normalize selector
        let selector = match selector.strip_prefix("0x") {
            Some(selector) => selector,
            None => selector,
        };

        trace!("resolving event selector {}", &selector);

        // get cached results
        if let Some(cached_results) =
            read_cache::<Vec<ResolvedFunction>>(&format!("selector.{selector}"))
                .map_err(|e| Error::Generic(format!("error reading cache: {}", e)))?
        {
            match cached_results.len() {
                0 => return Ok(None),
                _ => {
                    trace!("found cached results for selector: {}", &selector);
                    return Ok(Some(cached_results));
                }
            }
        }

        // get function possibilities from openchain
        let signatures = match get_json_from_url(
            &format!(
                "https://api.openchain.xyz/signature-database/v1/lookup?filter=false&function=0x{}",
                &selector
            ),
            10,
        )
        .await
        .map_err(|e| Error::Generic(format!("error fetching signatures from openchain: {}", e)))?
        {
            Some(signatures) => signatures,
            None => return Ok(None),
        };

        // convert the serde value into a vec of possible functions
        let results = signatures
            .get("result")
            .and_then(|result| result.get("function"))
            .and_then(|function| function.get(format!("0x{selector}")))
            .and_then(|item| item.as_array())
            .map(|array| array.to_vec())
            .ok_or_else(|| Error::Generic("error parsing signatures from openchain".to_string()))?;

        trace!("found {} possible functions for selector: {}", &results.len(), &selector);

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
            .map_err(|e| trace!("error storing signatures in cache: {}", e));

        Ok(match signature_list.len() {
            0 => None,
            _ => Some(signature_list),
        })
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

/// trait impls
/// trait impls
/// trait impls

impl TryFrom<&ResolvedFunction> for TraceFactory {
    // eyre
    type Error = eyre::Report;

    fn try_from(function: &ResolvedFunction) -> Result<Self, Self::Error> {
        let mut trace = TraceFactory::default();
        let decode_call = trace.add_call(
            0,
            line!(),
            "heimdall".to_string(),
            "decode".to_string(),
            vec![],
            "()".to_string(),
        );
        trace.br(decode_call);
        trace.add_message(decode_call, line!(), vec![format!("signature: {}", function.signature)]);
        trace.br(decode_call);

        // build inputs
        for (i, input) in function.decoded_inputs.as_ref().unwrap_or(&Vec::new()).iter().enumerate()
        {
            let mut decoded_inputs_as_message = display(vec![input.to_owned()], "           ");
            if decoded_inputs_as_message.is_empty() {
                break;
            }

            if i == 0 {
                decoded_inputs_as_message[0] = format!(
                    "input {}:{}{}",
                    i,
                    " ".repeat(4 - i.to_string().len()),
                    decoded_inputs_as_message[0].replacen("           ", "", 1)
                )
            } else {
                decoded_inputs_as_message[0] = format!(
                    "      {}:{}{}",
                    i,
                    " ".repeat(4 - i.to_string().len()),
                    decoded_inputs_as_message[0].replacen("           ", "", 1)
                )
            }

            // add to trace and decoded string
            trace.add_message(decode_call, 1, decoded_inputs_as_message.clone());
        }

        Ok(trace)
    }
}

/// tests
/// tests
/// tests

#[cfg(test)]
mod tests {
    use heimdall_cache::delete_cache;

    use crate::ether::signatures::{
        score_signature, ResolveSelector, ResolvedError, ResolvedFunction, ResolvedLog,
    };

    #[tokio::test]
    async fn resolve_function_signature_nominal() {
        let signature = String::from("095ea7b3");
        let _ = delete_cache(&format!("selector.{}", &signature));
        let result = ResolvedFunction::resolve(&signature)
            .await
            .expect("failed to resolve signature")
            .expect("failed to resolve signature");
        assert!(!result.is_empty());
    }

    #[tokio::test]
    async fn resolve_error_signature_nominal() {
        let signature = String::from("30cd7471");
        let _ = delete_cache(&format!("selector.{}", &signature));
        let result = ResolvedError::resolve(&signature)
            .await
            .expect("failed to resolve signature")
            .expect("failed to resolve signature");
        assert!(!result.is_empty());
    }

    #[tokio::test]
    async fn resolve_event_signature_nominal() {
        let signature =
            String::from("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");
        let _ = delete_cache(&format!("selector.{}", &signature));
        let result = ResolvedLog::resolve(&signature)
            .await
            .expect("failed to resolve signature")
            .expect("failed to resolve signature");
        assert!(!result.is_empty());
    }

    #[tokio::test]
    async fn resolve_function_signature_should_return_none_when_cached_results_not_found() {
        let signature = String::from("test_signature_nocache");
        let result = ResolvedFunction::resolve(&signature).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn resolve_function_signature_should_return_none_when_json_url_returns_empty_signatures()
    {
        let _ = delete_cache(&format!("selector.{}", "test_signature"));
        let signature = String::from("test_signature");
        let result = ResolvedFunction::resolve(&signature).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn resolve_error_signature_should_return_none_when_cached_results_not_found() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedError::resolve(&signature).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn resolve_error_signature_should_return_none_when_json_url_returns_none() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedError::resolve(&signature).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn resolve_error_signature_should_return_none_when_json_url_returns_empty_signatures() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedError::resolve(&signature).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn resolve_event_signature_should_return_none_when_cached_results_not_found() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedLog::resolve(&signature).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn resolve_event_signature_should_return_none_when_json_url_returns_none() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedLog::resolve(&signature).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn resolve_event_signature_should_return_none_when_json_url_returns_empty_signatures() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedLog::resolve(&signature).await;
        assert!(result.is_err());
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
