use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json::Value;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{debug, trace};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TransposeStats {
    count: u128,
    size: u128,
    time: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TransposeResponse {
    status: String,
    stats: TransposeStats,
    results: Vec<Value>,
}

/// executes a transpose SQL query and returns the response
async fn call_transpose(query: &str, api_key: &str) -> Option<TransposeResponse> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert("X-API-KEY", HeaderValue::from_str(api_key).ok()?);
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .ok()?;
    let response = client
        .post("https://api.transpose.io/sql")
        .body(query.to_owned())
        .headers(headers)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let body = response.text().await.ok()?;
    match serde_json::from_str(&body) {
        Ok(json) => Some(json),
        Err(error) => {
            debug!("invalid Transpose response: {error}");
            None
        }
    }
}

/// Get the label for the given address.
///
/// ```
/// use heimdall_common::resources::transpose::get_label;
///
/// let address = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
/// let api_key = "YOUR_API_KEY";
///
/// // let label = get_label(address, api_key).await;
/// ```
pub async fn get_label(address: &str, api_key: &str) -> Option<String> {
    let query = format!(
            "{{\"sql\":\"SELECT COALESCE( (SELECT name FROM ethereum.contract_labels WHERE contract_address = '{address}' ), (SELECT ens_name FROM ethereum.ens_names WHERE primary_address = '{address}' LIMIT 1), (SELECT protocol_name FROM ethereum.protocols WHERE contract_address = '{address}' ), (SELECT symbol FROM ethereum.tokens WHERE contract_address = '{address}' ), (SELECT symbol FROM ethereum.collections WHERE contract_address = '{address}' ) ) as label\",\"parameters\":{{}},\"options\":{{\"timeout\": 999999999}}}}",
        );

    trace!("querying label for address: {}", address);
    let start_time = Instant::now();
    let response = call_transpose(&query, api_key).await?;
    trace!("fetching label for {address} took {:?}", start_time.elapsed());

    if let Some(result) = response.results.into_iter().next() {
        return result.get("label").and_then(|label| label.as_str()).map(|label| label.to_string());
    };

    None
}
