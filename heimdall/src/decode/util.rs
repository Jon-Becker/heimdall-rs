use ethers::types::Transaction;
use heimdall_cache::util::encode_hex;
use heimdall_common::io::logging::Logger;

pub fn get_explanation(
    decoded: String,
    transaction: Transaction,
    openai_api_key: &String,
    logger: &Logger,
) -> Option<String> {
    let prompt = format!(
        "Using your knowledge of Ethereum ABIs, explain in human terms what this call may be doing.
        Be detailed, yet concise.

        Transaction From: 0x{}
        Transaction To (Interacted With): 0x{}
        Transaction Value (wei): {}
        \n{}\n\nTransaction explanation:",
        encode_hex(transaction.from.to_fixed_bytes().to_vec()),
        match transaction.to {
            Some(to) => encode_hex(to.to_fixed_bytes().to_vec()),
            None => String::from("0x0000000000000000000000000000000000000000"),
        },
        transaction.value,
        decoded
    );
    heimdall_common::resources::openai::complete(prompt, openai_api_key, logger)
}
