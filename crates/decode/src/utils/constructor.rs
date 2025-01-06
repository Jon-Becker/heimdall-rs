use eyre::{eyre, OptionExt, Result};
use heimdall_common::{
    constants::CONSTRUCTOR_REGEX,
    utils::strings::{decode_hex, encode_hex},
};

#[derive(Debug, Clone)]
pub(crate) struct Constructor {
    pub _constructor: Vec<u8>,
    pub _contract: Vec<u8>,
    pub _metadata: Vec<u8>,
    pub arguments: Vec<u8>,
}

pub(crate) fn parse_deployment_bytecode(input: Vec<u8>) -> Result<Constructor> {
    // convert input to a hex string
    let input = encode_hex(&input);

    //
    let input = input.to_lowercase().replace("0x", "");

    let captures = CONSTRUCTOR_REGEX
        .captures(&input)
        .map_err(|_| eyre!("Failed to parse constructor regex"))?
        .ok_or_eyre("nonstandard constructor bytecode, or no constructor arguments exist")?;

    let contract_length = captures
        .get(1)
        .or_else(|| captures.get(2))
        .or_else(|| captures.get(3))
        .ok_or_eyre("Contract length not found")?
        .as_str();
    let contract_length = u32::from_str_radix(contract_length, 16)? * 2;

    let contract_offset = captures
        .get(4)
        .or_else(|| captures.get(5))
        .or_else(|| captures.get(6))
        .ok_or_eyre("Contract offset not found")?
        .as_str();
    let contract_offset = u32::from_str_radix(contract_offset, 16)? * 2;

    let constructor_offset = 0;
    let metadata_length = u32::from_str_radix(
        &input[(contract_offset + contract_length - 4) as usize..
            (contract_offset + contract_length) as usize],
        16,
    )? * 2 +
        4;

    let constructor = &input[constructor_offset as usize..contract_offset as usize];
    let contract = &input[contract_offset as usize..(contract_offset + contract_length) as usize];
    let metadata = &input[(contract_offset + contract_length - metadata_length) as usize..
        (contract_offset + contract_length) as usize];
    let arguments = &input[(contract_offset + contract_length) as usize..];

    Ok(Constructor {
        _constructor: decode_hex(constructor)?,
        _contract: decode_hex(contract)?,
        _metadata: decode_hex(metadata)?,
        arguments: decode_hex(arguments)?,
    })
}
