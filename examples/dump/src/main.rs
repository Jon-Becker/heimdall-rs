use heimdall_dump::{dump, DumpArgsBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = DumpArgsBuilder::new()
        .target("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_string())
        .rpc_url("https://eth.llamarpc.com".to_string())
        .from_block(10000000)
        .to_block(Some(10000001))
        .build()?;

    let result = dump(args).await?;

    println!("Contract Storage: {:#?}", result);

    Ok(())
}
