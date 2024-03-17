use backoff::ExponentialBackoff;
use indicatif::ProgressBar;
use reqwest::header::HeaderMap;
use serde_json::Value;
use std::time::{Duration, Instant};

use crate::{info_spinner, Error};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

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
    backoff::future::retry(
        ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(10)),
            ..ExponentialBackoff::default()
        },
        || async {
            // build the headers
            let mut headers = HeaderMap::new();
            headers.insert("Content-Type", "application/json".parse().expect("failed to parse Content-Type header"));
            headers.insert("X-API-KEY", api_key.parse().expect("failed to parse API key header"));

            // clone the query
            let query = query.to_owned();

            // make the request
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(999999999))
                .build()
                .expect("failed to build reqwest client");

            let response = match client
                .post("https://api.transpose.io/sql")
                .body(query.clone())
                .headers(headers)
                .send()
                .await
            {
                Ok(res) => res,
                Err(e) => {
                    error!("failed to call Transpose .");
                    error!("error: {}", e);
                    return Err(backoff::Error::Permanent(()))
                }
            };

            // parse body
            match response.text().await {
                Ok(body) => Ok(match serde_json::from_str(&body) {
                    Ok(json) => json,
                    Err(e) => {
                        error!("Transpose request unsucessful.");
                        debug!("curl: curl -X GET \"https://api.transpose.io/sql\" -H \"accept: application/json\" -H \"Content-Type: application/json\" -H \"X-API-KEY: {}\" -d {}", api_key, query);
                        error!("error: {}", e);
                        debug!("response body: {:?}", body);
                        return Err(backoff::Error::Permanent(()))
                    }
                }),
                Err(e) => {
                    error!("failed to parse Transpose response body.");
                    error!("error: {}", e);
                    Err(backoff::Error::Permanent(()))
                }
            }
        },
    ).await
    .ok()
}

/// Get all interactions with the given address. Includes transactions to, from, as well as internal
/// transactions to and from the address.
///
/// ```
/// use heimdall_common::resources::transpose::get_transaction_list;
///
/// let chain = "ethereum";
/// let address = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
/// let api_key = "YOUR_API_KEY";
/// let bounds = (0, 1); // block number bounds
///
/// // let transactions = get_transaction_list(chain, address, api_key, bounds).await;
/// ```
pub async fn get_transaction_list(
    chain: &str,
    address: &str,
    api_key: &str,
    bounds: (&u128, &u128),
) -> Result<Vec<(u128, String)>, Error> {
    // get a new progress bar
    let transaction_list_progress = ProgressBar::new_spinner();
    transaction_list_progress.enable_steady_tick(Duration::from_millis(100));
    transaction_list_progress.set_style(info_spinner!());
    transaction_list_progress.set_message(format!("fetching transactions from '{address}' ."));
    let start_time = Instant::now();

    // build the SQL query
    let query = format!(
        "{{\"sql\":\"SELECT block_number, transaction_hash FROM  (SELECT transaction_hash, block_number FROM {chain}.transactions WHERE to_address = '{}' AND block_number BETWEEN {} AND {}  UNION  SELECT transaction_hash, block_number FROM {chain}.traces WHERE to_address = '{}' AND block_number BETWEEN {} AND {}) x\",\"parameters\":{{}},\"options\":{{\"timeout\": 999999999}}}}",
        address,
        bounds.0,
        bounds.1,
        address,
        bounds.0,
        bounds.1
    );

    let response = call_transpose(&query, api_key)
        .await
        .ok_or(Error::Generic("failed to get transaction list from Transpose".to_string()))?;

    transaction_list_progress.finish_and_clear();
    debug!("fetching transactions took {:?}", start_time.elapsed());

    let mut transactions = Vec::new();

    // parse the results
    for result in response.results {
        let block_number = result
            .get("block_number")
            .ok_or(Error::Generic("failed to parse block_number from Transpose".to_string()))?
            .as_u64()
            .ok_or(Error::Generic("failed to parse block_number from Transpose".to_string()))?
            as u128;

        let transaction_hash = result
            .get("transaction_hash")
            .ok_or(Error::Generic("failed to parse transaction_hash from Transpose".to_string()))?
            .as_str()
            .ok_or(Error::Generic("failed to parse transaction_hash from Transpose".to_string()))?
            .to_string();

        transactions.push((block_number, transaction_hash));
    }

    // sort the transactions by block number
    transactions.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(transactions)
}

/// Get the contract creation block and transaction hash for the given address.
///
/// ```
/// use heimdall_common::resources::transpose::get_contract_creation;
///
/// let chain = "ethereum";
/// let address = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
/// let api_key = "YOUR_API_KEY";
///
/// // let contract_creation = get_contract_creation(chain, address, api_key).await;
/// ```
pub async fn get_contract_creation(
    chain: &str,
    address: &str,
    api_key: &str,
) -> Option<(u128, String)> {
    // get a new progress bar
    let transaction_list_progress = ProgressBar::new_spinner();
    transaction_list_progress.enable_steady_tick(Duration::from_millis(100));
    transaction_list_progress.set_style(info_spinner!());
    transaction_list_progress.set_message(format!("fetching '{address}''s creation tx ."));
    let start_time = Instant::now();

    // build the SQL query
    let query = format!(
        "{{\"sql\":\"SELECT block_number, transaction_hash FROM {chain}.transactions WHERE TIMESTAMP = ( SELECT created_timestamp FROM {chain}.accounts WHERE address = '{address}' ) AND contract_address = '{address}'\",\"parameters\":{{}},\"options\":{{\"timeout\": 999999999}}}}",
    );

    let response = call_transpose(&query, api_key).await?;

    transaction_list_progress.finish_and_clear();
    debug!("fetching contract creation took {:?}", start_time.elapsed());

    // parse the results
    if let Some(result) = response.results.into_iter().next() {
        let block_number = result
            .get("block_number")
            .and_then(|block_number| block_number.as_u64())
            .map(|block_number| block_number as u128)?;

        let transaction_hash = result
            .get("transaction_hash")
            .and_then(|transaction_hash| transaction_hash.as_str())
            .map(|transaction_hash| transaction_hash.to_string())?;

        return Some((block_number, transaction_hash));
    };

    None
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
    // build the SQL query
    let query = format!(
            "{{\"sql\":\"SELECT COALESCE( (SELECT name FROM ethereum.contract_labels WHERE contract_address = '{address}' ), (SELECT ens_name FROM ethereum.ens_names WHERE primary_address = '{address}' LIMIT 1), (SELECT protocol_name FROM ethereum.protocols WHERE contract_address = '{address}' ), (SELECT symbol FROM ethereum.tokens WHERE contract_address = '{address}' ), (SELECT symbol FROM ethereum.collections WHERE contract_address = '{address}' ) ) as label\",\"parameters\":{{}},\"options\":{{\"timeout\": 999999999}}}}",
        );

    let response = call_transpose(&query, api_key).await?;

    // parse the results
    if let Some(result) = response.results.into_iter().next() {
        return result.get("label").and_then(|label| label.as_str()).map(|label| label.to_string());
    };

    None
}
