use clap_verbosity_flag::Verbosity;
use heimdall_core::decompile::DecompilerArgs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = heimdall_core::decompile::decompile(DecompilerArgs {
        target: String::from("0x9f00c43700bc0000Ff91bE00841F8e04c0495000"),
        verbose: Verbosity::new(0, 0),
        rpc_url: String::from("https://eth.llamarpc.com"),
        default: true,
        skip_resolving: true,
        include_solidity: true,
        include_yul: false,
    })
    .await?;

    println!("Decompiled contract: {:?}", result);

    Ok(())
}
