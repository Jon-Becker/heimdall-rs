use heimdall_core::snapshot::SnapshotArgsBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = heimdall_core::snapshot::snapshot(
        SnapshotArgsBuilder::new()
            .target("0x9f00c43700bc0000Ff91bE00841F8e04c0495000".to_string())
            .rpc_url("https://eth.llamarpc.com".to_string())
            .build()?,
    )
    .await?;

    println!("Snapshot contract: {:#?}", result);

    Ok(())
}
