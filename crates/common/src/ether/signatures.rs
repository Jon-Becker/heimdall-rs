//! This module contains the logic for resolving signatures from
//! 4-byte function selector or a 32-byte event selector.

use std::path::PathBuf;

use alloy_dyn_abi::{DynSolType, DynSolValue};
use alloy_json_abi::JsonAbi;
use async_trait::async_trait;

use crate::{
    ether::types::{
        dyn_sol_types_to_strings, inputs_to_abi_format, parse_function_parameters, AbiInput,
    },
    utils::{
        http::get_json_from_url,
        io::{logging::TraceFactory, types::display},
    },
};
use eyre::{OptionExt, Result};
use heimdall_cache::{store_cache, with_cache};
use serde::{
    ser::{SerializeMap, SerializeStruct, Serializer},
    Deserialize, Deserializer, Serialize,
};
use tracing::{debug, trace};

use super::types::DynSolValueExt;

fn deserialize_inputs<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    if !deserializer.is_human_readable() {
        return Vec::<String>::deserialize(deserializer);
    }
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Input {
        Type(String),
        Abi(AbiInput),
    }
    Ok(Vec::<Input>::deserialize(deserializer)?
        .into_iter()
        .map(|input| match input {
            Input::Type(value) => value,
            Input::Abi(value) => value.type_name,
        })
        .collect())
}

/// A resolved function signature. May contain decoded inputs.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ResolvedFunction {
    /// The name of the function. For example, `transfer`.
    pub name: String,
    /// The function signature. For example, `transfer(address,uint256)`.
    pub signature: String,
    /// The inputs of the function. For example, `["address", "uint256"]`.
    #[serde(deserialize_with = "deserialize_inputs")]
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
            serde_json::to_string(&inputs_to_abi_format(&self.inputs))?,
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

impl Serialize for ResolvedFunction {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer, {
        if !serializer.is_human_readable() {
            let mut value = serializer.serialize_struct("ResolvedFunction", 3)?;
            value.serialize_field("name", &self.name)?;
            value.serialize_field("signature", &self.signature)?;
            value.serialize_field("inputs", &self.inputs)?;
            return value.end();
        }
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("name", &self.name)?;
        map.serialize_entry("signature", &self.signature)?;
        map.serialize_entry("inputs", &inputs_to_abi_format(&self.inputs))?;
        map.end()
    }
}

/// A resolved error signature.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ResolvedError {
    /// The name of the error. For example, `revert`.
    pub name: String,
    /// The error signature. For example, `revert(string)`.
    pub signature: String,
    /// The inputs of the error. For example, `["string"]`.
    #[serde(deserialize_with = "deserialize_inputs")]
    pub inputs: Vec<String>,
}

impl ResolvedError {
    /// Returns the inputs of the error as a vector of [`DynSolType`]s.
    pub fn inputs(&self) -> Vec<DynSolType> {
        parse_function_parameters(&self.signature).expect("invalid signature")
    }
}

impl Serialize for ResolvedError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer, {
        if !serializer.is_human_readable() {
            let mut value = serializer.serialize_struct("ResolvedError", 3)?;
            value.serialize_field("name", &self.name)?;
            value.serialize_field("signature", &self.signature)?;
            value.serialize_field("inputs", &self.inputs)?;
            return value.end();
        }
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("name", &self.name)?;
        map.serialize_entry("signature", &self.signature)?;
        map.serialize_entry("inputs", &inputs_to_abi_format(&self.inputs))?;
        map.end()
    }
}
/// A resolved log signature.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ResolvedLog {
    /// The name of the log. For example, `Transfer`.
    pub name: String,
    /// The log signature. For example, `Transfer(address,address,uint256)`.
    pub signature: String,
    /// The inputs of the log. For example, `["address", "address", "uint256"]`.
    #[serde(deserialize_with = "deserialize_inputs")]
    pub inputs: Vec<String>,
}

impl ResolvedLog {
    /// Returns the inputs of the log as a vector of [`DynSolType`]s.
    pub fn inputs(&self) -> Vec<DynSolType> {
        parse_function_parameters(&self.signature).expect("invalid signature")
    }
}

impl Serialize for ResolvedLog {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer, {
        if !serializer.is_human_readable() {
            let mut value = serializer.serialize_struct("ResolvedLog", 3)?;
            value.serialize_field("name", &self.name)?;
            value.serialize_field("signature", &self.signature)?;
            value.serialize_field("inputs", &self.inputs)?;
            return value.end();
        }
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("name", &self.name)?;
        map.serialize_entry("signature", &self.signature)?;
        map.serialize_entry("inputs", &inputs_to_abi_format(&self.inputs))?;
        map.end()
    }
}
#[derive(Clone, Copy)]
enum SelectorKind {
    Function,
    Error,
    Event,
}

fn selector_cache_key(kind: SelectorKind, selector: &str) -> String {
    let kind = match kind {
        SelectorKind::Function => "function",
        SelectorKind::Error => "error",
        SelectorKind::Event => "event",
    };
    let selector =
        selector.strip_prefix("0x").or_else(|| selector.strip_prefix("0X")).unwrap_or(selector);
    format!("selector.{kind}.{}", selector.to_ascii_lowercase())
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
        let selector = selector
            .strip_prefix("0x")
            .or_else(|| selector.strip_prefix("0X"))
            .unwrap_or(selector)
            .to_ascii_lowercase();
        with_cache(&selector_cache_key(SelectorKind::Error, &selector), || async {
            trace!("resolving error selector {}", &selector);

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
                let text_signature = match signature.get("name") {
                    Some(text_signature) => text_signature.to_string().replace('"', ""),
                    None => continue,
                };

                let function_parts = match text_signature.split_once('(') {
                    Some(function_parts) => function_parts,
                    None => continue,
                };

                let parsed_inputs = match parse_function_parameters(&text_signature) {
                    Ok(inputs) => inputs,
                    Err(_) => continue,
                };

                signature_list.push(ResolvedError {
                    name: function_parts.0.to_string(),
                    signature: text_signature.to_string(),
                    inputs: dyn_sol_types_to_strings(&parsed_inputs),
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
        let selector = selector
            .strip_prefix("0x")
            .or_else(|| selector.strip_prefix("0X"))
            .unwrap_or(selector)
            .to_ascii_lowercase();
        with_cache(&selector_cache_key(SelectorKind::Event, &selector), || async {
            trace!("resolving event selector {}", &selector);

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
                let text_signature = match signature.get("name") {
                    Some(text_signature) => text_signature.to_string().replace('"', ""),
                    None => continue,
                };

                let function_parts = match text_signature.split_once('(') {
                    Some(function_parts) => function_parts,
                    None => continue,
                };

                let parsed_inputs = match parse_function_parameters(&text_signature) {
                    Ok(inputs) => inputs,
                    Err(_) => continue,
                };

                signature_list.push(ResolvedLog {
                    name: function_parts.0.to_string(),
                    signature: text_signature.to_string(),
                    inputs: dyn_sol_types_to_strings(&parsed_inputs),
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
        let selector = selector
            .strip_prefix("0x")
            .or_else(|| selector.strip_prefix("0X"))
            .unwrap_or(selector)
            .to_ascii_lowercase();
        with_cache(&selector_cache_key(SelectorKind::Function, &selector), || async {
            trace!("resolving function selector {}", &selector);

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
                let text_signature = match signature.get("name") {
                    Some(text_signature) => text_signature.to_string().replace('"', ""),
                    None => continue,
                };

                let function_parts = match text_signature.split_once('(') {
                    Some(function_parts) => function_parts,
                    None => continue,
                };

                let parsed_inputs = match parse_function_parameters(&text_signature) {
                    Ok(inputs) => inputs,
                    Err(_) => continue,
                };

                signature_list.push(ResolvedFunction {
                    name: function_parts.0.to_string(),
                    signature: text_signature.to_string(),
                    inputs: dyn_sol_types_to_strings(&parsed_inputs),
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

    for function in json_abi.functions() {
        let selector = function.selector().to_string().trim_start_matches("0x").to_string();
        let signature = function.signature();

        let inputs = match parse_function_parameters(&signature) {
            Ok(parsed) => dyn_sol_types_to_strings(&parsed),
            Err(_) => function.inputs.iter().map(|input| input.ty.clone()).collect(),
        };

        let resolved_function = ResolvedFunction {
            name: function.name.clone(),
            signature,
            inputs,
            decoded_inputs: None,
        };

        store_cache(
            &selector_cache_key(SelectorKind::Function, &selector),
            Some(vec![resolved_function]),
            None,
        )?;
    }
    for event in json_abi.events() {
        let selector = event.selector().to_string().trim_start_matches("0x").to_string();
        let signature = event.signature();

        let inputs = match parse_function_parameters(&signature) {
            Ok(parsed) => dyn_sol_types_to_strings(&parsed),
            Err(_) => event.inputs.iter().map(|input| input.ty.clone()).collect(),
        };

        let resolved_log = ResolvedLog { name: event.name.clone(), signature, inputs };

        store_cache(
            &selector_cache_key(SelectorKind::Event, &selector),
            Some(vec![resolved_log]),
            None,
        )?;
    }
    for error in json_abi.errors() {
        let selector = error.selector().to_string().trim_start_matches("0x").to_string();
        let signature = error.signature();

        let inputs = match parse_function_parameters(&signature) {
            Ok(parsed) => dyn_sol_types_to_strings(&parsed),
            Err(_) => error.inputs.iter().map(|input| input.ty.clone()).collect(),
        };

        let resolved_error = ResolvedError { name: error.name.clone(), signature, inputs };

        store_cache(
            &selector_cache_key(SelectorKind::Error, &selector),
            Some(vec![resolved_error]),
            None,
        )?;
    }

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
    let mut score = 1000;

    // this will prioritize shorter signatures, which are typically less spammy
    score -= signature.len() as u32;

    // prioritize signatures with less numbers
    score -= (signature.split('(').next().unwrap_or("").matches(|c: char| c.is_numeric()).count()
        as u32) *
        3;

    // prioritize signatures with parameters
    let num_params = signature.matches(',').count() + 1;
    score += num_params as u32 * 10;

    if let Some(num_words) = num_words {
        let num_dyn_params = signature.matches("bytes").count() +
            signature.matches("string").count() +
            signature.matches('[').count();
        let num_static_params = num_params.saturating_sub(num_dyn_params);

        if num_static_params < num_words {
            score = score.saturating_sub((num_words.saturating_sub(num_static_params)) as u32 * 10);
        }
    }

    score
}

/// trait impls
/// trait impls
/// trait impls

impl TryFrom<&ResolvedFunction> for TraceFactory {
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

    use crate::ether::{
        signatures::{
            dyn_sol_types_to_strings, score_signature, selector_cache_key, ResolveSelector,
            ResolvedError, ResolvedFunction, ResolvedLog, SelectorKind,
        },
        types::parse_function_parameters,
    };

    #[test]
    fn signature_json_round_trips_abi_input_objects() {
        let function = ResolvedFunction {
            name: "transfer".into(),
            signature: "transfer(address,uint256)".into(),
            inputs: vec!["address".into(), "uint256".into()],
            decoded_inputs: None,
        };
        let json = serde_json::to_string(&function).unwrap();
        assert_eq!(serde_json::from_str::<ResolvedFunction>(&json).unwrap(), function);
    }

    fn isolated_live_test(name: &str) -> bool {
        const CHILD: &str = "HEIMDALL_LIVE_SIGNATURE_TEST_CHILD";
        if std::env::var(CHILD).as_deref() == Ok(name) {
            return true;
        }
        let home = std::env::temp_dir().join(format!(
            "heimdall-live-signatures-{}-{}",
            std::process::id(),
            name.replace("::", "-")
        ));
        std::fs::create_dir_all(&home).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", name])
            .env(CHILD, name)
            .env("HOME", &home)
            .status()
            .unwrap();
        std::fs::remove_dir_all(home).unwrap();
        assert!(status.success());
        false
    }

    #[test]
    fn abi_signatures_are_cached_by_kind_and_normalized_selector() {
        const CHILD: &str = "HEIMDALL_SIGNATURE_CACHE_TEST_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let abi_path = std::path::Path::new(&std::env::var("HOME").unwrap()).join("abi.json");
            std::fs::write(&abi_path, r#"[
              {"type":"function","name":"burn","inputs":[{"name":"x","type":"uint256"}],"outputs":[]},
              {"type":"error","name":"collate_propagate_storage","inputs":[{"name":"x","type":"bytes16"}]},
              {"type":"event","name":"burn","inputs":[{"name":"x","type":"uint256","indexed":false}],"anonymous":false}
            ]"#).unwrap();
            super::cache_signatures_from_abi(abi_path).unwrap();
            let hash = alloy::primitives::keccak256("burn(uint256)");
            assert_eq!(
                &hash[..4],
                &alloy::primitives::keccak256("collate_propagate_storage(bytes16)")[..4]
            );
            let four_bytes = alloy::hex::encode(&hash[..4]);
            let event = alloy::hex::encode(hash);
            let function_key = selector_cache_key(SelectorKind::Function, &four_bytes);
            let error_key = selector_cache_key(SelectorKind::Error, &four_bytes);
            let event_key = selector_cache_key(SelectorKind::Event, &event);
            assert!(heimdall_cache::read_cache::<Option<Vec<ResolvedFunction>>>(&function_key)
                .unwrap()
                .is_some());
            assert!(heimdall_cache::read_cache::<Option<Vec<ResolvedError>>>(&error_key)
                .unwrap()
                .is_some());
            assert!(heimdall_cache::read_cache::<Option<Vec<ResolvedLog>>>(&event_key)
                .unwrap()
                .is_some());
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let function = runtime
                .block_on(ResolvedFunction::resolve(&format!("0X{}", four_bytes.to_uppercase())))
                .unwrap()
                .unwrap();
            let error = runtime.block_on(ResolvedError::resolve(&four_bytes)).unwrap().unwrap();
            let log = runtime.block_on(ResolvedLog::resolve(&event)).unwrap().unwrap();
            assert_eq!(function[0].signature, "burn(uint256)");
            assert_eq!(function[0].inputs, vec!["uint256"]);
            let json = serde_json::to_value(&function[0]).unwrap();
            assert_eq!(json["inputs"][0]["type"], "uint256");
            assert_eq!(error[0].signature, "collate_propagate_storage(bytes16)");
            assert_eq!(error[0].inputs, vec!["bytes16"]);
            assert_eq!(log[0].signature, "burn(uint256)");
            assert_eq!(log[0].inputs, vec!["uint256"]);
            return;
        }
        let home =
            std::env::temp_dir().join(format!("heimdall-signatures-cache-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "ether::signatures::tests::abi_signatures_are_cached_by_kind_and_normalized_selector"])
            .env(CHILD, "1").env("HOME", &home).status().unwrap();
        std::fs::remove_dir_all(home).unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    #[ignore = "requires the live OpenChain signature service"]
    async fn resolve_function_signature_nominal() {
        if !isolated_live_test("ether::signatures::tests::resolve_function_signature_nominal") {
            return;
        }
        let signature = String::from("095ea7b3");
        let _ = delete_cache(&selector_cache_key(SelectorKind::Function, &signature));
        let result = ResolvedFunction::resolve(&signature)
            .await
            .expect("failed to resolve signature")
            .expect("failed to resolve signature");
        assert!(!result.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires the live OpenChain signature service"]
    async fn resolve_multicall_signature() {
        if !isolated_live_test("ether::signatures::tests::resolve_multicall_signature") {
            return;
        }
        let signature = String::from("1749e1e3");
        let _ = delete_cache(&selector_cache_key(SelectorKind::Function, &signature));
        let result = ResolvedFunction::resolve(&signature)
            .await
            .expect("failed to resolve signature")
            .expect("failed to resolve signature");

        let multicall = result.iter().find(|f| f.name == "multicall");
        assert!(multicall.is_some(), "multicall function not found");

        let multicall = multicall.unwrap();
        // The inputs should be ["tuple[]"] not ["(address", "uint256", "bytes)[]"]
        assert_eq!(multicall.inputs, vec!["tuple[]"]);
    }

    #[tokio::test]
    #[ignore = "requires the live OpenChain signature service"]
    async fn resolve_error_signature_nominal() {
        if !isolated_live_test("ether::signatures::tests::resolve_error_signature_nominal") {
            return;
        }
        let signature = String::from("30cd7471");
        let _ = delete_cache(&selector_cache_key(SelectorKind::Error, &signature));
        let result = ResolvedError::resolve(&signature)
            .await
            .expect("failed to resolve signature")
            .expect("failed to resolve signature");
        assert!(!result.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires the live OpenChain signature service"]
    async fn resolve_event_signature_nominal() {
        if !isolated_live_test("ether::signatures::tests::resolve_event_signature_nominal") {
            return;
        }
        let signature =
            String::from("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");
        let _ = delete_cache(&selector_cache_key(SelectorKind::Event, &signature));
        let result = ResolvedLog::resolve(&signature)
            .await
            .expect("failed to resolve signature")
            .expect("failed to resolve signature");
        assert!(!result.is_empty());
    }

    #[test]
    fn score_signature_should_return_correct_score() {
        let signature = String::from("test_signature");
        let score = score_signature(&signature, None);
        assert_eq!(score, 996);
    }

    #[test]
    fn test_complex_signature_parsing() {
        let test_cases = vec![
            ("function((uint256,address)[])", vec!["tuple[]"]),
            ("function(address,(uint256,uint256))", vec!["address", "tuple"]),
            (
                "function(uint256[],bytes32,(address,uint256)[])",
                vec!["uint256[]", "bytes32", "tuple[]"],
            ),
            ("multicall((address,uint256,bytes)[])", vec!["tuple[]"]),
            ("function((address,uint256,bytes)[],(uint256,bool))", vec!["tuple[]", "tuple"]),
        ];

        for (signature, expected_inputs) in test_cases {
            let parsed = parse_function_parameters(signature).unwrap();
            let string_inputs = dyn_sol_types_to_strings(&parsed);
            assert_eq!(string_inputs, expected_inputs, "Failed for signature: {}", signature);
        }
    }

    #[test]
    fn test_score_signature_handles_overflow_in_num_static_params() {
        let signature = "bytes(bytes[])"; // 1 param, but matches "bytes" twice and has 1 bracket
        let score = score_signature(&signature, Some(0));
        assert!(score > 0);
    }

    #[test]
    fn test_score_signature_handles_overflow_in_score_reduction() {
        let signature = "func()"; // 1 param (count from commas + 1), 0 dynamic params
        let score = score_signature(&signature, Some(10));
        assert_eq!(score, 914);
    }

    #[test]
    fn test_score_signature_saturating_sub_prevents_underflow() {
        let signature = "bytes(bytes,string,bytes[])";
        let score = score_signature(&signature, Some(2));
        assert!(score > 0);
    }
}
