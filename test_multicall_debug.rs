use alloy_dyn_abi::{DynSolValue, DynSolCall, DynSolReturns};
use alloy::primitives::{Address, U256, Selector};
use heimdall_common::utils::strings::decode_hex;

fn main() {
    // Test calldata from the failing test
    let calldata = "1749e1e3000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000813ccee6e0fc0fbc506f834122c7c082cd4c33f0000000000000000000000000000000000000000000000000000000000176a2400000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044585e33b000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

    let bytes = decode_hex(&calldata).unwrap();
    let selector = &bytes[0..4];
    let data = &bytes[4..];

    println!("Selector: 0x{}", hex::encode(selector));

    // Try to decode as (address, uint256, bytes)[]
    use alloy_dyn_abi::DynSolType;

    let tuple_type = DynSolType::Tuple(vec![
        DynSolType::Address,
        DynSolType::Uint(256),
        DynSolType::Bytes,
    ]);
    let array_type = DynSolType::Array(Box::new(tuple_type.clone()));

    println!("\nTrying to decode as: {:?}", array_type);

    match array_type.abi_decode(data) {
        Ok(decoded) => {
            println!("Successfully decoded!");
            println!("Decoded value: {:?}", decoded);

            // Check if it matches multicall pattern
            match &decoded {
                DynSolValue::Array(items) => {
                    println!("\nArray has {} items", items.len());
                    for (i, item) in items.iter().enumerate() {
                        println!("\nItem {}:", i);
                        if let DynSolValue::Tuple(tuple_items) = item {
                            println!("  Tuple has {} elements", tuple_items.len());
                            for (j, elem) in tuple_items.iter().enumerate() {
                                match elem {
                                    DynSolValue::Address(addr) => println!("  [{}] Address: {:?}", j, addr),
                                    DynSolValue::Uint(val, bits) => println!("  [{}] Uint{}: {}", j, bits, val),
                                    DynSolValue::Bytes(data) => println!("  [{}] Bytes: 0x{} ({} bytes)", j, hex::encode(data), data.len()),
                                    _ => println!("  [{}] Other: {:?}", j, elem),
                                }
                            }
                        }
                    }
                }
                _ => println!("Not an array"),
            }
        }
        Err(e) => {
            println!("Failed to decode: {:?}", e);
        }
    }

    // Also try other tuple arrangements
    println!("\n\nTrying alternative decodings...");

    // Try (address, bytes)[]
    let tuple_type2 = DynSolType::Tuple(vec![
        DynSolType::Address,
        DynSolType::Bytes,
    ]);
    let array_type2 = DynSolType::Array(Box::new(tuple_type2));

    println!("\nTrying: {:?}", array_type2);
    match array_type2.abi_decode(data) {
        Ok(decoded) => println!("Success: {:?}", decoded),
        Err(e) => println!("Failed: {:?}", e),
    }
}
