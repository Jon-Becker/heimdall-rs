use ethers::{
    abi::{decode, ParamType},
    types::U256,
};
use heimdall_common::utils::{
    io::file::write_lines_to_file,
    strings::{encode_hex, hex_to_ascii},
};

use crate::{
    dump::{constants::DECODE_AS_TYPES, structures::dump_state::DumpState},
    error::Error,
};

/// A single row in the CSV
#[derive(Debug, Clone)]
pub struct DumpRow {
    pub last_modified: String,
    pub alias: String,
    pub slot: String,
    pub decoded_type: String,
    pub value: String,
}

/// Convert [`DumpState`] to a Vec of [`DumpRow`]s, which can be used to build a CSV.
pub fn build_csv(state: &DumpState) -> Vec<DumpRow> {
    let mut lines: Vec<DumpRow> = Vec::new();

    // sort by key ascending
    let mut storage_iter = state.storage.iter().collect::<Vec<_>>();
    storage_iter.sort_by_key(|(slot, _)| *slot);

    for (slot, value) in storage_iter {
        let decoded_value = match value.decode_as_type_index {
            0 => format!("0x{}", encode_hex(value.value.to_fixed_bytes().into())),
            1 => format!("{}", !value.value.is_zero()),
            2 => format!(
                "0x{}",
                encode_hex(value.value.to_fixed_bytes().into()).get(24..).unwrap_or("")
            ),
            3 => match decode(&[ParamType::String], value.value.as_bytes()) {
                Ok(decoded) => decoded[0].to_string(),
                Err(_) => hex_to_ascii(&encode_hex(value.value.to_fixed_bytes().into()))
                    .unwrap_or("decoding error".to_string()),
            },
            4 => {
                let decoded = U256::from_big_endian(&value.value.to_fixed_bytes());
                format!("{decoded}")
            }
            _ => "decoding error".to_string(),
        };

        lines.push(DumpRow {
            last_modified: value
                .modifiers
                .iter()
                .max_by_key(|m| m.0)
                .map(|m| m.0.to_string())
                .unwrap_or("None".to_string()),
            alias: value.alias.as_ref().unwrap_or(&String::from("None")).to_string(),
            slot: encode_hex(slot.to_fixed_bytes().into()),
            decoded_type: DECODE_AS_TYPES[value.decode_as_type_index].to_string(),
            value: decoded_value,
        })
    }
    lines
}

/// Write the storage to a CSV file.
pub fn write_storage_to_csv(
    output_dir: &str,
    file_name: &str,
    state: &DumpState,
) -> Result<(), Error> {
    let mut csv_rows = build_csv(state);
    let mut lines: Vec<String> = Vec::new();

    // sort by last modified descending
    csv_rows.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));

    // add header
    lines.push(String::from("last_modified,alias,slot,decoded_type,value"));

    // add rows
    for row in csv_rows {
        lines.push(format!(
            "{},{},{},{},{}",
            row.last_modified, row.alias, row.slot, row.decoded_type, row.value
        ));
    }

    // write to file
    write_lines_to_file(&format!("{}/{}", output_dir, file_name), lines)
        .map_err(|e| Error::Generic(format!("failed to write to file: {}", e)))?;

    Ok(())
}
