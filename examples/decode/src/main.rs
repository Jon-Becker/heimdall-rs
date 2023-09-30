use heimdall_core::decode::DecodeArgsBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = heimdall_core::decode::decode(
        DecodeArgsBuilder::new()
            .target("0xc47f00270000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000b6a6265636b65722e657468000000000000000000000000000000000000000000".to_string())
            .rpc_url("https://eth.llamarpc.com".to_string())
            .build()?,
    )
    .await?;

    println!("Decode result: {:#?}", result);

    Ok(())
}
