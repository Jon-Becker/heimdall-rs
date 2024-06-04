use heimdall_decompiler::{decompile, DecompilerArgsBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = DecompilerArgsBuilder::new()
        .target("0x9f00c43700bc0000Ff91bE00841F8e04c0495000".to_string())
        .rpc_url("https://eth.llamarpc.com".to_string())
        .build()?;

    let result = decompile(args).await?;

    println!("Decompiled contract: {:#?}", result);

    Ok(())
}
