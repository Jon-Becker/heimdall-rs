use indicatif::ProgressBar;
use reqwest::header::HeaderMap;
use serde_json::Value;
use std::time::{Duration, Instant};

use crate::io::logging::Logger;
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

async fn _call_transpose(query: &str, api_key: &str) -> Option<TransposeResponse> {
    // get a new logger
    let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "INFO".into());
    let (logger, _) = Logger::new(&level);

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
            std::process::exit(1)
        }
    };

    // parse body
    match response.text().await {
        Ok(body) => Some(match serde_json::from_str(&body) {
            Ok(json) => json,
            Err(e) => {
                logger.error("Transpose request unsucessful.");
                logger.debug(&format!("curl: curl -X GET \"https://api.transpose.io/sql\" -H \"accept: application/json\" -H \"Content-Type: application/json\" -H \"X-API-KEY: {api_key}\" -d {query}"));
                logger.error(&format!("error: {e}"));
                logger.debug(&format!("response body: {body:?}"));
                std::process::exit(1)
            }
        }),
        Err(e) => {
            logger.error("failed to parse Transpose response body.");
            logger.error(&format!("error: {e}"));
            std::process::exit(1)
        }
    }
}

pub async fn get_transaction_list(
    chain: &str,
    address: &str,
    api_key: &str,
    bounds: (&u128, &u128),
) -> Vec<(u128, String)> {
    // get a new logger
    let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "INFO".into());
    let (logger, _) = Logger::new(&level);

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

    let response = match _call_transpose(&query, api_key).await {
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

pub async fn get_contract_creation(
    chain: &str,
    address: &str,
    api_key: &str,
) -> Option<(u128, String)> {
    // get a new logger
    let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "INFO".into());
    let (logger, _) = Logger::new(&level);

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

    let response = match _call_transpose(&query, api_key).await {
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

        return Some((block_number, transaction_hash))
    };

    None
}
