use super::{decode_raw, decode_raw_result, fixed_bytes, DynSolValue};

#[tokio::test]
async fn packed_text_keeps_its_empty_sibling_after_offset_like_scalars() {
    for scalars in [vec![64u8], vec![128], vec![64, 96]] {
        let mut args = vec![DynSolValue::String("hi".into())];
        args.extend(scalars.into_iter().map(DynSolValue::from));
        args.push(DynSolValue::Array(Vec::new()));
        assert_eq!(decode_raw(&DynSolValue::Tuple(args.clone())).await, args);
    }
}

#[tokio::test]
async fn an_offset_to_an_array_element_must_identify_a_valid_tail() {
    let array = DynSolValue::Array(vec![fixed_bytes(b"AB"), fixed_bytes(b"CD")]);
    let args = vec![array, 96u8.into(), 160u8.into()];
    assert_eq!(decode_raw(&DynSolValue::Tuple(args.clone())).await, args);
}

#[tokio::test]
async fn inner_pointer_preserves_a_tightly_sized_byte_argument() {
    let args = vec![64u8.into(), DynSolValue::Bytes(vec![1, 2]), 37u8.into()];
    let encoded = DynSolValue::Tuple(args.clone()).abi_encode_params();
    let result = decode_raw_result(&encoded).await;
    assert_eq!(result.decoded.decoded_inputs.unwrap(), args);

    // The same inner length word and trailing zeroes are also valid byte data.
    // Only the tuple with a pointer to the inner length supplies corroboration.
    let mut bytes = vec![0; 37];
    bytes[31..34].copy_from_slice(&[2, 1, 2]);
    let other = [DynSolValue::Bytes(bytes.clone()), 96u8.into()];
    assert!(result.alternatives.iter().any(|alt| alt.decoded_inputs.as_deref() == Some(&other)));
    let args = vec![DynSolValue::Bytes(bytes), 7u8.into()];
    assert_eq!(decode_raw(&DynSolValue::Tuple(args.clone())).await, args);
}

#[tokio::test]
async fn simple_call_omits_alternatives_from_json() {
    let encoded = DynSolValue::Tuple(vec![7u8.into()]).abi_encode_params();
    let result = decode_raw_result(&encoded).await;
    assert!(result.alternatives.is_empty());
    let json: serde_json::Value = serde_json::from_str(&result.to_json().unwrap()).unwrap();
    assert!(json.get("alternatives").is_none());
}

#[tokio::test]
async fn raw_decode_keeps_canonical_arguments_when_packed_tails_can_be_reordered() {
    let first = DynSolValue::Bytes(vec![0xaa, 0xbb]);
    let last = fixed_bytes(&[0xcc, 0xdd]);
    for args in [
        vec![first.clone(), 64u8.into(), 2u8.into(), last.clone()],
        vec![first.clone(), first, 96u8.into(), 2u8.into(), last],
    ] {
        let encoded = DynSolValue::Tuple(args.clone()).abi_encode_params();
        let result = decode_raw_result(&encoded).await;
        assert_eq!(result.decoded.decoded_inputs.as_ref().unwrap(), &args);
        let json: serde_json::Value = serde_json::from_str(&result.to_json().unwrap()).unwrap();
        assert_eq!(json["inputs"].as_array().unwrap().len(), args.len());
        let alternative = result.alternatives.first().unwrap().decoded_inputs.as_ref().unwrap();
        assert_eq!(alternative.last(), Some(&DynSolValue::Bytes(vec![0xcc, 0xdd])));
        assert_eq!(
            json["alternatives"][0]["decoded_inputs"].as_array().unwrap().len(),
            alternative.len()
        );
    }
}
