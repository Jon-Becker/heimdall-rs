use ethers::types::Transaction;
use heimdall_common::utils::strings::encode_hex;

/// Get an explanation of the decoded transaction using the OpenAI API
pub async fn get_explanation(
    decoded: String,
    transaction: Transaction,
    openai_api_key: &str,
) -> Option<String> {
    // create the prompt
    let prompt = format!(
        "You are an expert in Ethereum, the EVM, and DeFi protocols. You will be given the following information:

        - The transaction sender (From)
        - The contract or account the transaction is interacting with (To)
        - The value of the transaction in wei (which is the smallest unit of Ether, Ethereum's native cryptocurrency)
        - The decoded transaction data

        Use this information to explain the transaction in a way that a non-technical person can understand.

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
    heimdall_common::resources::openai::complete(&prompt, openai_api_key).await
}
