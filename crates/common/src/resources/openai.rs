use async_openai::{types::CreateCompletionRequestArgs, Client};
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
    let client = Client::new().with_api_key(api_key);

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
                Some(response.choices[0].text.clone())
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
