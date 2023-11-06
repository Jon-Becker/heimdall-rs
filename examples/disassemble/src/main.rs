use heimdall_core::disassemble::DisassemblerArgsBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = heimdall_core::disassemble::disassemble(
        DisassemblerArgsBuilder::new()
            .target("0x9f00c43700bc0000Ff91bE00841F8e04c0495000".to_string())
            .rpc_url("https://eth.llamarpc.com".to_string())
            .build()?,
    )
    .await?;

    println!("Disassembled contract: {:#?}", result);

    Ok(())
}
