use std::{io::Read, time::{Instant, Duration}};

use indicatif::ProgressBar;
use reqwest::header::{HeaderMap};
use serde_json::Value;

use crate::{io::logging::Logger};
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
    results: Vec<Value>
}

fn _call_transpose(query: String, api_key: &String) -> Option<TransposeResponse> {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "application/json".parse().unwrap());
    headers.insert("X-API-KEY", api_key.parse().unwrap());

    // make the request
    let client = reqwest::blocking::Client::builder().redirect(reqwest::redirect::Policy::none()).build() .unwrap();
    let mut response = match client.post("https://api.transpose.io/sql").headers(headers).body(query).send() {
        Ok(res) => res,
        Err(e) => {
            let (logger, _) = Logger::new("TRACE");
            logger.error(&format!("failed to get transaction list from Transpose: {}", e));
            std::process::exit(1)
        }
    };

    // parse body
    let mut body = String::new();
    match response.read_to_string(&mut body) {
        Ok(_) => {
            Some(match serde_json::from_str(&body) {
                Ok(json) => json,
                Err(e) => {
                    let (logger, _) = Logger::new("TRACE");
                    logger.error(&format!("failed to parse transaction list from Transpose: {}", e));
                    std::process::exit(1)
                }
            })
        },
        Err(e) => {
            let (logger, _) = Logger::new("TRACE");
            logger.error(&format!("failed to get transaction list from Transpose: {}", e));
            std::process::exit(1)
        }
    }
}

pub fn get_transaction_list(
    address: &String,
    api_key: &String,
    bounds: (&u128, &u128),
    logger: &Logger
) -> Vec<(u128, String)> {
    
    // get a new progress bar
    let transaction_list_progress = ProgressBar::new_spinner();
    transaction_list_progress.enable_steady_tick(Duration::from_millis(100));
    transaction_list_progress.set_style(logger.info_spinner());
    transaction_list_progress.set_message(format!("fetching transactions from '{}' .", address));
    let start_time = Instant::now();

    // build the SQL query
    let query = format!(
        "{{\"sql\":\"SELECT block_number, transaction_hash FROM  (SELECT transaction_hash, block_number FROM ethereum.transactions WHERE to_address = '{}' AND block_number BETWEEN {} AND {}  UNION  SELECT transaction_hash, block_number FROM ethereum.traces WHERE to_address = '{}' AND block_number BETWEEN {} AND {}) x\",\"parameters\":{{}},\"options\":{{}}}}",
        address,
        bounds.0,
        bounds.1,
        address,
        bounds.0,
        bounds.1
    );

    let response = match _call_transpose(query, api_key) {
        Some(response) => response,
        None => {
            logger.error(&format!("failed to get transaction list from Transpose"));
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
                    logger.error(&format!("failed to parse block_number from Transpose"));
                    std::process::exit(1)
                }
            },
            None => {
                logger.error(&format!("failed to fetch block_number from Transpose response"));
                std::process::exit(1)
            }
        };
        let transaction_hash: String = match result.get("transaction_hash") {
            Some(transaction_hash) => match transaction_hash.as_str() {
                Some(transaction_hash) => transaction_hash.to_string(),
                None => {
                    logger.error(&format!("failed to parse transaction_hash from Transpose"));
                    std::process::exit(1)
                }
            },
            None => {
                logger.error(&format!("failed to fetch transaction_hash from Transpose response"));
                std::process::exit(1)
            }
        };

        transactions.push((block_number, transaction_hash));
    }

    // sort the transactions by block number
    transactions.sort_by(|a, b| a.0.cmp(&b.0));

    transactions
}