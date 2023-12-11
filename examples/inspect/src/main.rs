use heimdall_core::inspect::InspectArgsBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = heimdall_core::inspect::inspect(
        InspectArgsBuilder::new()
            .target("0xa5f676d0ee4c23cc1ccb0b802be5aaead5827a3337c06e9da8b0a85dfa3e7dd5".to_string())
            .rpc_url("https://eth.llamarpc.com".to_string())
            .build()?,
    )
    .await?;

    println!("InspectResult: {:#?}", result);

    Ok(())
}
