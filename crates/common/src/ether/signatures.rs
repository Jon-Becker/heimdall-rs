//! This module contains the logic for resolving signatures from
//! 4-byte function selector or a 32-byte event selector.

use std::path::PathBuf;

use alloy_dyn_abi::{DynSolType, DynSolValue};
use alloy_json_abi::JsonAbi;
use async_trait::async_trait;

use crate::{
    ether::types::parse_function_parameters,
    utils::{
        http::get_json_from_url,
        io::{logging::TraceFactory, types::display},
        strings::replace_last,
    },
};
use eyre::{OptionExt, Result};
use heimdall_cache::{store_cache, with_cache};
use serde::{Deserialize, Serialize};
use tracing::{debug, trace};

use super::types::DynSolValueExt;

/// A resolved function signature. May contain decoded inputs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedFunction {
    /// The name of the function. For example, `transfer`.
    pub name: String,
    /// The function signature. For example, `transfer(address,uint256)`.
    pub signature: String,
    /// The inputs of the function. For example, `["address", "uint256"]`.
    pub inputs: Vec<String>,
    /// The decoded inputs of the function. For example, `[DynSolValue::Address("0x1234"),
    /// DynSolValue::Uint(123)]`.
    #[serde(skip)]
    pub decoded_inputs: Option<Vec<DynSolValue>>,
}

impl ResolvedFunction {
    /// Returns the inputs of the function as a vector of [`DynSolType`]s.
    pub fn inputs(&self) -> Vec<DynSolType> {
        parse_function_parameters(&self.signature).expect("invalid signature")
    }

    /// A helper function to convert the struct into a JSON string.
    /// We use this because `decoded_inputs` cannot be serialized by serde.
    pub fn to_json(&self) -> Result<String> {
        Ok(format!(
            r#"{{
  "name": "{}",
  "signature": "{}",
  "inputs": {},
  "decoded_inputs": [{}]
}}"#,
            &self.name,
            &self.signature,
            serde_json::to_string(&self.inputs)?,
            if let Some(decoded_inputs) = &self.decoded_inputs {
                decoded_inputs
                    .iter()
                    .map(|input| input.serialize().to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            } else {
                "".to_string()
            }
        ))
    }
}

/// A resolved error signature.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedError {
    /// The name of the error. For example, `revert`.
    pub name: String,
    /// The error signature. For example, `revert(string)`.
    pub signature: String,
    /// The inputs of the error. For example, `["string"]`.
    pub inputs: Vec<String>,
}

impl ResolvedError {
    /// Returns the inputs of the error as a vector of [`DynSolType`]s.
    pub fn inputs(&self) -> Vec<DynSolType> {
        parse_function_parameters(&self.signature).expect("invalid signature")
    }
}
/// A resolved log signature.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedLog {
    /// The name of the log. For example, `Transfer`.
    pub name: String,
    /// The log signature. For example, `Transfer(address,address,uint256)`.
    pub signature: String,
    /// The inputs of the log. For example, `["address", "address", "uint256"]`.
    pub inputs: Vec<String>,
}

impl ResolvedLog {
    /// Returns the inputs of the log as a vector of [`DynSolType`]s.
    pub fn inputs(&self) -> Vec<DynSolType> {
        parse_function_parameters(&self.signature).expect("invalid signature")
    }
}
/// A trait for resolving a selector into a vector of [`ResolvedFunction`]s, [`ResolvedError`]s, or
#[async_trait]
pub trait ResolveSelector {
    /// Resolves a selector into a vector of [`ResolvedFunction`]s, [`ResolvedError`]s, or
    /// [`ResolvedLog`]s.
    async fn resolve(selector: &str) -> Result<Option<Vec<Self>>>
    where
        Self: Sized;
}

#[async_trait]
impl ResolveSelector for ResolvedError {
    async fn resolve(selector: &str) -> Result<Option<Vec<Self>>> {
        with_cache(&format!("selector.{selector}"), || async {
            // normalize selector
            let selector = match selector.strip_prefix("0x") {
                Some(selector) => selector,
                None => selector,
            };

            trace!("resolving error selector {}", &selector);

            // get function possibilities from openchain
            let signatures = match get_json_from_url(
                &format!(
                    "https://api.openchain.xyz/signature-database/v1/lookup?filter=false&function=0x{}",
                    &selector
                ),
                10,
            )
            .await?
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
                .ok_or_eyre("error parsing signatures from openchain")?;

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

            Ok(match signature_list.len() {
                0 => None,
                _ => Some(signature_list),
            })
        })
        .await
    }
}

#[async_trait]
impl ResolveSelector for ResolvedLog {
    async fn resolve(selector: &str) -> Result<Option<Vec<Self>>> {
        with_cache(&format!("selector.{selector}"), || async {
            // normalize selector
            let selector = match selector.strip_prefix("0x") {
                Some(selector) => selector,
                None => selector,
            };

            trace!("resolving event selector {}", &selector);

            // get function possibilities from openchain
            let signatures = match get_json_from_url(
                &format!(
                "https://api.openchain.xyz/signature-database/v1/lookup?filter=false&event=0x{}",
                &selector
            ),
                10,
            )
            .await?
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
                .ok_or_eyre("error parsing signatures from openchain")?;

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

            Ok(match signature_list.len() {
                0 => None,
                _ => Some(signature_list),
            })
        })
        .await
    }
}

#[async_trait]
impl ResolveSelector for ResolvedFunction {
    async fn resolve(selector: &str) -> Result<Option<Vec<Self>>> {
        with_cache(&format!("selector.{selector}"), || async {
            // normalize selector
            let selector = match selector.strip_prefix("0x") {
                Some(selector) => selector,
                None => selector,
            };

            trace!("resolving function selector {}", &selector);

            // get function possibilities from openchain
            let signatures = match get_json_from_url(
                &format!(
                "https://api.openchain.xyz/signature-database/v1/lookup?filter=false&function=0x{}",
                &selector
            ),
                10,
            )
            .await?
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
                .ok_or_eyre("error parsing signatures from openchain")?;

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

            Ok(match signature_list.len() {
                0 => None,
                _ => Some(signature_list),
            })
        })
        .await
    }
}

/// Given the path to an ABI file, parses all [`ResolvedFunction`]s, [`ResolvedError`]s, and
/// [`ResolvedLog`]s from the ABI and saves them to the cache.
pub fn cache_signatures_from_abi(path: PathBuf) -> Result<()> {
    let abi = std::fs::read_to_string(&path)?;
    let json_abi = JsonAbi::from_json_str(&abi)?;

    debug!("caching signatures from abi: {}", path.display());

    json_abi.functions().for_each(|function| {
        let selector = function.selector().to_string().trim_start_matches("0x").to_string();
        let inputs: Vec<String> = function.inputs.iter().map(|input| input.ty.clone()).collect();

        let resolved_function = ResolvedFunction {
            name: function.name.clone(),
            signature: function.signature(),
            inputs,
            decoded_inputs: None,
        };

        store_cache(&format!("selector.{selector}"), Some(vec![resolved_function]), None).ok();
    });
    json_abi.events().for_each(|event| {
        let selector = event.selector().to_string().trim_start_matches("0x").to_string();
        let inputs: Vec<String> = event.inputs.iter().map(|input| input.ty.clone()).collect();

        let resolved_log =
            ResolvedLog { name: event.name.clone(), signature: event.signature(), inputs };

        store_cache(&format!("selector.{selector}"), Some(vec![resolved_log]), None).ok();
    });
    json_abi.errors().for_each(|error| {
        let selector = error.selector().to_string().trim_start_matches("0x").to_string();
        let inputs: Vec<String> = error.inputs.iter().map(|input| input.ty.clone()).collect();

        let resolved_error =
            ResolvedError { name: error.name.clone(), signature: error.signature(), inputs };

        store_cache(&format!("selector.{selector}"), Some(vec![resolved_error]), None).ok();
    });

    debug!(
        "cached {} functions, {} logs, and {} errors from provided abi",
        json_abi.functions().count(),
        json_abi.events().count(),
        json_abi.errors().count(),
    );

    Ok(())
}

/// Heuristic to score a function signature based on its spamminess.
pub fn score_signature(signature: &str, num_words: Option<usize>) -> u32 {
    // the score starts at 1000
    let mut score = 1000;

    // remove the length of the signature from the score
    // this will prioritize shorter signatures, which are typically less spammy
    score -= signature.len() as u32;

    // prioritize signatures with less numbers
    score -= (signature.split('(').next().unwrap_or("").matches(|c: char| c.is_numeric()).count()
        as u32) *
        3;

    // prioritize signatures with parameters
    let num_params = signature.matches(',').count() + 1;
    score += num_params as u32 * 10;

    // count the number of parameters in the signature, if enabled
    if let Some(num_words) = num_words {
        let num_dyn_params = signature.matches("bytes").count() +
            signature.matches("string").count() +
            signature.matches('[').count();
        let num_static_params = num_params - num_dyn_params;

        // reduce the score if the signature has less static parameters than there are words in the
        // calldata
        if num_static_params < num_words {
            score -= (num_words - num_static_params) as u32 * 10;
        }
    }

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
            trace.add_message(decode_call, 1, decoded_inputs_as_message);
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
        let score = score_signature(&signature, None);
        assert_eq!(score, 996);
    }
}
