use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestUserMessage,
        ChatCompletionRequestUserMessageContent, CreateChatCompletionRequestArgs,
    },
    Client,
};
use tracing::error;

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

    // Use provided model or fall back to default
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
