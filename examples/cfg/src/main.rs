use heimdall_cfg::{cfg, CfgArgsBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CfgArgsBuilder::new()
        .target("0x9f00c43700bc0000Ff91bE00841F8e04c0495000".to_string())
        .rpc_url("https://eth.llamarpc.com".to_string())
        .build()?;

    let result = cfg(args).await?;

    println!("Contract Cfg: {:#?}", result);

    Ok(())
}
