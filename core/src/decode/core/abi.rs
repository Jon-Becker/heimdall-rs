use std::collections::HashSet;

use ethers::types::U256;
use heimdall_common::ether::evm::core::types::{
    get_padding, get_padding_size, get_potential_types_for_word, Padding,
};

/// Finds the offsets of all ABI-encoded items in the given calldata.
pub fn is_parameter_abiencoded(
    parameter_index: usize,
    calldata_words: &[&str],
) -> (bool, Option<String>, Option<HashSet<usize>>) {
    let mut coverages = HashSet::from([parameter_index]);

    // convert this word to a U256
    // TODO: this can panic. make this entire function a `Result`
    let word = U256::from_str_radix(calldata_words[parameter_index], 16).unwrap();

    // if the word is a multiple of 32, it may be an offset pointing to the start of an
    // ABI-encoded item
    if word % 32 != U256::zero() {
        return (false, None, None);
    }

    // check if the pointer is pointing to a valid location in the calldata
    let word_offset = word / 32;
    if word_offset >= U256::from(calldata_words.len()) {
        return (false, None, None);
    }

    // note: `size` is the size of the ABI-encoded item. It varies depending on the type of the
    // item.
    let size = U256::from_str_radix(
        calldata_words.get(word_offset.as_usize()).expect("word_offset out of bounds"),
        16,
    )
    .unwrap();

    // check if there are enough words left in the calldata to contain the ABI-encoded item.
    // if there aren't, it doesn't necessarily mean that the calldata is invalid, but it does
    // indicate that they aren't an array of items.
    let data_start_word_offset = word_offset + 1;
    let data_end_word_offset = data_start_word_offset + size;
    if data_end_word_offset >= U256::from(calldata_words.len()) {
        // this could still be bytes, string, or a non ABI-encoded item

        // join words AFTER the word_offset. these are where potential data is.
        let data_words = &calldata_words[data_start_word_offset.as_usize()..];

        // check if there are enough remaining bytes to contain the ABI-encoded item.
        if data_words.join("").len() / 2 < size.as_usize() {
            return (false, None, None);
        }

        // if `size` is less than 32 bytes, we only need to check the first word for `32 - size`
        // null-bytes. tricky because sometimes a null byte could be part of the
        // data.
        if size <= U256::from(32) {
            let potential_data = data_words[0];

            // get the padding of the data
            let padding_size = get_padding_size(potential_data);

            // if the padding is greater than `32 - size`, then this is not an ABI-encoded item.
            if padding_size > 32 - size.as_usize() {
                return (false, None, None);
            }

            // insert the word offset into the coverage set
            coverages.insert(data_start_word_offset.as_usize());
            coverages.insert(word_offset.as_usize());
            (true, Some(String::from("bytes")), Some(coverages))
        } else {
            // recalculate data_end_word_offset based on `size`
            // size is in bytes, and one word is 32 bytes. find out how many words we need to
            // cover `size` bytes.
            let word_count_for_size = U256::from((size.as_u32() as f32 / 32f32).ceil() as u32); // wont panic unless calldata is huge
            let data_end_word_offset = data_start_word_offset + word_count_for_size;

            // get the last word of the data
            let last_word = data_words
                .get(word_count_for_size.as_usize() - 1)
                .expect("word_count_for_size out of bounds");

            // how many bytes should be in the last word?
            let last_word_size = size.as_usize() % 32;

            // if the padding is greater than `32 - last_word_size`, then this is not an ABI-encoded
            // item.
            let padding_size = get_padding_size(last_word);
            if padding_size > 32 - last_word_size {
                return (false, None, None);
            }

            // insert all word offsets from `data_start_word_offset` to `data_end_word_offset` into
            // the coverage set
            for i in word_offset.as_usize()..data_end_word_offset.as_usize() {
                coverages.insert(i);
            }

            (true, Some(String::from("bytes")), Some(coverages))
        }
    } else {
        // this could be an array of items.
        let data_words =
            &calldata_words[data_start_word_offset.as_usize()..data_end_word_offset.as_usize()];

        let (_min_size, potential_type) = data_words
            .iter()
            .map(|w| {
                let (padding_size, mut potential_types) = get_potential_types_for_word(w);

                // perform heuristics
                // - if we use right-padding, this is probably bytesN
                // - if we use left-padding, this is probably uintN or intN
                // - if we use no padding, this is probably bytes32
                match get_padding(w) {
                    Padding::Left => potential_types.retain(|t| t.starts_with("uint")),
                    _ => potential_types.retain(|t| t.starts_with("bytes")),
                }

                (padding_size, potential_types)
            })
            .fold((0, String::from("")), |(max_size, mut potential_types), (size, types)| {
                if size > max_size {
                    potential_types = types
                        .first()
                        .expect("potential types is empty when decoding abi.encoded array")
                        .clone();
                    (size, potential_types)
                } else {
                    (max_size, potential_types)
                }
            });

        // insert all word offsets from `data_start_word_offset` to `data_end_word_offset` into the
        // coverage set
        for i in word_offset.as_usize()..data_end_word_offset.as_usize() {
            coverages.insert(i);
        }
        (true, Some(format!("{potential_type}[]")), Some(coverages))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_abi_encoded_parameters_nominal() {
        // calldata from https://docs.soliditylang.org/en/develop/abi-spec.html
        // * Signature: `f(uint256,uint32[],bytes10,bytes)`
        // * Values: `(0x123, [0x456, 0x789], "1234567890", "Hello, world!")`
        // 0x8be65246
        // 0. 0000000000000000000000000000000000000000000000000000000000000123
        // 1. 0000000000000000000000000000000000000000000000000000000000000080
        // 2. 3132333435363738393000000000000000000000000000000000000000000000
        // 3. 00000000000000000000000000000000000000000000000000000000000000e0
        // 4. 0000000000000000000000000000000000000000000000000000000000000002
        // 5. 0000000000000000000000000000000000000000000000000000000000000456
        // 6. 0000000000000000000000000000000000000000000000000000000000000789
        // 7. 000000000000000000000000000000000000000000000000000000000000000d
        // 8. 48656c6c6f2c20776f726c642100000000000000000000000000000000000000
        let calldata = "0x8be6524600000000000000000000000000000000000000000000000000000000000001230000000000000000000000000000000000000000000000000000000000000080313233343536373839300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e0000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000004560000000000000000000000000000000000000000000000000000000000000789000000000000000000000000000000000000000000000000000000000000000d48656c6c6f2c20776f726c642100000000000000000000000000000000000000";

        let calldata = &calldata[10..];

        // chunk in blocks of 32 bytes (64 hex chars)
        let calldata_words = calldata
            .as_bytes()
            .chunks(64)
            .map(|chunk| {
                let s = std::str::from_utf8(chunk).unwrap();
                s
            })
            .collect::<Vec<&str>>();

        for i in 0..calldata_words.len() {
            let (is_abi_encoded, _, coverages) = is_parameter_abiencoded(i, &calldata_words);
            println!(
                "{i} - is_abi_encoded: {}, with word coverage: {:?}",
                is_abi_encoded, coverages
            );

            if i == 1 {
                assert!(is_abi_encoded);
                assert_eq!(coverages, Some(HashSet::from([1, 4, 5, 6])));
            } else if i == 3 {
                assert!(is_abi_encoded);
                assert_eq!(coverages, Some(HashSet::from([3, 7, 8])));
            } else {
                assert!(!is_abi_encoded);
                assert_eq!(coverages, None);
            }
        }
    }
    #[test]
    fn test_detect_abi_encoded_parameters_2() {
        // calldata from https://docs.soliditylang.org/en/develop/abi-spec.html
        // * Signature: `f(uint256,uint32[],bytes10,bytes)`
        // * Values: `(0x123, [0x456, 0x789], "1234567890", "Hello, world!")`
        // 0x8be65246
        // 0. 0000000000000000000000000000000000000000000000000000000000000123
        // 1. 0000000000000000000000000000000000000000000000000000000000000080
        // 2. 3132333435363738393000000000000000000000000000000000000000000000
        // 3. 00000000000000000000000000000000000000000000000000000000000000e0
        // 4. 0000000000000000000000000000000000000000000000000000000000000002
        // 5. 0000000000000000000000000000000000000000000000000000000000000456
        // 6. 0000000000000000000000000000000000000000000000000000000000000789
        // 7. 000000000000000000000000000000000000000000000000000000000000002d
        // 8. 48656c6c6f2c20776f726c642148656c6c6f2c20776f726c642148656c6c6f2c
        // 9. 48656c6c6f2c20776f726c642100000000000000000000000000000000000000
        let calldata = "0x8be6524600000000000000000000000000000000000000000000000000000000000001230000000000000000000000000000000000000000000000000000000000000080313233343536373839300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e0000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000004560000000000000000000000000000000000000000000000000000000000000789000000000000000000000000000000000000000000000000000000000000002d48656c6c6f2c20776f726c642148656c6c6f2c20776f726c642148656c6c6f2c48656c6c6f2c20776f726c642100000000000000000000000000000000000000";

        let calldata = &calldata[10..];

        // chunk in blocks of 32 bytes (64 hex chars)
        let calldata_words = calldata
            .as_bytes()
            .chunks(64)
            .map(|chunk| {
                let s = std::str::from_utf8(chunk).unwrap();
                s
            })
            .collect::<Vec<&str>>();

        for i in 0..calldata_words.len() {
            let (is_abi_encoded, _, coverages) = is_parameter_abiencoded(i, &calldata_words);
            println!(
                "{i} - is_abi_encoded: {}, with word coverage: {:?}",
                is_abi_encoded, coverages
            );

            if i == 1 {
                assert!(is_abi_encoded);
                assert_eq!(coverages, Some(HashSet::from([1, 4, 5, 6])));
            } else if i == 3 {
                assert!(is_abi_encoded);
                assert_eq!(coverages, Some(HashSet::from([3, 7, 8, 9])));
            } else {
                assert!(!is_abi_encoded);
                assert_eq!(coverages, None);
            }
        }
    }
}
