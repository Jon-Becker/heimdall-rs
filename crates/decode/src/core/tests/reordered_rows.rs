use alloy_dyn_abi::DynSolValue;

use super::decode_raw_bytes;

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
