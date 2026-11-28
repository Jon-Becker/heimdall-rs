use alloy_dyn_abi::DynSolValue;

use super::{decode_raw_bytes, decode_raw_result};

/// Encode the children in physical order, then restore the logical head order.
pub(super) fn encode_reordered_array(values: &[DynSolValue], order: &[usize]) -> Vec<u8> {
    let physical = DynSolValue::Array(order.iter().map(|&i| values[i].clone()).collect());
    let mut encoded = physical.abi_encode_params();
    // The first two words are the argument offset and the array length.
    let offsets = &mut encoded[64..64 + 32 * order.len()];
    let sorted = offsets.to_vec();
    for (&logical, offset) in order.iter().zip(sorted.chunks_exact(32)) {
        offsets[32 * logical..32 * (logical + 1)].copy_from_slice(offset);
    }
    encoded
}

fn integers(values: &[u8]) -> DynSolValue {
    DynSolValue::Array(values.iter().copied().map(DynSolValue::from).collect())
}

#[tokio::test]
async fn raw_decode_preserves_reordered_integer_rows() {
    let rows = vec![integers(&[1, 2]), integers(&[3, 4])];
    let encoded = encode_reordered_array(&rows, &[1, 0]);
    assert_eq!(decode_raw_bytes(&encoded).await, [DynSolValue::Array(rows)]);

    let rows = vec![integers(&[1]), integers(&[]), integers(&[7, 8, 9])];
    for order in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
        let encoded = encode_reordered_array(&rows, &order);
        assert_eq!(decode_raw_bytes(&encoded).await, [DynSolValue::Array(rows.clone())]);
    }
}

#[tokio::test]
async fn raw_decode_preserves_nested_rows_with_reordered_tails() {
    let rows = vec![
        DynSolValue::Array(vec![integers(&[1, 2]), integers(&[3])]),
        DynSolValue::Array(vec![integers(&[4]), integers(&[5, 6, 7])]),
    ];
    let encoded = encode_reordered_array(&rows, &[1, 0]);
    assert_eq!(decode_raw_bytes(&encoded).await, [DynSolValue::Array(rows)]);
}

#[tokio::test]
async fn surrounding_rows_resolve_reordered_empty_and_zero_children() {
    let children = vec![integers(&[]), integers(&[0])];
    let rows = DynSolValue::Array(vec![
        DynSolValue::Array(children.clone()),
        DynSolValue::Array(vec![integers(&[1])]),
    ]);
    for order in [[0, 1], [1, 0]] {
        let mut encoded = rows.abi_encode_params();
        let first_row = encode_reordered_array(&children, &order);
        // The tuple offset, outer length and two child offsets precede the first row.
        let first_row_body = &first_row[32..];
        encoded[128..128 + first_row_body.len()].copy_from_slice(first_row_body);
        assert_eq!(decode_raw_bytes(&encoded).await, std::slice::from_ref(&rows));
    }
}

#[tokio::test]
async fn reordered_single_byte_leaves_keep_their_packed_type() {
    let values = [0xff, 0x80].map(|byte| DynSolValue::Bytes(vec![byte]));
    let encoded = encode_reordered_array(&values, &[1, 0]);
    assert_eq!(decode_raw_bytes(&encoded).await, [DynSolValue::Array(values.to_vec())]);
}

#[tokio::test]
async fn raw_decode_preserves_ambiguous_packed_and_empty_rows() {
    let empty = DynSolValue::Array(Vec::new());
    for first_row in [empty.clone(), DynSolValue::Array(vec![DynSolValue::Bytes(vec![0xcc])])] {
        let array = DynSolValue::Array(vec![first_row, empty.clone()]);
        let result = decode_raw_result(&array.abi_encode_params()).await;
        assert!(std::iter::once(&result.decoded).chain(&result.alternatives).any(|candidate| {
            candidate.inputs == ["bytes[][]"] &&
                candidate.decoded_inputs.as_deref() == Some(std::slice::from_ref(&array))
        }));
    }
}

#[tokio::test]
async fn zero_only_scalar_rows_keep_the_full_word_interpretation() {
    let mut integer_word = [0; 32];
    integer_word[31] = 1;
    let mut bytes_word = [0x61; 32];
    bytes_word[31] = 0;
    for (word, primary) in [(integer_word, "uint8[][]"), (bytes_word, "bytes31[][]")] {
        let array = DynSolValue::Array(vec![
            DynSolValue::Array(vec![super::fixed_bytes(&word)]),
            DynSolValue::Array(Vec::new()),
            DynSolValue::Array(vec![super::fixed_bytes(&[0; 32])]),
        ]);
        let result = decode_raw_result(&array.abi_encode_params()).await;
        assert_eq!(result.decoded.inputs, [primary]);
        let mut full_words = result.alternatives.iter().filter(|alt| alt.inputs == ["bytes32[][]"]);
        assert_eq!(
            full_words.next().unwrap().decoded_inputs.as_deref(),
            Some(std::slice::from_ref(&array))
        );
        assert!(full_words.next().is_none());
    }

    let mixed = DynSolValue::Array(vec![integers(&[0, 1]), integers(&[1])]);
    let result = decode_raw_result(&mixed.abi_encode_params()).await;
    assert!(result.alternatives.iter().all(|alt| alt.inputs != ["bytes32[][]"]));
}

#[tokio::test]
async fn nested_packed_and_empty_arrays_keep_their_textual_tuple_sibling() {
    let strings = DynSolValue::Array(vec![DynSolValue::String("hi".into())]);
    let empty = DynSolValue::Array(Vec::new());
    for first in [empty.clone(), DynSolValue::Array(vec![DynSolValue::Bytes(vec![0xcc])])] {
        let args = [strings.clone(), DynSolValue::Array(vec![first, empty.clone()])];
        let result =
            decode_raw_result(&DynSolValue::Tuple(args.to_vec()).abi_encode_params()).await;
        assert!(result.alternatives.iter().any(|alt| {
            alt.inputs == ["string[]", "bytes[][]"] && alt.decoded_inputs.as_deref() == Some(&args)
        }));
    }

    let zero_rows =
        DynSolValue::Array(vec![DynSolValue::Array(vec![super::fixed_bytes(&[0; 32])])]);
    let args = [zero_rows, strings];
    let result = decode_raw_result(&DynSolValue::Tuple(args.to_vec()).abi_encode_params()).await;
    assert!(result.alternatives.iter().any(|alt| {
        alt.inputs == ["bytes32[][]", "string[]"] && alt.decoded_inputs.as_deref() == Some(&args)
    }));
}
