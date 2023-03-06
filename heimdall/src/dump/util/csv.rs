use heimdall_common::{utils::strings::encode_hex, io::{file::write_lines_to_file, logging::Logger}};

use crate::dump::DumpState;

pub fn write_storage_to_csv(output_dir: &String, state: &DumpState, logger: &Logger) {
    let mut lines = {
        let mut lines = Vec::new();

        // sort by key ascending
        let mut storage_iter = state.storage.iter().collect::<Vec<_>>();
        storage_iter.sort_by_key(|(slot, _)| *slot);

        for (slot, slot_data) in storage_iter {
            lines.push(
                format!(
                    "{},{},{},{}",
                    encode_hex(slot.to_fixed_bytes().into()),
                    encode_hex(slot_data.value.to_fixed_bytes().into()),
                    slot_data.modifiers.iter().max_by_key(|m| m.0).unwrap().0.to_string(),
                    slot_data.alias.as_ref().unwrap_or(&String::from("None"))
                )
            );
        }
        lines
    };

    // add header
    lines.insert(0, String::from("slot,value,last_modified,alias"));

    // save to file
    write_lines_to_file(&format!("{output_dir}/storage_dump.csv"), lines);
    logger.success(&format!("wrote storage dump to to '{output_dir}/storage_dump.csv' ."));
}