use heimdall_decoder::{decode, DecodeArgs};

#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::init();

    let args = DecodeArgs {
        target: String::from("0x1749e1e3000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000813ccee6e0fc0fbc506f834122c7c082cd4c33f0000000000000000000000000000000000000000000000000000000000176a2400000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044585e33b000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"),
        rpc_url: String::from(""),
        abi: None,
        openai_api_key: String::from(""),
        explain: false,
        default: true,
        constructor: false,
        truncate_calldata: false,
        skip_resolving: false,
        raw: true,
        output: String::from("json"),
    };

    match decode(args).await {
        Ok(result) => {
            println!("Decode successful!");
            println!("Signature: {}", result.decoded.signature);
            println!("Has multicall results: {}", result.multicall_results.is_some());
            if let Some(mc) = &result.multicall_results {
                println!("Multicall results count: {}", mc.len());
            }
        }
        Err(e) => {
            println!("Decode failed: {:?}", e);
        }
    }
}
