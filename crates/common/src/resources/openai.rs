use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestUserMessage,
        ChatCompletionRequestUserMessageContent, CreateChatCompletionRequestArgs,
        CreateCompletionRequestArgs,
    },
    Client,
};
use tracing::error;

/// Complete the given prompt using the OpenAI API.
///
/// ```
/// use heimdall_common::resources::openai::complete;
///
/// let prompt = "what is love?";
/// let api_key = "your-api-key";
/// // complete(prompt, api_key).await;
pub async fn complete(prompt: &str, api_key: &str) -> Option<String> {
    let config = OpenAIConfig::new().with_api_key(api_key);
    let client = Client::with_config(config);

    let request = match CreateCompletionRequestArgs::default()
        .model("gpt-3.5-turbo-instruct")
        .prompt(prompt)
        .max_tokens(512_u16)
        .temperature(0.75)
        .frequency_penalty(1.1)
        .n(2)
        .build()
    {
        Ok(request) => request,
        Err(e) => {
            error!("failed to create completion request: {}", e);
            return None;
        }
    };

    match client.completions().create(request).await {
        Ok(response) => {
            if !response.choices.is_empty() {
                Some(response.choices[0].text.to_owned())
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

/// Complete the given prompt via chat using the OpenAI API.
///
/// ```
/// use heimdall_common::resources::openai::complete_chat;
///
/// let prompt = "what is love?";
/// let api_key = "your-api-key";
/// // complete_chat(prompt, api_key).await;
pub async fn complete_chat(prompt: &str, api_key: &str) -> Option<String> {
    let http_client =
        reqwest::Client::builder().timeout(std::time::Duration::from_secs(90)).build().unwrap();
    let config = OpenAIConfig::new().with_api_key(api_key);
    let client = Client::with_config(config).with_http_client(http_client);

    let request = match CreateChatCompletionRequestArgs::default()
        .model("o1-mini")
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
