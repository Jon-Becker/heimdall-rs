use crate::io::logging::Logger;
use async_openai::{types::CreateCompletionRequestArgs, Client};

pub async fn complete(prompt: &str, api_key: &str) -> Option<String> {
    let client = Client::new().with_api_key(api_key);

    // get a new logger
    let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "INFO".into());
    let (logger, _) = Logger::new(&level);
    let request = match CreateCompletionRequestArgs::default()
        .model("text-davinci-003")
        .prompt(prompt)
        .max_tokens(512_u16)
        .temperature(0.75)
        .frequency_penalty(1.1)
        .n(2)
        .build()
    {
        Ok(request) => request,
        Err(e) => {
            logger.error(&format!("failed to create completion request: {e}"));
            return None
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
            logger.error(&format!("failed to create completion request: {e}"));
            None
        }
    }
}
