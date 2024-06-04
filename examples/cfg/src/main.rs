use heimdall_cfg::{cfg, CFGArgsBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CFGArgsBuilder::new()
        .target("0x9f00c43700bc0000Ff91bE00841F8e04c0495000".to_string())
        .rpc_url("https://eth.llamarpc.com".to_string())
        .build()?;

    let result = cfg(args).await?;

    println!("Contract CFG: {:#?}", result);

    Ok(())
}
