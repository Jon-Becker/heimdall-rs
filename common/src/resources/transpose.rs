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

fn _call_transpose(endpoint: String, api_key: &String, logger: &Logger) -> Option<TransposeResponse> {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "application/json".parse().unwrap());
    headers.insert("X-API-KEY", api_key.parse().unwrap());

    // make the request
    let client = reqwest::blocking::Client::builder().redirect(reqwest::redirect::Policy::none()).build() .unwrap();
    let mut response = match client
        .get(format!("https://api.transpose.io/endpoint/{endpoint}"))
        .body("{\"options\":{\"timeout\": 999999999}}")
        .headers(headers)
        .send()
    {
        Ok(res) => res,
        Err(e) => {
            logger.error(&format!("failed to call Transpose endpoint '{endpoint}' ."));
            logger.error(&format!("error: {}", e));
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
                    logger.error(&format!("Transpose request unsucessful."));
                    logger.debug(&format!("curl: curl -X GET \"https://api.transpose.io/endpoint/{endpoint}\" -H \"accept: application/json\" -H \"Content-Type: application/json\" -H \"X-API-KEY: {api_key}\" -d \"{{\\\"options\\\":{{\\\"timeout\\\": 999999999}}}}\""));
                    logger.error(&format!("error: {}", e));
                    logger.debug(&format!("response body: {:?}", body));
                    std::process::exit(1)
                }
            })
        },
        Err(e) => {
            logger.error(&format!("failed to parse Transpose response body."));
            logger.error(&format!("error: {}", e));
            logger.debug(&format!("response body: {:?}", body));
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

    let response = match _call_transpose(
        format!("get-all-transactions/1?address={}&from_block={}&to_block={}", address, bounds.0, bounds.1),
        api_key,
        logger
    ) {
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

pub fn get_contract_creation(
    address: &String,
    api_key: &String,
    logger: &Logger
) -> Option<(u128, String)> {
    
    // get a new progress bar
    let transaction_list_progress = ProgressBar::new_spinner();
    transaction_list_progress.enable_steady_tick(Duration::from_millis(100));
    transaction_list_progress.set_style(logger.info_spinner());
    transaction_list_progress.set_message(format!("fetching '{address}''s creation tx ."));
    let start_time = Instant::now();

    let response = match _call_transpose(
        format!("fast-contract-address-creation-lookup/1?contract_address={address}"),
        api_key,
        logger
    ) {
        Some(response) => response,
        None => {
            logger.error(&format!("failed to get creation tx from Transpose"));
            std::process::exit(1)
        }
    };

    transaction_list_progress.finish_and_clear();
    logger.debug(&format!("fetching contract creation took {:?}", start_time.elapsed()));

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

        return Some((block_number, transaction_hash));
    };

    None
}