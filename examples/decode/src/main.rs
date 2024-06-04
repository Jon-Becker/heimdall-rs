use heimdall_decoder::{decode, DecodeArgsBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = DecodeArgsBuilder::new()
        .target("0xc47f00270000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000b6a6265636b65722e657468000000000000000000000000000000000000000000".to_string())
        .rpc_url("https://eth.llamarpc.com".to_string())
        .build()?;

    let result = decode(args).await?;

    println!("Decode result: {:#?}", result.decoded);

    Ok(())
}
