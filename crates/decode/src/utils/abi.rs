use std::ops::Range;

use alloy::primitives::{Selector, U256};
use alloy_dyn_abi::{DynSolCall, DynSolReturns, DynSolType, DynSolValue};
use alloy_json_abi::Param;
use eyre::eyre;
use heimdall_vm::core::types::{get_padding, get_potential_types_for_word, to_type, Padding};
use tracing::trace;

use crate::error::Error;

#[derive(Debug, Clone)]
pub(crate) struct AbiEncoded {
    pub ty: DynSolType,
    /// Length word and payload, excluding the offset in the enclosing head.
    pub tail: Range<usize>,
}

/// Attempt to decode the given calldata with the given types.
pub(crate) fn try_decode(
    inputs: &[DynSolType],
    byte_args: &[u8],
) -> Result<(Vec<DynSolValue>, Vec<Param>), Error> {
    trace!("try_decode: inputs={:?}, byte_args.len()={}", inputs, byte_args.len());

    // For non-standard sized calldata that isn't a multiple of 32 bytes,
    // we need to pad it to ensure proper ABI decoding
    let padded_args = if !byte_args.is_empty() && !byte_args.len().is_multiple_of(32) {
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

/// Maximum number of array dimensions to infer. Packed bytes and strings are leaves.
const MAX_DYNAMIC_DEPTH: usize = 4;
const PADDING_SAMPLE_WORDS: usize = 8;

pub(crate) fn offset_in_words(word: &[u8]) -> Option<usize> {
    let bytes = usize::try_from(U256::from_be_slice(word)).ok()?;
    (bytes != 0 && bytes.is_multiple_of(32)).then_some(bytes / 32)
}

impl AbiEncoded {
    /// Packed data provides evidence that a scalar-looking head word really is
    /// a pointer, even when following it changes the physical order of tails.
    pub(crate) fn has_packed_data(&self, words: &[&[u8]]) -> bool {
        let payload = &words[self.tail.start + 1..self.tail.end];
        let Ok(length) = usize::try_from(U256::from_be_slice(words[self.tail.start])) else {
            return false
        };
        matches!(self.ty, DynSolType::Bytes | DynSolType::String) &&
            has_packed_data(payload, length)
    }
}

fn has_packed_data(payload: &[&[u8]], length: usize) -> bool {
    length > 0 &&
        payload.last().is_some_and(|w| w[(length - 1) % 32] != 0) &&
        payload
            .iter()
            .any(|w| w.iter().any(|&byte| byte != 0) && get_padding(w) != Padding::Left)
}

pub(crate) fn infer_tail_ending_at(
    words: &[&[u8]],
    start: usize,
    end: usize,
    canonical_budget: &mut usize,
    reordered_budget: &mut usize,
) -> Option<AbiEncoded> {
    let tail = &words[start..end];
    let (len, data) = tail.split_first()?;
    let size = usize::try_from(U256::from_be_slice(len)).ok()?;
    // Word counts prove packed and flat extents in constant time. Only a
    // nested candidate can consume more words than both of those layouts.
    let nested = data.len() > size;
    let budget = match plausible_tail_order(size, data, 0)? {
        TailOrder::Canonical => canonical_budget,
        TailOrder::Reordered => reordered_budget,
    };
    if nested && tail.len() > *budget {
        return None;
    }
    let param = infer_dynamic_tail(start, words, end).filter(|p| p.tail.end == end);
    if nested && param.is_none() {
        *budget -= tail.len();
    }
    param
}

enum TailOrder {
    Canonical,
    Reordered,
}

/// Inspect a constant number of words per dimension to decide which layouts
/// could fill the interval. Full inference then checks every sibling.
fn plausible_tail_order(size: usize, data: &[&[u8]], depth: usize) -> Option<TailOrder> {
    if size == 0 {
        return data.is_empty().then_some(TailOrder::Canonical);
    }
    if data.len() == size.div_ceil(32) && packed_payload_fits(data, size) {
        return Some(TailOrder::Canonical);
    }
    if depth == MAX_DYNAMIC_DEPTH || size > data.len() {
        return None;
    }
    if size == data.len() {
        return Some(TailOrder::Canonical);
    }
    let offset = |word| offset_in_words(word).filter(|n| (size..data.len()).contains(n));
    let first = offset(data[0])?;
    let last = offset(data[size - 1])?;
    if first == size {
        let (len, payload) = data[last..].split_first()?;
        let child_size = usize::try_from(U256::from_be_slice(len)).ok()?;
        if let Some(order) = plausible_tail_order(child_size, payload, depth + 1) {
            return Some(order);
        }
    }
    // A reordered array's last head pointer can name an interior row. All
    // three known starts must have enough data for at least one interpretation.
    (first != last &&
        [size, first, last].iter().all(|&i| {
            let payload = &data[i + 1..];
            usize::try_from(U256::from_be_slice(data[i]))
                .is_ok_and(|n| n <= payload.len() || packed_payload_fits(payload, n))
        }))
    .then_some(TailOrder::Reordered)
}

fn packed_payload_fits(data: &[&[u8]], size: usize) -> bool {
    // Only the final word of nonstandard calldata can be short.
    data.get((size - 1) / 32)
        .and_then(|word| word.get((size - 1) % 32 + 1..))
        .is_some_and(|padding| padding.iter().all(|byte| *byte == 0))
}

/// Keep the interpretations shared by all siblings until the outermost type is known.
enum InferredType {
    Empty,
    // A zero word fits every scalar width.
    Word(Option<InferredWord>),
    // A packed byte and a one-element array can occupy exactly the same tail.
    Dynamic { array: Option<Box<InferredType>>, packed: Option<PackedType> },
}

enum PackedType {
    Bytes,
    // Printable UTF-8 can take its type from a textual sibling.
    Utf8,
    // Two independently printable multiword members also establish a string array.
    LongUtf8,
    Text,
}

impl PackedType {
    fn infer(data: &[&[u8]], size: usize, depth: usize, prefer_text: bool) -> Self {
        if depth == 0 && !prefer_text {
            return Self::Bytes;
        }
        let packed = &data[..size.div_ceil(32)];
        let classify = |bytes: &[u8]| {
            let Ok(text) = std::str::from_utf8(&bytes[..size]) else { return Self::Bytes };
            if text.chars().any(|ch| ch.is_control() && !matches!(ch, '\t' | '\n' | '\r')) {
                Self::Bytes
            } else if prefer_text {
                Self::Text
            } else if packed.len() > 1 {
                Self::LongUtf8
            } else {
                Self::Utf8
            }
        };
        match packed {
            [word] => classify(word),
            words => classify(&words.concat()),
        }
    }

    fn common(self, other: Self) -> Self {
        match (self, other) {
            (Self::Bytes, _) | (_, Self::Bytes) => Self::Bytes,
            (Self::Text, _) | (_, Self::Text) | (Self::LongUtf8, Self::LongUtf8) => Self::Text,
            (Self::LongUtf8, _) | (_, Self::LongUtf8) => Self::LongUtf8,
            _ => Self::Utf8,
        }
    }
}

impl InferredType {
    fn common(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Self::Word(a), Self::Word(b)) => Some(Self::Word(match (a, b) {
                (Some(a), Some(b)) => Some(a.common(b)),
                (a, b) => a.or(b),
            })),
            (Self::Empty, ty) | (ty, Self::Empty) if !matches!(ty, Self::Word(_)) => Some(ty),
            (Self::Dynamic { array: a, packed: p }, Self::Dynamic { array: b, packed: q }) => {
                let array = a.zip(b).and_then(|(a, b)| a.common(*b)).map(Box::new);
                let packed = p.zip(q).map(|(p, q)| p.common(q));
                (array.is_some() || packed.is_some()).then_some(Self::Dynamic { array, packed })
            }
            _ => None,
        }
    }

    fn into_type(self) -> DynSolType {
        match self {
            Self::Empty => DynSolType::Bytes,
            Self::Word(word) => word.map_or(DynSolType::FixedBytes(32), InferredWord::into_type),
            Self::Dynamic { packed: Some(PackedType::Text), .. } => DynSolType::String,
            Self::Dynamic { array: Some(ty), .. } => DynSolType::Array(Box::new(ty.into_type())),
            Self::Dynamic { .. } => DynSolType::Bytes,
        }
    }
}

/// Infer a type and the contiguous region it consumes, starting at its length word.
pub(crate) fn infer_dynamic_tail(offset: usize, words: &[&[u8]], end: usize) -> Option<AbiEncoded> {
    let tail = words.get(offset..end)?;
    // A confirmed boundary can hide the mixed padding that identifies a string
    // before another tuple argument. Include the next tail's length word when
    // a long packed payload would occupy the entire padding sample.
    let tuple_text = if end < words.len() {
        let size = usize::try_from(U256::from_be_slice(tail.first()?)).ok()?;
        let packed_words = size.div_ceil(32);
        1 + packed_words == tail.len() &&
            words[offset + 1..].get(..size).is_some_and(|sample| {
                let next_tail =
                    sample.get(packed_words).filter(|_| packed_words >= PADDING_SAMPLE_WORDS);
                has_mixed_padding(sample, next_tail.copied())
            })
    } else {
        false
    };
    let (ty, count) = infer_dynamic_value(tail, 0, tuple_text)?;
    let ty = match ty {
        InferredType::Empty => DynSolType::Array(Box::new(DynSolType::Bytes)),
        ty => ty.into_type(),
    };
    Some(AbiEncoded { ty, tail: offset..offset + count })
}

fn infer_dynamic_value(
    words: &[&[u8]],
    depth: usize,
    has_text_hint: bool,
) -> Option<(InferredType, usize)> {
    let (length_word, data) = words.split_first()?;
    let size = usize::try_from(U256::from_be_slice(length_word)).ok()?;
    if size == 0 {
        return Some((InferredType::Empty, 1));
    }

    let packed_words = size.div_ceil(32);
    let packed_fits = packed_payload_fits(data, size);
    let elements = data.get(..size);
    // Packed strings can resemble arrays when other heads or tails follow them.
    // Use a bounded sample to distinguish mixed padding from uniform element padding.
    let mixed_padding = packed_fits && size < data.len() && has_mixed_padding(&data[..size], None);
    let prefer_text = mixed_padding || (has_text_hint && (depth == 0 || packed_words == 1));
    let packed_type = || PackedType::infer(data, size, depth, prefer_text);
    let Some(elements) = elements.filter(|_| !mixed_padding && depth < MAX_DYNAMIC_DEPTH) else {
        return packed_fits.then(|| {
            (InferredType::Dynamic { array: None, packed: Some(packed_type()) }, 1 + packed_words)
        });
    };

    let (element, payload_words) = infer_dynamic_elements(elements, data, depth)
        .unwrap_or_else(|| (InferredType::Word(infer_flat_element_type(elements)), size));
    let packed = (packed_fits && packed_words == payload_words).then(packed_type);
    Some((InferredType::Dynamic { array: Some(Box::new(element)), packed }, 1 + payload_words))
}

fn has_mixed_padding(words: &[&[u8]], next_tail: Option<&[u8]>) -> bool {
    let Some((first, rest)) = words.split_first() else { return false };
    if first.iter().all(|&byte| byte == 0) {
        return false;
    }
    let padding = get_padding(first);
    rest.iter().take(PADDING_SAMPLE_WORDS - 1).any(|word| get_padding(word) != padding) ||
        next_tail.is_some_and(|length| {
            get_padding(length) != padding || length.iter().all(|&byte| byte == 0)
        })
}

/// All elements of a dynamic ABI array share one type. Follow offsets only when
/// their tails consecutively partition the payload, so one probe cannot revisit
/// a sibling's words, and the inferred type is valid for every element.
fn infer_dynamic_elements(
    elements: &[&[u8]],
    data: &[&[u8]],
    depth: usize,
) -> Option<(InferredType, usize)> {
    // Identify the physical order before following any pointers. A failed
    // canonical probe must never visit the same children again in a new order.
    let mut previous = 0;
    for word in elements {
        let offset = offset_in_words(word)?;
        if !(elements.len()..data.len()).contains(&offset) || offset == previous {
            return None;
        }
        if offset < previous {
            let mut starts = std::collections::HashSet::with_capacity(elements.len());
            for word in elements {
                let start = offset_in_words(word)?;
                if !(elements.len()..data.len()).contains(&start) || !starts.insert(start) {
                    return None;
                }
            }
            if !starts.contains(&elements.len()) {
                return None;
            }
            let physical = (elements.len()..data.len()).filter(|i| starts.contains(i));
            return infer_adjacent_elements(
                physical.take(starts.len()),
                data,
                elements.len(),
                depth,
                TailOrder::Reordered,
            );
        }
        previous = offset;
    }
    let physical = elements.iter().filter_map(|word| offset_in_words(word));
    infer_adjacent_elements(physical, data, elements.len(), depth, TailOrder::Canonical)
}

fn infer_adjacent_elements(
    starts: impl Iterator<Item = usize>,
    data: &[&[u8]],
    count: usize,
    depth: usize,
    order: TailOrder,
) -> Option<(InferredType, usize)> {
    let mut starts = starts.peekable();
    let mut cursor = count;
    let mut element_ty = InferredType::Empty;
    let mut packed_children = 0;
    while let Some(start) = starts.next() {
        if start != cursor {
            return None;
        }
        let end = starts.peek().copied().unwrap_or(data.len());
        let (ty, used) = infer_dynamic_value(&data[start..end], depth + 1, count > 1)?;
        cursor += used;
        if matches!(order, TailOrder::Reordered) &&
            matches!(ty, InferredType::Dynamic { packed: Some(_), .. })
        {
            let size = usize::try_from(U256::from_be_slice(data[start])).ok()?;
            if has_packed_data(&data[start + 1..cursor], size) {
                packed_children += 1;
            }
        }
        element_ty = element_ty.common(ty)?;
    }
    if matches!(order, TailOrder::Reordered) {
        // Reordered packed leaves can also resemble one-element arrays. Keep
        // the packed interpretation when it fits every member. Other reordered
        // rows must account for the entire interval supplied by their parent.
        if let InferredType::Dynamic { array, packed: Some(_) } = &mut element_ty {
            *array = None;
            if cursor != data.len() && packed_children < 2 {
                return None;
            }
        } else if cursor != data.len() {
            return None;
        }
    }
    Some((element_ty, cursor))
}

pub(crate) fn infer_scalar_type(word: &[u8]) -> DynSolType {
    let (_, types) = get_potential_types_for_word(word);
    let left_padded = get_padding(word) == Padding::Left;
    let ty = types
        .into_iter()
        .find(|ty| {
            if left_padded {
                ty.starts_with("uint") || ty == "address"
            } else {
                ty.starts_with("bytes") || ty == "string"
            }
        })
        .expect("a word always has a scalar ABI type");
    to_type(&ty)
}

// Zeros at both ends can make a number look like fixed bytes or vice versa.
// Keep each observed width and the width needed to hold every sibling.
struct InferredWord {
    uint_width: usize,
    bytes_width: usize,
    required_uint_width: usize,
    required_bytes_width: usize,
    has_address: bool,
}

impl InferredWord {
    fn from_word(word: &[u8]) -> Option<Self> {
        let first = word.iter().position(|&byte| byte != 0)?;
        let last = word.iter().rposition(|&byte| byte != 0)?;
        // try_decode right-pads a short final word before passing it to Alloy.
        let padded;
        let word = if word.len() < 32 {
            padded = alloy::primitives::B256::right_padding_from(word);
            padded.as_slice()
        } else {
            word
        };
        let ty = infer_scalar_type(word);
        let (uint_width, bytes_width) = match ty {
            DynSolType::Uint(bits) => (bits / 8, 0),
            DynSolType::Address => (20, 0),
            DynSolType::FixedBytes(bytes) => (0, bytes),
            _ => unreachable!("word inference yields an integer, address, or fixed bytes"),
        };
        Some(Self {
            uint_width,
            bytes_width,
            required_uint_width: 32 - first,
            required_bytes_width: last + 1,
            has_address: ty == DynSolType::Address,
        })
    }

    fn common(self, other: Self) -> Self {
        Self {
            uint_width: self.uint_width.max(other.uint_width),
            bytes_width: self.bytes_width.max(other.bytes_width),
            required_uint_width: self.required_uint_width.max(other.required_uint_width),
            required_bytes_width: self.required_bytes_width.max(other.required_bytes_width),
            has_address: self.has_address || other.has_address,
        }
    }

    fn into_type(self) -> DynSolType {
        if self.has_address && self.required_uint_width <= 20 {
            return DynSolType::Address;
        }
        // For mixed padding, a nonzero byte at the opposite end rules out
        // every narrower interpretation in that direction.
        let fits_uint = self.uint_width >= self.required_uint_width &&
            (self.bytes_width == 0 || self.required_bytes_width == 32);
        let fits_bytes = self.bytes_width >= self.required_bytes_width &&
            (self.uint_width == 0 || self.required_uint_width == 32);
        match (fits_uint, fits_bytes) {
            (true, false) => DynSolType::Uint(self.uint_width * 8),
            (false, true) => DynSolType::FixedBytes(self.bytes_width),
            _ => DynSolType::FixedBytes(32),
        }
    }
}

fn infer_flat_element_type(elements: &[&[u8]]) -> Option<InferredWord> {
    elements.iter().filter_map(|word| InferredWord::from_word(word)).reduce(InferredWord::common)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use heimdall_common::utils::strings::decode_hex;

    use super::*;

    fn probe(index: usize, words: &[&[u8]]) -> Option<AbiEncoded> {
        infer_dynamic_tail(offset_in_words(words[index])?, words, words.len())
    }

    fn covered_words(index: usize, param: Option<&AbiEncoded>) -> HashSet<usize> {
        param.map(|p| std::iter::once(index).chain(p.tail.clone()).collect()).unwrap_or_default()
    }

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
        let calldata = include_str!(
            "../../../core/tests/testdata/decode/abi/detect_abi_encoded_parameters_nominal.hex"
        );

        let calldata = decode_hex(&calldata[10..]).expect("invalid fixture");
        let calldata_words = calldata.chunks(32).collect::<Vec<_>>();

        for i in 0..calldata_words.len() {
            let abi_encoded_params = probe(i, &calldata_words);
            let is_abi_encoded = abi_encoded_params.is_some();
            let coverages = covered_words(i, abi_encoded_params.as_ref());

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
        let calldata = include_str!(
            "../../../core/tests/testdata/decode/abi/detect_abi_encoded_parameters_2.hex"
        );

        let calldata = decode_hex(&calldata[10..]).expect("invalid fixture");
        let calldata_words = calldata.chunks(32).collect::<Vec<_>>();

        for i in 0..calldata_words.len() {
            let abi_encoded_params = probe(i, &calldata_words);
            let is_abi_encoded = abi_encoded_params.is_some();
            let coverages = covered_words(i, abi_encoded_params.as_ref());

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
        let calldata =
            include_str!("../../../core/tests/testdata/decode/abi/detect_abi_encoded_complex.hex");

        let calldata = decode_hex(&calldata[10..]).expect("invalid fixture");
        let calldata_words = calldata.chunks(32).collect::<Vec<_>>();

        for i in 0..calldata_words.len() {
            let abi_encoded_params = probe(i, &calldata_words);
            let is_abi_encoded = abi_encoded_params.is_some();
            let coverages = covered_words(i, abi_encoded_params.as_ref());
            let ty = abi_encoded_params.map(|p| p.ty.to_string()).unwrap_or_default();

            println!(
                "{i} - is_abi_encoded: {}, ty: {:?}, with word coverage: {:?}",
                is_abi_encoded, ty, coverages
            );

            if i == 0 {
                assert!(is_abi_encoded);
                assert_eq!(coverages, HashSet::from([0, 2, 3, 4, 5, 6, 7, 8, 9]));
            } else if i == 1 {
                assert!(is_abi_encoded);
                assert_eq!(ty, "string[]");
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
        let calldata =
            include_str!("../../../core/tests/testdata/decode/abi/detect_abi_encoded_mixed.hex");

        let calldata = decode_hex(&calldata[10..]).expect("invalid fixture");
        let calldata_words = calldata.chunks(32).collect::<Vec<_>>();

        for i in 0..calldata_words.len() {
            let abi_encoded_params = probe(i, &calldata_words);
            let is_abi_encoded = abi_encoded_params.is_some();
            let coverages = covered_words(i, abi_encoded_params.as_ref());
            let ty = abi_encoded_params.map(|p| p.ty.to_string()).unwrap_or_default();

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

    #[test]
    fn repeated_offsets_have_bounded_tails() {
        for (value, expected_ty, expected_tail) in [
            (0x20, DynSolType::Array(Box::new(DynSolType::Uint(8))), 1..34),
            (0x40, DynSolType::Bytes, 2..5),
        ] {
            let mut word = [0; 32];
            word[31] = value;
            let words = [word.as_slice(); 48];
            let decoded = probe(0, &words).expect("valid dynamic tail");
            assert_eq!(decoded.ty, expected_ty);
            assert_eq!(decoded.tail, expected_tail);
        }
    }
}
