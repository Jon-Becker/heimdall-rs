use hashbrown::HashSet;

use alloy::primitives::{Selector, U256};
use alloy_dyn_abi::{DynSolCall, DynSolReturns, DynSolType, DynSolValue};
use alloy_json_abi::Param;
use eyre::eyre;
use heimdall_common::utils::strings::encode_hex;
use heimdall_vm::core::types::{
    get_padding, get_padding_size, get_potential_types_for_word, Padding,
};
use tracing::trace;

use crate::error::Error;

#[derive(Debug, Clone)]
pub(crate) struct AbiEncoded {
    pub ty: String,
    pub coverages: HashSet<usize>,
}

/// Attempt to decode the given calldata with the given types.
pub(crate) fn try_decode(
    inputs: &[DynSolType],
    byte_args: &[u8],
) -> Result<(Vec<DynSolValue>, Vec<Param>), Error> {
    trace!("try_decode: inputs={:?}, byte_args.len()={}", inputs, byte_args.len());

    // For non-standard sized calldata that isn't a multiple of 32 bytes,
    // we need to pad it to ensure proper ABI decoding
    let padded_args = if !byte_args.is_empty() && byte_args.len() % 32 != 0 {
        let mut padded = byte_args.to_vec();
        // Pad to the next multiple of 32 bytes
        let padding_needed = 32 - (byte_args.len() % 32);
        padded.extend(vec![0u8; padding_needed]);
        trace!("padded byte_args from {} to {} bytes", byte_args.len(), padded.len());
        padded
    } else {
        byte_args.to_vec()
    };

    // convert inputs to tuple
    let ty =
        DynSolCall::new(Selector::default(), inputs.to_vec(), None, DynSolReturns::new(Vec::new()));

    let result = ty
        .abi_decode_input(&padded_args)
        .map_err(|e| Error::Eyre(eyre!("failed to decode calldata: {}", e)))?;
    // convert tokens to params
    let mut params: Vec<Param> = Vec::new();
    for (i, input) in inputs.iter().enumerate() {
        params.push(Param {
            ty: input.to_string(),
            name: format!("arg{i}"),
            components: Vec::new(),
            internal_type: None,
        });
    }

    Ok((result, params))
}

/// Finds the offsets of all ABI-encoded items in the given calldata.
pub(crate) fn try_decode_dynamic_parameter(
    parameter_index: usize,
    calldata_words: &[Vec<u8>],
) -> Result<Option<AbiEncoded>, Error> {
    trace!(
        "calldata_words: {:#?}",
        calldata_words.iter().map(|w| encode_hex(w)).collect::<Vec<String>>()
    );

    // initialize a [`HashSet<usize>`] called `word_coverages` with `parameter_index`
    // this works similarly to `covered_words`, but is used to keep track of which
    // words we've covered while attempting to ABI-decode the current word
    let mut coverages = HashSet::from([parameter_index]);

    // (1) the first validation step. this checks if the current word could be a valid
    // pointer to an ABI-encoded dynamic type. if it is not, we return None.
    let (byte_offset, word_offset) =
        match process_and_validate_word(parameter_index, calldata_words) {
            Ok((byte_offset, word_offset)) => (byte_offset, word_offset),
            Err(_) => return Ok(None),
        };

    // (3) the second validation step. this checks if the pointed-to word is a valid pointer to a
    // word in `calldata_words`. if it is not, we return an [`Error::BoundsError`].
    //
    // note: `size` is the size of the ABI-encoded item. It varies depending on the type of the
    // item. For example, the size of a `bytes` is the number of bytes in the encoded data, while
    // for a dynamic-length array, the size is the number of elements in the array.
    let size_word = calldata_words
        .get(word_offset.try_into().unwrap_or(usize::MAX))
        .ok_or(Error::BoundsError)?;
    let size = U256::from_be_slice(size_word).min(U256::from(usize::MAX));

    // (3) add the size word index to `word_coverages`, since this word is part of the ABI-encoded
    // type and should not be decoded again
    coverages.insert(word_offset.try_into().unwrap_or(usize::MAX));

    // (4) check if there are enough words left in the calldata to contain the ABI-encoded item.
    // if there aren't, it doesn't necessarily mean that the calldata is invalid, but it does
    // indicate that this type cannot be an array, since there aren't enough words left to store
    // the array elements.
    let data_start_word_offset = word_offset + U256::from(1);
    let data_end_word_offset = data_start_word_offset + size;
    if data_end_word_offset > U256::from(calldata_words.len()) {
        try_decode_dynamic_parameter_bytes(
            parameter_index,
            calldata_words,
            byte_offset,
            word_offset,
            data_start_word_offset,
            size,
            coverages,
        )
    } else {
        try_decode_dynamic_parameter_array(
            parameter_index,
            calldata_words,
            byte_offset,
            word_offset,
            data_start_word_offset,
            data_end_word_offset,
            size,
            coverages,
        )
    }
}

/// Processes a word and validates that it is could be a ptr to an ABI-encoded item.
fn process_and_validate_word(
    parameter_index: usize,
    calldata_words: &[Vec<u8>],
) -> Result<(U256, U256), Error> {
    let word = U256::from_be_slice(calldata_words[parameter_index].as_slice());

    // if the word is a multiple of 32, it may be an offset pointing to the start of an
    // ABI-encoded item
    if word % U256::from(32) != U256::ZERO || word == U256::ZERO {
        trace!("parameter {}: '{}' doesnt appear to be an offset ptr", parameter_index, word);
        return Err(Error::BoundsError);
    }

    // check if the pointer is pointing to a valid location in the calldata
    let word_offset = word / U256::from(32);
    if word_offset >= U256::from(calldata_words.len()) {
        trace!("parameter {}: '{}' is out of bounds (offset check)", parameter_index, word);
        return Err(Error::BoundsError);
    }

    Ok((word, word_offset))
}

/// Handle ABI-encoded bytes
fn try_decode_dynamic_parameter_bytes(
    parameter_index: usize,
    calldata_words: &[Vec<u8>],
    word: U256,
    word_offset: U256,
    data_start_word_offset: U256,
    size: U256,
    coverages: HashSet<usize>,
) -> Result<Option<AbiEncoded>, Error> {
    let mut coverages = coverages;
    trace!("parameter {}: '{}' may be bytes", parameter_index, word);

    // (1) join all words from `data_start_word_offset` to the end of `calldata_words`.
    // this is where the encoded data may be stored.
    let data_words = &calldata_words[data_start_word_offset.try_into().unwrap_or(usize::MAX)..];

    // (2) perform a quick validation check to see if there are enough remaining bytes
    // to contain the ABI-encoded item. If there aren't, return an [`Error::BoundsError`].
    if data_words.concat().len() < size.try_into().unwrap_or(usize::MAX) {
        trace!("parameter {}: '{}' is out of bounds (bytes check)", parameter_index, word);
        return Ok(None);
    }

    // (3) calculate how many words are needed to store the encoded data with size `size`.
    let word_count_for_size =
        U256::from((size.try_into().unwrap_or(u32::MAX) as f32 / 32f32).ceil() as u32); // wont panic unless calldata is huge
    let data_end_word_offset = data_start_word_offset + word_count_for_size;
    trace!("with data: {:#?}", encode_hex(&data_words.concat()));

    // (4) get the last word in `data_words`, so we can perform a size check. There should be
    // `size % 32` bytes in this word, and the rest should be null bytes.
    let last_word = data_words
        .get(word_count_for_size.try_into().unwrap_or(usize::MAX) - 1)
        .ok_or(Error::BoundsError)?;
    let last_word_size = size.try_into().unwrap_or(usize::MAX) % 32;

    // if the padding size of this last word is greater than `32 - last_word_size`,
    // there are too many bytes in the last word, and this is not a valid ABI-encoded type.
    // return an [`Error::BoundsError`].
    let padding_size = get_padding_size(last_word);
    if padding_size > 32 - last_word_size {
        trace!("parameter {}: '{}' with size {} cannot fit into last word with padding of {} bytes (bytes)", parameter_index, word, size, padding_size);
        return Ok(None);
    }

    // (5) we've covered all words from `data_start_word_offset` to `data_end_word_offset`,
    // so add them to `word_coverages`.
    coverages.extend(
        (word_offset.try_into().unwrap_or(usize::MAX)..
            data_end_word_offset.try_into().unwrap_or(usize::MAX))
            .collect::<Vec<usize>>(),
    );

    trace!("parameter {}: '{}' is bytes", parameter_index, word);
    Ok(Some(AbiEncoded { ty: String::from("bytes"), coverages }))
}

/// Handle ABI-encoded bytes
#[allow(clippy::too_many_arguments)]
fn try_decode_dynamic_parameter_array(
    parameter_index: usize,
    calldata_words: &[Vec<u8>],
    word: U256,
    word_offset: U256,
    data_start_word_offset: U256,
    data_end_word_offset: U256,
    size: U256,
    coverages: HashSet<usize>,
) -> Result<Option<AbiEncoded>, Error> {
    let mut coverages = coverages;
    trace!("parameter {}: '{}' may be an array", parameter_index, word);

    // (1) join all words from `data_start_word_offset` to `data_end_word_offset`. This is where
    // the encoded data may be stored.
    let data_words = &calldata_words[data_start_word_offset.try_into().unwrap_or(usize::MAX)..
        data_end_word_offset.try_into().unwrap_or(usize::MAX)];
    trace!("potential array items: {:#?}", data_words);

    // (2) first, check if this is a `string` type, since some string encodings may appear to be
    // arrays.
    if let Ok(Some(abi_encoded)) = try_decode_dynamic_parameter_string(
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

    // (3) this is not a `string` type, so we can assume that it is an array. we can extend
    // `word_coverages` with the indices of all words from `data_start_word_offset` to
    // `data_end_word_offset`, since we've now covered all words in the ABI-encoded type.
    coverages.extend(
        (word_offset.try_into().unwrap_or(usize::MAX)..
            data_end_word_offset.try_into().unwrap_or(usize::MAX))
            .collect::<Vec<usize>>(),
    );

    // (4) get the potential type of the array elements. under the hood, this function:
    //     - iterates over each word in `data_words`
    //     - checks if the word is a dynamic type by recursively calling
    //       `try_decode_dynamic_parameter`
    //         - if it is a dynamic type, we know the type of the array elements and can return it
    //         - if it is a static type, find the potential types that can represent each element in
    //           the array
    let potential_type = get_potential_type(
        data_words,
        parameter_index,
        calldata_words,
        word,
        data_start_word_offset,
        &mut coverages,
    );
    let type_str = format!("{potential_type}[]");
    trace!("parameter {}: '{}' is {}", parameter_index, word, type_str);
    Ok(Some(AbiEncoded { ty: type_str, coverages }))
}

/// Determine if the given word is an abi-encoded string.
#[allow(clippy::too_many_arguments)]
fn try_decode_dynamic_parameter_string(
    data_words: &[Vec<u8>],
    parameter_index: usize,
    calldata_words: &[Vec<u8>],
    word: U256,
    word_offset: U256,
    data_start_word_offset: U256,
    size: U256,
    coverages: HashSet<usize>,
) -> Result<Option<AbiEncoded>, Error> {
    let mut coverages = coverages;
    // (1) check if the data words all have conforming padding
    // we do this check because strings will typically be of the form:
    // 0000000000000000000000000000000000000000000000000000000000000003 // length of 3
    // 6f6e650000000000000000000000000000000000000000000000000000000000 // "one"
    //
    // so, if the data words have conforming padding, we can assume that this is not a string
    // and is instead an array.
    if data_words
        .iter()
        .map(|word| get_padding(word))
        .all(|padding| padding == get_padding(&data_words[0]))
    {
        trace!("parameter {}: '{}' is not string (conforming padding)", parameter_index, word);
        return Ok(None);
    }
    trace!("parameter {}: '{}' may be string", parameter_index, word);

    // (3) calculate how many words are needed to store the encoded data with size `size`.
    let word_count_for_size =
        U256::from((size.try_into().unwrap_or(u32::MAX) as f32 / 32f32).ceil() as u32);
    let data_end_word_offset = data_start_word_offset + word_count_for_size;
    trace!(
        "with data: {:#?}",
        encode_hex(
            &calldata_words[data_start_word_offset.try_into().unwrap_or(usize::MAX)..
                data_end_word_offset.try_into().unwrap_or(usize::MAX)]
                .concat()
        )
    );

    // (4) get the last word in `data_words`, so we can perform a size check. There should be
    // `size % 32` bytes in this word, and the rest should be null bytes.
    let last_word = data_words
        .get(word_count_for_size.try_into().unwrap_or(usize::MAX) - 1)
        .ok_or(Error::BoundsError)?;
    let last_word_size = size.try_into().unwrap_or(usize::MAX) % 32;

    // if the padding size of this last word is greater than `32 - last_word_size`,
    // there are too many bytes in the last word, and this is not a valid ABI-encoded type.
    // return an [`Error::BoundsError`].
    let padding_size = get_padding_size(last_word);
    if padding_size > 32 - last_word_size {
        trace!("parameter {}: '{}' with size {} cannot fit into last word with padding of {} bytes (string)", parameter_index, word, size, padding_size);
        return Err(Error::BoundsError);
    }

    // (5) we've covered all words from `data_start_word_offset` to `data_end_word_offset`,
    // so add them to `word_coverages`.
    coverages.extend(
        (word_offset.try_into().unwrap_or(usize::MAX)..
            data_end_word_offset.try_into().unwrap_or(usize::MAX))
            .collect::<Vec<usize>>(),
    );

    trace!("parameter {}: '{}' is string", parameter_index, word);
    Ok(Some(AbiEncoded { ty: String::from("string"), coverages }))
}

/// Handle determining the most potential type of an abi-encoded item.
fn get_potential_type(
    data_words: &[Vec<u8>],
    parameter_index: usize,
    calldata_words: &[Vec<u8>],
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
            let data_words =
                &calldata_words[data_start_word_offset.try_into().unwrap_or(usize::MAX)..];

            // first, check if this word *could* be a nested abi-encoded item
            trace!(
                "parameter {}: '{}' checking for nested abi-encoded data",
                parameter_index,
                word
            );
            if let Ok(Some(nested_abi_encoded_param)) = try_decode_dynamic_parameter(i, data_words)
            {
                // we need to add data_start_word_offset to all the offsets in nested_coverages
                // because they are relative to the start of the nested abi-encoded item.
                let nested_coverages = nested_abi_encoded_param
                    .coverages
                    .into_iter()
                    .map(|nested_coverage| {
                        nested_coverage + data_start_word_offset.try_into().unwrap_or(usize::MAX)
                    })
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
                potential_type.clone_from(types.first().expect("types is empty"));
            }

            (max_size, potential_type)
        });

    potential_type
}

#[cfg(test)]
mod tests {
    use heimdall_common::utils::strings::decode_hex;

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
                let s = std::str::from_utf8(chunk).expect("failed to convert chunk to string");
                decode_hex(s).expect("failed to decode hex")
            })
            .collect::<Vec<_>>();

        for i in 0..calldata_words.len() {
            let abi_encoded_params = try_decode_dynamic_parameter(i, &calldata_words)
                .expect("failed to decode dynamic parameter");
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
                let s = std::str::from_utf8(chunk).expect("failed to convert chunk to string");
                decode_hex(s).expect("failed to decode hex")
            })
            .collect::<Vec<_>>();

        for i in 0..calldata_words.len() {
            let abi_encoded_params = try_decode_dynamic_parameter(i, &calldata_words)
                .expect("failed to decode dynamic parameter");
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
                let s = std::str::from_utf8(chunk).expect("failed to convert chunk to string");
                decode_hex(s).expect("failed to decode hex")
            })
            .collect::<Vec<_>>();

        for i in 0..calldata_words.len() {
            let abi_encoded_params = try_decode_dynamic_parameter(i, &calldata_words)
                .expect("failed to decode dynamic parameter");
            let is_abi_encoded = abi_encoded_params.is_some();
            let coverages =
                abi_encoded_params.as_ref().map(|p| p.coverages.to_owned()).unwrap_or_default();
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
                let s = std::str::from_utf8(chunk).expect("failed to convert chunk to string");
                decode_hex(s).expect("failed to decode hex")
            })
            .collect::<Vec<_>>();

        for i in 0..calldata_words.len() {
            let abi_encoded_params = try_decode_dynamic_parameter(i, &calldata_words)
                .expect("failed to decode dynamic parameter");
            let is_abi_encoded = abi_encoded_params.is_some();
            let coverages =
                abi_encoded_params.as_ref().map(|p| p.coverages.to_owned()).unwrap_or_default();
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
