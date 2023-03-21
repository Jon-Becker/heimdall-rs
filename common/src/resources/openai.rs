use crate::{io::logging::Logger};
use async_openai::{
    types::{CreateCompletionRequestArgs},
    Client,
};

pub fn complete(prompt: String, api_key: &String, logger: &Logger) -> Option<String> {
    let client = Client::new().with_api_key(api_key);

    // create new runtime block
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let response = rt.block_on(async {
        let request = match
            CreateCompletionRequestArgs::default()
            .model("text-davinci-003")
            .prompt(prompt)
            .max_tokens(1024_u16)
            .temperature(0.75)
            .n(2)
            .build()
        {
            Ok(request) => request,
            Err(e) => {
                logger.error(&format!("failed to create completion request: {}", e));
                return None;
            }
        };
        
        match client.completions().create(request).await {
            Ok(response) => {
                if response.choices.len() > 0 {
                    return Some(response.choices[0].text.clone());
                }
                else {
                    return None;
                }
            },
            Err(e) => {
                logger.error(&format!("failed to create completion request: {}", e));
                return None;
            }
        }
    });

    response
}