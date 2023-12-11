use backoff::ExponentialBackoff;
use indicatif::ProgressBar;
use reqwest::header::HeaderMap;
use serde_json::Value;
use std::time::{Duration, Instant};

use crate::{debug_max, utils::io::logging::Logger};
use serde::{Deserialize, Serialize};

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
            // get a new logger
            let logger = Logger::default();

            // build the headers
            let mut headers = HeaderMap::new();
            headers.insert("Content-Type", "application/json".parse().unwrap());
            headers.insert("X-API-KEY", api_key.parse().unwrap());

            // clone the query
            let query = query.to_owned();

            // make the request
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(999999999))
                .build()
                .unwrap();

            let response = match client
                .post("https://api.transpose.io/sql")
                .body(query.clone())
                .headers(headers)
                .send()
                .await
            {
                Ok(res) => res,
                Err(e) => {
                    logger.error("failed to call Transpose .");
                    logger.error(&format!("error: {e}"));
                    return Err(backoff::Error::Permanent(()))
                }
            };

            // parse body
            match response.text().await {
                Ok(body) => Ok(match serde_json::from_str(&body) {
                    Ok(json) => json,
                    Err(e) => {
                        logger.error("Transpose request unsucessful.");
                        logger.debug(&format!("curl: curl -X GET \"https://api.transpose.io/sql\" -H \"accept: application/json\" -H \"Content-Type: application/json\" -H \"X-API-KEY: {api_key}\" -d {query}"));
                        logger.error(&format!("error: {e}"));
                        logger.debug(&format!("response body: {body:?}"));
                        return Err(backoff::Error::Permanent(()))
                    }
                }),
                Err(e) => {
                    logger.error("failed to parse Transpose response body.");
                    logger.error(&format!("error: {e}"));
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
) -> Vec<(u128, String)> {
    // get a new logger
    let logger = Logger::default();

    // get a new progress bar
    let transaction_list_progress = ProgressBar::new_spinner();
    transaction_list_progress.enable_steady_tick(Duration::from_millis(100));
    transaction_list_progress.set_style(logger.info_spinner());
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

    let response = match call_transpose(&query, api_key).await {
        Some(response) => response,
        None => {
            logger.error("failed to get transaction list from Transpose");
            std::process::exit(1)
        }
    };

    transaction_list_progress.finish_and_clear();
    logger.debug(&format!("fetching transactions took {:?}", start_time.elapsed()));

    let mut transactions = Vec::new();

    // parse the results
    for result in response.results {
        let block_number: u128 = match result.get("block_number") {
            Some(block_number) => match block_number.as_u64() {
                Some(block_number) => block_number as u128,
                None => {
                    logger.error("failed to parse block_number from Transpose");
                    std::process::exit(1)
                }
            },
            None => {
                logger.error("failed to fetch block_number from Transpose response");
                std::process::exit(1)
            }
        };
        let transaction_hash: String = match result.get("transaction_hash") {
            Some(transaction_hash) => match transaction_hash.as_str() {
                Some(transaction_hash) => transaction_hash.to_string(),
                None => {
                    logger.error("failed to parse transaction_hash from Transpose");
                    std::process::exit(1)
                }
            },
            None => {
                logger.error("failed to fetch transaction_hash from Transpose response");
                std::process::exit(1)
            }
        };

        transactions.push((block_number, transaction_hash));
    }

    // sort the transactions by block number
    transactions.sort_by(|a, b| a.0.cmp(&b.0));

    transactions
}

/// Get the contrct creation block and transaction hash for the given address.
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
    // get a new logger
    let logger = Logger::default();

    // get a new progress bar
    let transaction_list_progress = ProgressBar::new_spinner();
    transaction_list_progress.enable_steady_tick(Duration::from_millis(100));
    transaction_list_progress.set_style(logger.info_spinner());
    transaction_list_progress.set_message(format!("fetching '{address}''s creation tx ."));
    let start_time = Instant::now();

    // build the SQL query
    let query = format!(
        "{{\"sql\":\"SELECT block_number, transaction_hash FROM {chain}.transactions WHERE TIMESTAMP = ( SELECT created_timestamp FROM {chain}.accounts WHERE address = '{address}' ) AND contract_address = '{address}'\",\"parameters\":{{}},\"options\":{{\"timeout\": 999999999}}}}",
    );

    let response = match call_transpose(&query, api_key).await {
        Some(response) => response,
        None => {
            logger.error("failed to get creation tx from Transpose");
            std::process::exit(1)
        }
    };

    transaction_list_progress.finish_and_clear();
    logger.debug(&format!("fetching contract creation took {:?}", start_time.elapsed()));

    // parse the results
    if let Some(result) = response.results.into_iter().next() {
        let block_number: u128 = match result.get("block_number") {
            Some(block_number) => match block_number.as_u64() {
                Some(block_number) => block_number as u128,
                None => {
                    logger.error("failed to parse block_number from Transpose");
                    std::process::exit(1)
                }
            },
            None => {
                logger.error("failed to fetch block_number from Transpose response");
                std::process::exit(1)
            }
        };
        let transaction_hash: String = match result.get("transaction_hash") {
            Some(transaction_hash) => match transaction_hash.as_str() {
                Some(transaction_hash) => transaction_hash.to_string(),
                None => {
                    logger.error("failed to parse transaction_hash from Transpose");
                    std::process::exit(1)
                }
            },
            None => {
                logger.error("failed to fetch transaction_hash from Transpose response");
                std::process::exit(1)
            }
        };

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

    let response = match call_transpose(&query, api_key).await {
        Some(response) => response,
        None => {
            debug_max!(&format!("failed to get label from Transpose for address: {}", address));
            return None;
        }
    };

    // parse the results
    if let Some(result) = response.results.into_iter().next() {
        let label: String = match result.get("label") {
            Some(label) => match label.as_str() {
                Some(label) => label.to_string(),
                None => {
                    debug_max!(&format!(
                        "failed to parse label from Transpose for address: {}",
                        address
                    ));
                    return None;
                }
            },
            None => {
                debug_max!(&format!(
                    "failed to fetch label from Transpose response for address: {}",
                    address
                ));
                return None;
            }
        };
        return Some(label);
    };

    None
}
