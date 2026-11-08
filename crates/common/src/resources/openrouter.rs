use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestUserMessage,
        ChatCompletionRequestUserMessageContent, CreateChatCompletionRequestArgs, ResponseFormat,
        ResponseFormatJsonSchema,
    },
    Client,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tracing::error;

/// Structured output response for annotated contract source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotatedContractResponse {
    /// The annotated and cleaned up Solidity source code.
    pub source: String,
}

impl AnnotatedContractResponse {
    /// JSON Schema for an annotated contract response.
    pub fn json_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "The annotated and cleaned up Solidity source code"
                }
            },
            "required": ["source"],
            "additionalProperties": false
        })
    }
}

/// The default model to use for OpenRouter completions.
pub const DEFAULT_MODEL: &str = "openai/gpt-4o-mini";

/// The OpenRouter API base URL.
const OPENROUTER_API_BASE: &str = "https://openrouter.ai/api/v1";

/// Complete the given prompt via chat using the OpenRouter API.
///
/// OpenRouter provides access to multiple LLM providers through a unified API.
/// Models are specified in the format "provider/model-name" (e.g., "openai/gpt-4o").
///
/// ```
/// use heimdall_common::resources::openrouter::complete_chat;
///
/// let prompt = "what is love?";
/// let api_key = "your-openrouter-api-key";
/// let model = "openai/gpt-4o-mini";
/// // complete_chat(prompt, api_key, model).await;
/// ```
pub async fn complete_chat(prompt: &str, api_key: &str, model: &str) -> Option<String> {
    let http_client =
        reqwest::Client::builder().timeout(std::time::Duration::from_secs(90)).build().unwrap();

    let config = OpenAIConfig::new().with_api_key(api_key).with_api_base(OPENROUTER_API_BASE);

    let client = Client::with_config(config).with_http_client(http_client);

    let model_to_use = if model.is_empty() { DEFAULT_MODEL } else { model };

    let request = match CreateChatCompletionRequestArgs::default()
        .model(model_to_use)
        .messages(vec![ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
            name: Some("user".to_string()),
            content: ChatCompletionRequestUserMessageContent::Text(prompt.to_string()),
        })])
        .build()
    {
        Ok(request) => request,
        Err(e) => {
            error!("failed to create completion request: {}", e);
            return None;
        }
    };

    match client.chat().create(request).await {
        Ok(response) => {
            if !response.choices.is_empty() {
                response.choices[0].message.content.to_owned()
            } else {
                None
            }
        }
        Err(e) => {
            error!("failed to create completion request: {}", e);
            None
        }
    }
}

/// Complete the given prompt via chat using structured JSON output via the OpenRouter API.
///
/// This compatibility entry point requests the annotated-contract schema. To request a different
/// response structure, use [`complete_chat_structured_with_schema`].
///
/// ```
/// use heimdall_common::resources::openrouter::{complete_chat_structured, AnnotatedContractResponse};
///
/// let prompt = "Annotate this contract";
/// let api_key = "your-openrouter-api-key";
/// let model = "openai/gpt-4o-mini";
/// // complete_chat_structured::<AnnotatedContractResponse>(prompt, api_key, model, "annotated_contract").await;
/// ```
pub async fn complete_chat_structured<T: DeserializeOwned>(
    prompt: &str,
    api_key: &str,
    model: &str,
    schema_name: &str,
) -> Option<T> {
    complete_chat_structured_with_schema(
        prompt,
        api_key,
        model,
        schema_name,
        AnnotatedContractResponse::json_schema(),
        Some("Annotated contract source code"),
    )
    .await
}

/// Complete a prompt using the caller's JSON Schema for structured output.
///
/// The schema must describe the structure expected by `T`.
pub async fn complete_chat_structured_with_schema<T: DeserializeOwned>(
    prompt: &str,
    api_key: &str,
    model: &str,
    schema_name: &str,
    schema: serde_json::Value,
    description: Option<&str>,
) -> Option<T> {
    let http_client =
        reqwest::Client::builder().timeout(std::time::Duration::from_secs(120)).build().unwrap();

    let config = OpenAIConfig::new().with_api_key(api_key).with_api_base(OPENROUTER_API_BASE);

    let client = Client::with_config(config).with_http_client(http_client);

    let model_to_use = if model.is_empty() { DEFAULT_MODEL } else { model };

    let response_format = structured_response_format(schema_name, schema, description);

    let request = match CreateChatCompletionRequestArgs::default()
        .model(model_to_use)
        .response_format(response_format)
        .messages(vec![ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
            name: Some("user".to_string()),
            content: ChatCompletionRequestUserMessageContent::Text(prompt.to_string()),
        })])
        .build()
    {
        Ok(request) => request,
        Err(e) => {
            error!("failed to create completion request: {}", e);
            return None;
        }
    };

    match client.chat().create(request).await {
        Ok(response) => {
            if !response.choices.is_empty() {
                if let Some(content) = response.choices[0].message.content.as_ref() {
                    match serde_json::from_str::<T>(content) {
                        Ok(parsed) => Some(parsed),
                        Err(e) => {
                            error!("failed to parse structured response: {}", e);
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
        Err(e) => {
            error!("failed to create completion request: {}", e);
            None
        }
    }
}

fn structured_response_format(
    schema_name: &str,
    schema: serde_json::Value,
    description: Option<&str>,
) -> ResponseFormat {
    ResponseFormat::JsonSchema {
        json_schema: ResponseFormatJsonSchema {
            name: schema_name.to_string(),
            description: description.map(str::to_owned),
            schema: Some(schema),
            strict: Some(true),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn structured_response_format_preserves_caller_schema() {
        let schema = json!({
            "type": "object",
            "properties": { "count": { "type": "integer" } },
            "required": ["count"],
            "additionalProperties": false
        });
        let format = structured_response_format("counter", schema.clone(), None);
        match format {
            ResponseFormat::JsonSchema { json_schema } => {
                assert_eq!(json_schema.name, "counter");
                assert_eq!(json_schema.schema, Some(schema));
                assert_eq!(json_schema.strict, Some(true));
                assert!(json_schema.description.is_none());
            }
            _ => panic!("expected JSON schema response format"),
        }
    }

    #[test]
    fn annotated_contract_schema_matches_response_shape() {
        let schema = AnnotatedContractResponse::json_schema();
        assert_eq!(schema["properties"]["source"]["type"], "string");
        assert_eq!(schema["required"], json!(["source"]));
        assert_eq!(schema["additionalProperties"], false);
        let format = structured_response_format(
            "annotated_contract",
            schema,
            Some("Annotated contract source code"),
        );
        match format {
            ResponseFormat::JsonSchema { json_schema } => {
                assert_eq!(
                    json_schema.description.as_deref(),
                    Some("Annotated contract source code")
                );
                assert_eq!(json_schema.name, "annotated_contract");
            }
            _ => panic!("expected JSON schema response format"),
        }
        let response: AnnotatedContractResponse =
            serde_json::from_value(json!({ "source": "contract Example {}" })).unwrap();
        assert_eq!(response.source, "contract Example {}");
    }
}
