use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;
use tracing::trace;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

/// Make a GET request and return its JSON body, or `None` if the body is not JSON.
/// Transport and HTTP status failures are returned to the caller.
pub async fn get_json_from_url(url: &str, timeout: u64) -> Result<Option<Value>, reqwest::Error> {
    let client = Client::builder()
        .user_agent(APP_USER_AGENT)
        .timeout(Duration::from_secs(timeout))
        .build()?;

    for attempt in 0..3 {
        trace!("GET {}", url);
        match client.get(url).send().await {
            Ok(response) => {
                let body = response.error_for_status()?.text().await?;
                return Ok(serde_json::from_str(&body).ok());
            }
            Err(error) if attempt < 2 => {
                trace!("GET {}: {:?}", url, error);
                sleep(Duration::from_millis(500 << attempt)).await;
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("the final request returns either a response or an error")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    async fn serve_once(status: &str, body: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        tokio::spawn(async move {
            let (mut connection, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            let _ = connection.read(&mut request).await.unwrap();
            connection.write_all(response.as_bytes()).await.unwrap();
        });
        format!("http://{address}")
    }

    #[tokio::test]
    async fn returns_json_and_surfaces_http_errors() {
        let url = serve_once("200 OK", "{\"ok\":true}").await;
        assert_eq!(get_json_from_url(&url, 2).await.unwrap().unwrap()["ok"], true);
        let url = serve_once("503 Service Unavailable", "{}").await;
        assert!(get_json_from_url(&url, 2).await.unwrap_err().is_status());
    }
}
