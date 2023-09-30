use heimdall_core::decode::DecodeArgsBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = heimdall_core::decode::decode(
        DecodeArgsBuilder::new()
            .target("0x9f00c43700bc0000Ff91bE00841F8e04c0495000".to_string())
            .rpc_url("https://eth.llamarpc.com".to_string())
            .build()?,
    )
    .await?;

    Ok(())
}
