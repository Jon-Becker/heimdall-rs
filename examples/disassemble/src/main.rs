use heimdall_disassembler::{disassemble, DisassemblerArgsBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = DisassemblerArgsBuilder::new()
        .target("0x9f00c43700bc0000Ff91bE00841F8e04c0495000".to_string())
        .rpc_url("https://eth.llamarpc.com".to_string())
        .build()?;

    let result = disassemble(args).await?;

    println!("Disassembled contract: {:#?}", result);

    Ok(())
}
