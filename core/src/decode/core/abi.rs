use std::{cmp::Ordering, collections::HashSet};

use ethers::types::U256;
use heimdall_common::{
    debug_max,
    ether::evm::core::types::{
        get_padding, get_padding_size, get_potential_types_for_word, Padding,
    },
};

use crate::error::Error;

#[derive(Debug, Clone)]
pub struct AbiEncoded {
    pub ty: String,
    pub coverages: HashSet<usize>,
}

/// Finds the offsets of all ABI-encoded items in the given calldata.
pub fn is_parameter_abi_encoded(
    parameter_index: usize,
    calldata_words: &[&str],
) -> Result<Option<AbiEncoded>, Error> {
    debug_max!("calldata_words: {:#?}", calldata_words);
    let mut coverages = HashSet::from([parameter_index]);

    // convert this word to a U256
    let (word, word_offset) = match process_and_validate_word(parameter_index, calldata_words) {
        Ok((word, word_offset)) => (word, word_offset),
        Err(_) => return Ok(None),
    };
    coverages.insert(word_offset.as_usize());

    // note: `size` is the size of the ABI-encoded item. It varies depending on the type of the
    // item.
    let size_word = calldata_words.get(word_offset.as_usize()).ok_or(Error::BoundsError)?;
    let size = U256::from_str_radix(size_word, 16)?.min(U256::from(usize::MAX));

    // check if there are enough words left in the calldata to contain the ABI-encoded item.
    // if there aren't, it doesn't necessarily mean that the calldata is invalid, but it does
    // indicate that they aren't an array of items.
    let data_start_word_offset = word_offset + 1;
    let data_end_word_offset = data_start_word_offset + size;
    match data_end_word_offset.cmp(&U256::from(calldata_words.len())) {
        Ordering::Greater => is_parameter_abi_encoded_bytes(
            parameter_index,
            calldata_words,
            word,
            word_offset,
            data_start_word_offset,
            size,
            coverages,
        ),
        _ => is_parameter_abi_encoded_array(
            parameter_index,
            calldata_words,
            word,
            word_offset,
            data_start_word_offset,
            data_end_word_offset,
            size,
            coverages,
        ),
    }
}

/// Processes a word and validates that it is could be a ptr to an ABI-encoded item.
fn process_and_validate_word(
    parameter_index: usize,
    calldata_words: &[&str],
) -> Result<(U256, U256), Error> {
    let word = U256::from_str_radix(calldata_words[parameter_index], 16)?;

    // if the word is a multiple of 32, it may be an offset pointing to the start of an
    // ABI-encoded item
    if word % 32 != U256::zero() || word == U256::zero() {
        debug_max!("parameter {}: '{}' doesnt appear to be an offset ptr", parameter_index, word);
        return Err(Error::BoundsError);
    }

    // check if the pointer is pointing to a valid location in the calldata
    let word_offset = word / 32;
    if word_offset >= U256::from(calldata_words.len()) {
        debug_max!("parameter {}: '{}' is out of bounds (offset check)", parameter_index, word);
        return Err(Error::BoundsError);
    }

    Ok((word, word_offset))
}

/// Handle ABI-encoded bytes
fn is_parameter_abi_encoded_bytes(
    parameter_index: usize,
    calldata_words: &[&str],
    word: U256,
    word_offset: U256,
    data_start_word_offset: U256,
    size: U256,
    coverages: HashSet<usize>,
) -> Result<Option<AbiEncoded>, Error> {
    let mut coverages = coverages;
    debug_max!("parameter {}: '{}' may be bytes, string", parameter_index, word);

    // join words AFTER the word_offset. these are where potential data is.
    let data_words = &calldata_words[data_start_word_offset.as_usize()..];

    // check if there are enough remaining bytes to contain the ABI-encoded item.
    if data_words.join("").len() / 2 < size.as_usize() {
        debug_max!("parameter {}: '{}' is out of bounds (bytes check)", parameter_index, word);
        return Ok(None);
    }

    // if `size` is less than 32 bytes, we only need to check the first word for `32 - size`
    // null-bytes. tricky because sometimes a null byte could be part of the
    // data.
    if size <= U256::from(32) {
        let potential_data = data_words[0];
        debug_max!("with data: {}", potential_data);

        // get the padding of the data
        let padding_size = get_padding_size(potential_data);

        // if the padding is greater than `32 - size`, then this is not an ABI-encoded item.
        if padding_size > 32 - size.as_usize() {
            debug_max!("parameter {}: '{}' with size {} cannot fit into word with padding of {} bytes (bytes)", parameter_index, word, size, padding_size);
            return Ok(None);
        }

        // insert the word offset into the coverage set
        coverages.insert(data_start_word_offset.as_usize());
        coverages.insert(word_offset.as_usize());
        debug_max!("parameter {}: '{}' is bytes", parameter_index, word);
        Ok(Some(AbiEncoded { ty: String::from("bytes"), coverages }))
    } else {
        // recalculate data_end_word_offset based on `size`
        // size is in bytes, and one word is 32 bytes. find out how many words we need to
        // cover `size` bytes.
        let word_count_for_size = U256::from((size.as_u32() as f32 / 32f32).ceil() as u32); // wont panic unless calldata is huge
        let data_end_word_offset = data_start_word_offset + word_count_for_size;
        let data_words = &calldata_words[data_start_word_offset.as_usize()..];
        debug_max!("with data: {:#?}", data_words.join(""));

        // get the last word of the data
        let last_word =
            data_words.get(word_count_for_size.as_usize() - 1).ok_or(Error::BoundsError)?;

        // how many bytes should be in the last word?
        let last_word_size = size.as_usize() % 32;

        // if the padding is greater than `32 - last_word_size`, then this is not an ABI-encoded
        // item.
        let padding_size = get_padding_size(last_word);
        if padding_size > 32 - last_word_size {
            debug_max!("parameter {}: '{}' with size {} cannot fit into last word with padding of {} bytes (bytes)", parameter_index, word, size, padding_size);
            return Ok(None);
        }

        // insert all word offsets from `data_start_word_offset` to `data_end_word_offset` into
        // the coverage set
        for i in word_offset.as_usize()..data_end_word_offset.as_usize() {
            coverages.insert(i);
        }

        debug_max!("parameter {}: '{}' is bytes", parameter_index, word);
        Ok(Some(AbiEncoded { ty: String::from("bytes"), coverages }))
    }
}

/// Handle ABI-encoded bytes
fn is_parameter_abi_encoded_array(
    parameter_index: usize,
    calldata_words: &[&str],
    word: U256,
    word_offset: U256,
    data_start_word_offset: U256,
    data_end_word_offset: U256,
    size: U256,
    coverages: HashSet<usize>,
) -> Result<Option<AbiEncoded>, Error> {
    let mut coverages = coverages;
    debug_max!("parameter {}: '{}' may be an array", parameter_index, word);
    let data_words =
        &calldata_words[data_start_word_offset.as_usize()..data_end_word_offset.as_usize()];
    debug_max!("potential array items: {:#?}", data_words);

    // check if this array is a string (of bytes)
    if let Ok(Some(abi_encoded)) = is_parameter_abi_encoded_string(
        data_words,
        parameter_index,
        calldata_words,
        word,
        word_offset,
        data_start_word_offset,
        size,
        coverages.clone(),
    ) {
        return Ok(Some(abi_encoded));
    }
    // this is an array!

    // insert all word offsets from `data_start_word_offset` to `data_end_word_offset` into the
    // coverage set
    for i in word_offset.as_usize()..data_end_word_offset.as_usize() {
        coverages.insert(i);
    }

    // get most-likely potential type of the array
    let potential_type = get_potential_type(
        data_words,
        parameter_index,
        calldata_words,
        word,
        data_start_word_offset,
        &mut coverages,
    );
    let type_str = format!("{potential_type}[]");
    debug_max!("parameter {}: '{}' is {}", parameter_index, word, type_str);
    Ok(Some(AbiEncoded { ty: type_str, coverages }))
}

/// Determine if the given word is an abi-encoded string.
fn is_parameter_abi_encoded_string(
    data_words: &[&str],
    parameter_index: usize,
    calldata_words: &[&str],
    word: U256,
    word_offset: U256,
    data_start_word_offset: U256,
    size: U256,
    coverages: HashSet<usize>,
) -> Result<Option<AbiEncoded>, Error> {
    let mut coverages = coverages;
    // check if the data words all have conforming padding
    // we do this check because strings will typically be of the form:
    // 0000000000000000000000000000000000000000000000000000000000000003 // length of 3
    // 6f6e650000000000000000000000000000000000000000000000000000000000 // "one"
    //
    // so, if the data words have conforming padding, we can assume that this is not a string
    // and is instead an array.
    let padding_matches: bool = data_words
        .iter()
        .map(|word| get_padding(word))
        .all(|padding| padding == get_padding(data_words[0]));
    if !padding_matches {
        // size is in bytes now, we just need to do the same as bytes bound checking
        debug_max!("parameter {}: '{}' may be string", parameter_index, word);

        // if `size` is less than 32 bytes, we only need to check the first word for `32 - size`
        // null-bytes. tricky because sometimes a null byte could be part of the
        // data.
        if size <= U256::from(32) {
            let potential_data = data_words[0];
            debug_max!("with data: {}", potential_data);

            // get the padding of the data
            let padding_size = get_padding_size(potential_data);

            // if the padding is greater than `32 - size`, then this is not an ABI-encoded item.
            if padding_size > 32 - size.as_usize() {
                debug_max!("parameter {}: '{}' with size {} cannot fit into word with padding of {} bytes (string)", parameter_index, word, size, padding_size);
                return Ok(None);
            }

            // yay! we have a string!
            // insert the word offset into the coverage set
            coverages.insert(data_start_word_offset.as_usize());
            coverages.insert(word_offset.as_usize());
            return Ok(Some(AbiEncoded {
                ty: String::from("string"),
                coverages: coverages.clone(),
            }));
        } else {
            // recalculate data_end_word_offset based on `size`
            // size is in bytes, and one word is 32 bytes. find out how many words we need to
            // cover `size` bytes.
            let word_count_for_size = U256::from((size.as_u32() as f32 / 32f32).ceil() as u32); // wont panic unless calldata is huge
            let data_end_word_offset = data_start_word_offset + word_count_for_size;
            debug_max!(
                "with data: {:#?}",
                calldata_words[data_start_word_offset.as_usize()..data_end_word_offset.as_usize()]
                    .join("")
            );

            // get the last word of the data
            let last_word =
                data_words.get(word_count_for_size.as_usize() - 1).ok_or(Error::BoundsError)?;

            // how many bytes should be in the last word?
            let last_word_size = size.as_usize() % 32;

            // if the padding is greater than `32 - last_word_size`, then this is not an
            // ABI-encoded item.
            let padding_size = get_padding_size(last_word);
            if padding_size > 32 - last_word_size {
                debug_max!("parameter {}: '{}' with size {} cannot fit into last word with padding of {} bytes (string)", parameter_index, word, size, padding_size);
                return Ok(None);
            }

            // yay! we have a string!
            // insert all word offsets from `data_start_word_offset` to `data_end_word_offset`
            // into the coverage set
            for i in word_offset.as_usize()..data_end_word_offset.as_usize() {
                coverages.insert(i);
            }
        }

        debug_max!("parameter {}: '{}' is string", parameter_index, word);
        return Ok(Some(AbiEncoded { ty: String::from("string"), coverages: coverages.clone() }));
    }

    Ok(None)
}

/// Handle determining the most potential type of an abi-encoded item.
fn get_potential_type(
    data_words: &[&str],
    parameter_index: usize,
    calldata_words: &[&str],
    word: U256,
    data_start_word_offset: U256,
    coverages: &mut HashSet<usize>,
) -> String {
    let (_, potential_type) = data_words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            // we need to get a slice of calldata_words from `data_start_word_offset` to the end
            // of the calldata_words. this is because nested abi-encoded items
            // reset the offsets of the words.
            let data_words = &calldata_words[data_start_word_offset.as_usize()..];

            // first, check if this word *could* be a nested abi-encoded item
            debug_max!(
                "parameter {}: '{}' checking for nested abi-encoded data",
                parameter_index,
                word
            );
            if let Ok(Some(nested_abi_encoded_param)) = is_parameter_abi_encoded(i, data_words) {
                // we need to add data_start_word_offset to all the offsets in nested_coverages
                // because they are relative to the start of the nested abi-encoded item.
                let nested_coverages = nested_abi_encoded_param
                    .coverages
                    .into_iter()
                    .map(|nested_coverage| nested_coverage + data_start_word_offset.as_usize())
                    .collect::<HashSet<usize>>();

                // merge coverages and nested_coverages
                coverages.extend(nested_coverages);
                return (32, vec![nested_abi_encoded_param.ty]);
            }

            let (padding_size, mut potential_types) = get_potential_types_for_word(w);

            // perform heuristics
            // - if we use right-padding, this is probably bytesN
            // - if we use left-padding, this is probably uintN or intN
            // - if we use no padding, this is probably bytes32
            match get_padding(w) {
                Padding::Left => {
                    potential_types.retain(|t| t.starts_with("uint") || t.starts_with("address"))
                }
                _ => potential_types.retain(|t| t.starts_with("bytes") || t.starts_with("string")),
            }

            (padding_size, potential_types)
        })
        .fold((0, String::from("")), |(max_size, mut potential_type), (size, types)| {
            // "address" and "string" are priority types
            if types.contains(&String::from("string")) {
                return (32, String::from("string"));
            } else if types.contains(&String::from("address")) {
                return (32, String::from("address"));
            }

            if size > max_size {
                potential_type = types.first().expect("types is empty").clone();
                (max_size, potential_type)
            } else {
                (max_size, potential_type)
            }
        });

    potential_type
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
            let abi_encoded_params = is_parameter_abi_encoded(i, &calldata_words).unwrap();
            let is_abi_encoded = abi_encoded_params.is_some();
            let coverages = abi_encoded_params.map(|p| p.coverages).unwrap_or_default();

            println!(
                "{i} - is_abi_encoded: {}, with word coverage: {:?}",
                is_abi_encoded, coverages
            );

            if i == 1 {
                assert!(is_abi_encoded);
                assert_eq!(coverages, HashSet::from([1, 4, 5, 6]));
            } else if i == 3 {
                assert!(is_abi_encoded);
                assert_eq!(coverages, HashSet::from([3, 7, 8]));
            } else {
                assert!(!is_abi_encoded);
                assert_eq!(coverages, HashSet::new());
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
            let abi_encoded_params = is_parameter_abi_encoded(i, &calldata_words).unwrap();
            let is_abi_encoded = abi_encoded_params.is_some();
            let coverages = abi_encoded_params.map(|p| p.coverages).unwrap_or_default();

            println!(
                "{i} - is_abi_encoded: {}, with word coverage: {:?}",
                is_abi_encoded, coverages
            );

            if i == 1 {
                assert!(is_abi_encoded);
                assert_eq!(coverages, HashSet::from([1, 4, 5, 6]));
            } else if i == 3 {
                assert!(is_abi_encoded);
                assert_eq!(coverages, HashSet::from([3, 7, 8, 9]));
            } else {
                assert!(!is_abi_encoded);
                assert_eq!(coverages, HashSet::new());
            }
        }
    }

    #[test]
    fn test_detect_abi_encoded_complex() {
        // calldata from https://docs.soliditylang.org/en/develop/abi-spec.html
        // * Signature: `g(uint256[][],string[])`
        // * Values: `([[1, 2], [3]], ["one", "two", "three"])`
        //   - **Coverages**
        //     - uint256[][]: [0, 2, 3, 4, 5, 6, 7, 8, 9]
        //     - string[]:    [1, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]
        // 0x2289b18c
        //  0. 0000000000000000000000000000000000000000000000000000000000000040
        //  1. 0000000000000000000000000000000000000000000000000000000000000140
        //  2. 0000000000000000000000000000000000000000000000000000000000000002
        //  3. 0000000000000000000000000000000000000000000000000000000000000040
        //  4. 00000000000000000000000000000000000000000000000000000000000000a0
        //  5. 0000000000000000000000000000000000000000000000000000000000000002
        //  6. 0000000000000000000000000000000000000000000000000000000000000001
        //  7. 0000000000000000000000000000000000000000000000000000000000000002
        //  8. 0000000000000000000000000000000000000000000000000000000000000001
        //  9. 0000000000000000000000000000000000000000000000000000000000000003
        // 10. 0000000000000000000000000000000000000000000000000000000000000003
        // 11. 0000000000000000000000000000000000000000000000000000000000000060
        // 12. 00000000000000000000000000000000000000000000000000000000000000a0
        // 13. 00000000000000000000000000000000000000000000000000000000000000e0
        // 14. 0000000000000000000000000000000000000000000000000000000000000003
        // 15. 6f6e650000000000000000000000000000000000000000000000000000000000
        // 16. 0000000000000000000000000000000000000000000000000000000000000003
        // 17. 74776f0000000000000000000000000000000000000000000000000000000000
        // 18. 0000000000000000000000000000000000000000000000000000000000000005
        // 19. 7468726565000000000000000000000000000000000000000000000000000000
        let calldata = "0x2289b18c000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001400000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000e000000000000000000000000000000000000000000000000000000000000000036f6e650000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000374776f000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000057468726565000000000000000000000000000000000000000000000000000000";

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
            let abi_encoded_params = is_parameter_abi_encoded(i, &calldata_words).unwrap();
            let is_abi_encoded = abi_encoded_params.is_some();
            let coverages = abi_encoded_params.clone().map(|p| p.coverages).unwrap_or_default();
            let ty = abi_encoded_params.map(|p| p.ty).unwrap_or_default();

            println!(
                "{i} - is_abi_encoded: {}, ty: {:?}, with word coverage: {:?}",
                is_abi_encoded, ty, coverages
            );

            if i == 0 {
                assert!(is_abi_encoded);
                assert_eq!(coverages, HashSet::from([0, 2, 3, 4, 5, 6, 7, 8, 9]));
            } else if i == 1 {
                assert!(is_abi_encoded);
                assert_eq!(coverages, HashSet::from([1, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]));
            }
        }
    }

    #[test]
    fn test_detect_abi_encoded_mixed() {
        // calldata from https://docs.soliditylang.org/en/develop/abi-spec.html
        // * Signature: `sam(bytes,bool,uint256[])`
        // * Values: `("dave", true, [1, 2, 3])`
        //   - **Coverages**
        //     - bytes:     [0, 3, 4]
        //     - bool:      [1]
        //     - uint256[]: [2, 5, 6, 7, 8]
        // 0xa5643bf2
        // 0. 0000000000000000000000000000000000000000000000000000000000000060-
        // 1. 0000000000000000000000000000000000000000000000000000000000000001-
        // 2. 00000000000000000000000000000000000000000000000000000000000000a0-
        // 3. 0000000000000000000000000000000000000000000000000000000000000004-
        // 4. 6461766500000000000000000000000000000000000000000000000000000000-
        // 5. 0000000000000000000000000000000000000000000000000000000000000003
        // 6. 0000000000000000000000000000000000000000000000000000000000000001
        // 7. 0000000000000000000000000000000000000000000000000000000000000002
        // 8. 0000000000000000000000000000000000000000000000000000000000000003
        let calldata = "0xa5343bf20000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000000464617665000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000003";

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
            let abi_encoded_params = is_parameter_abi_encoded(i, &calldata_words).unwrap();
            let is_abi_encoded = abi_encoded_params.is_some();
            let coverages = abi_encoded_params.clone().map(|p| p.coverages).unwrap_or_default();
            let ty = abi_encoded_params.map(|p| p.ty).unwrap_or_default();

            println!(
                "{i} - is_abi_encoded: {}, ty: {:?}, with word coverage: {:?}",
                is_abi_encoded, ty, coverages
            );

            if i == 0 {
                assert!(is_abi_encoded);
                assert_eq!(coverages, HashSet::from([0, 3, 4]));
            } else if i == 1 {
                assert!(!is_abi_encoded);
            } else if i == 2 {
                assert!(is_abi_encoded);
                assert_eq!(coverages, HashSet::from([2, 5, 6, 7, 8]));
            }
        }
    }
}
