use ethers::{abi::{decode, ParamType}, types::U256};
use heimdall_common::{utils::strings::{encode_hex, hex_to_ascii}, io::{file::write_lines_to_file, logging::Logger}};

use crate::dump::{DumpState, constants::DECODE_AS_TYPES};

pub fn write_storage_to_csv(output_dir: &String, file_name: &String, state: &DumpState, logger: &Logger) {
    let mut lines = {
        let mut lines = Vec::new();

        // sort by key ascending
        let mut storage_iter = state.storage.iter().collect::<Vec<_>>();
        storage_iter.sort_by_key(|(slot, _)| *slot);

        for (slot, value) in storage_iter {
            let decoded_value = match value.decode_as_type_index {
                0 => format!("0x{}", encode_hex(value.value.to_fixed_bytes().into())),
                1 => format!("{}", !value.value.is_zero()),
                2 => format!("0x{}", encode_hex(value.value.to_fixed_bytes().into()).get(24..).unwrap_or("")),
                3 => match decode(&[ParamType::String], value.value.as_bytes()) {
                    Ok(decoded) => decoded[0].to_string(),
                    Err(_) => hex_to_ascii(&encode_hex(value.value.to_fixed_bytes().into()))
                },
                4 => {
                    let decoded = U256::from_big_endian(&value.value.to_fixed_bytes());
                    format!("{}", decoded)
                },
                _ => format!("decoding error")
            };
            lines.push(
                format!(
                    "{},{},{},{},{}",
                    value.modifiers.iter().max_by_key(|m| m.0).unwrap().0.to_string(),
                    value.alias.as_ref().unwrap_or(&String::from("None")),
                    encode_hex(slot.to_fixed_bytes().into()),
                    DECODE_AS_TYPES[value.decode_as_type_index],
                    decoded_value,
                )
            );
        }
        lines
    };

    // add header
    lines.insert(0, String::from("last_modified,alias,slot,decoded_type,value"));

    // save to file
    write_lines_to_file(&format!("{output_dir}/{file_name}"), lines);
    logger.success(&format!("wrote storage dump to to '{output_dir}/{file_name}' ."));
}