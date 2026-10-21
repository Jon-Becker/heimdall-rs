use alloy::{
    primitives::{Address, B256},
    rpc::types::{Filter, Log},
};
use alloy_dyn_abi::{DynSolType, DynSolValue, Specifier};
use alloy_json_abi::{Event, JsonAbi};
use eyre::{bail, eyre, Result};
use hashbrown::HashMap;
use heimdall_common::ether::provider::MultiTransportProvider;
use tracing::{debug, warn};

use crate::interfaces::DecompilerArgs;

const MAX_BLOCKS: u64 = 10_000;
const BLOCKS_PER_REQUEST: u64 = 200;
const MAX_INPUTS: usize = 16;

pub(super) fn validate_lookup(args: &DecompilerArgs) -> Result<Address> {
    if args.rpc_url.is_empty() {
        bail!("--indexed-log-check requires --rpc-url");
    }
    let address =
        args.indexed_log_address.or_else(|| args.target.parse().ok()).ok_or_else(|| {
            eyre!("--indexed-log-check requires an address target or --indexed-log-address")
        })?;
    if let (Some(from), Some(to)) = (args.indexed_log_from_block, args.indexed_log_to_block) {
        block_range(Some(from), to)?;
    }
    Ok(address)
}

fn block_range(from: Option<u64>, to: u64) -> Result<(u64, u64)> {
    let from = from.unwrap_or_else(|| to.saturating_sub(MAX_BLOCKS - 1));
    if from > to || to - from >= MAX_BLOCKS {
        bail!("indexed log range must contain between 1 and 10,000 blocks");
    }
    Ok((from, to))
}

pub(super) async fn recover_event_indexing(abi: &mut JsonAbi, args: &DecompilerArgs) -> Result<()> {
    let address = validate_lookup(args)?;
    let topics: Vec<_> = abi
        .events()
        .filter(|event| {
            !event.anonymous && !event.inputs.is_empty() && event.inputs.len() <= MAX_INPUTS
        })
        .map(Event::selector)
        .collect();
    if topics.is_empty() {
        debug!("no resolved events eligible for indexed log lookup");
        return Ok(());
    }

    let provider = MultiTransportProvider::connect(&args.rpc_url).await?;
    let to = match args.indexed_log_to_block {
        Some(block) => block,
        None => provider.get_block_number().await?,
    };
    let (from, mut end) = block_range(args.indexed_log_from_block, to)?;
    let mut candidates: HashMap<_, _> = abi
        .events()
        .filter_map(|event| IndexedLayouts::new(event).map(|layouts| (event.selector(), layouts)))
        .collect();
    loop {
        let start = end.saturating_sub(BLOCKS_PER_REQUEST - 1).max(from);
        let filter = Filter::new()
            .address(address)
            .event_signature(topics.clone())
            .from_block(start)
            .to_block(end);
        let logs = provider.get_logs(&filter).await?;
        for log in logs {
            if log.address() != address ||
                log.removed ||
                !log.block_number.is_some_and(|block| (start..=end).contains(&block))
            {
                continue;
            }
            let Some(topic) = log.topics().first().copied() else { continue };
            if let Some(layouts) = candidates.get_mut(&topic) {
                layouts.observe(&log);
            }
        }
        if start == from {
            break;
        }
        end = start - 1;
    }

    // Apply only after every request succeeds, so a failed lookup cannot leave partial corrections.
    for event in abi.events.values_mut().flatten() {
        let topic = event.selector();
        let Some(layouts) = candidates.get(&topic) else { continue };
        if let Some(layout) = layouts.unique() {
            for (input, indexed) in event.inputs.iter_mut().zip(layout) {
                input.indexed = *indexed;
            }
            debug!(signature = %event.signature(), samples = layouts.samples, "recovered indexed event layout");
        } else if layouts.samples > 0 {
            warn!(signature = %event.signature(), "logs do not identify one indexed layout; retaining inferred layout");
        }
    }
    Ok(())
}

#[derive(Debug)]
struct IndexedLayouts {
    selector: B256,
    types: Vec<DynSolType>,
    layouts: Vec<Vec<bool>>,
    samples: usize,
}

impl IndexedLayouts {
    fn new(event: &Event) -> Option<Self> {
        if event.anonymous || event.inputs.len() > MAX_INPUTS {
            return None;
        }
        let types = event
            .inputs
            .iter()
            .map(Specifier::resolve)
            .collect::<std::result::Result<Vec<DynSolType>, _>>()
            .ok()?;
        let layouts = (0u32..(1 << types.len()))
            .filter(|mask| mask.count_ones() <= 3)
            .map(|mask| (0..types.len()).map(|index| mask & (1 << index) != 0).collect())
            .collect();
        Some(Self { selector: event.selector(), types, layouts, samples: 0 })
    }

    fn observe(&mut self, log: &Log) {
        if log.removed || log.topics().first() != Some(&self.selector) {
            return;
        }
        self.samples += 1;
        let count = log.topics().len() - 1;
        self.layouts.retain(|layout| {
            layout.iter().filter(|indexed| **indexed).count() == count &&
                layout_matches(&self.types, layout, log)
        });
    }

    fn unique(&self) -> Option<&[bool]> {
        if self.samples > 0 && self.layouts.len() == 1 {
            Some(&self.layouts[0])
        } else {
            None
        }
    }
}

fn layout_matches(types: &[DynSolType], layout: &[bool], log: &Log) -> bool {
    let mut topics = log.topics().iter().skip(1);
    let mut body = Vec::new();
    for (ty, indexed) in types.iter().zip(layout) {
        if *indexed {
            let Some(topic) = topics.next() else { return false };
            match ty {
                DynSolType::Bytes |
                DynSolType::String |
                DynSolType::Array(_) |
                DynSolType::FixedArray(..) |
                DynSolType::Tuple(_) => {}
                _ => {
                    let Ok(value) = ty.abi_decode(topic.as_slice()) else { return false };
                    if !canonical_value(&value) || value.abi_encode() != topic.as_slice() {
                        return false;
                    }
                }
            }
        } else {
            body.push(lossless_decode_type(ty));
        }
    }
    let data = &log.data().data;
    let Ok(value) = DynSolType::Tuple(body).abi_decode_sequence(data) else { return false };
    canonical_value(&value) &&
        value.abi_encode_sequence().is_some_and(|encoded| encoded == data.as_ref())
}

// Solidity strings can contain arbitrary bytes. Decode them as ABI-equivalent bytes to avoid
// Alloy's lossy UTF-8 conversion changing the payload used for canonical re-encoding checks.
fn lossless_decode_type(ty: &DynSolType) -> DynSolType {
    match ty {
        DynSolType::String => DynSolType::Bytes,
        DynSolType::Array(inner) => DynSolType::Array(Box::new(lossless_decode_type(inner))),
        DynSolType::FixedArray(inner, size) => {
            DynSolType::FixedArray(Box::new(lossless_decode_type(inner)), *size)
        }
        DynSolType::Tuple(types) => {
            DynSolType::Tuple(types.iter().map(lossless_decode_type).collect())
        }
        _ => ty.clone(),
    }
}

fn canonical_value(value: &DynSolValue) -> bool {
    match value {
        DynSolValue::Uint(value, bits) => value.bit_len() <= *bits,
        DynSolValue::Int(value, bits) => {
            let bytes = value.to_be_bytes::<32>();
            let start = 32 - bits / 8;
            let padding = if bytes[start] & 0x80 == 0 { 0 } else { 0xff };
            bytes[..start].iter().all(|byte| *byte == padding)
        }
        DynSolValue::FixedBytes(value, size) => value[*size..].iter().all(|byte| *byte == 0),
        DynSolValue::Array(values) |
        DynSolValue::FixedArray(values) |
        DynSolValue::Tuple(values) => values.iter().all(canonical_value),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{keccak256, LogData, U256};
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Fixture {
        chain_id: u64,
        address: Address,
        signature: String,
        expected_indexed: Option<Vec<bool>>,
        logs: Vec<Log>,
    }

    #[test]
    fn real_ethereum_receipts_recover_only_unique_layouts() {
        for json in [
            include_str!("../../../core/tests/testdata/decompile/indexed-events/bayc-transfer.json"),
            include_str!("../../../core/tests/testdata/decompile/indexed-events/bayc-approval-for-all.json"),
            include_str!("../../../core/tests/testdata/decompile/indexed-events/ens-name-wrapped.json"),
            include_str!("../../../core/tests/testdata/decompile/indexed-events/ens-name-unwrapped.json"),
            include_str!("../../../core/tests/testdata/decompile/indexed-events/weth-transfer-ambiguous.json"),
        ] {
            let fixture: Fixture = serde_json::from_str(json).unwrap();
            assert_eq!(fixture.chain_id, 1);
            let event = Event::parse(&fixture.signature).unwrap();
            let mut candidates = IndexedLayouts::new(&event).unwrap();
            assert!(candidates.unique().is_none());
            for log in &fixture.logs {
                assert_eq!(log.address(), fixture.address);
                assert!(log.transaction_hash.is_some());
                assert!(log.block_hash.is_some());
                assert_eq!(log.topics()[0], event.selector());
                candidates.observe(log);
            }
            assert_eq!(
                candidates.unique(),
                fixture.expected_indexed.as_deref(),
                "{} at {}",
                fixture.signature,
                fixture.address
            );
        }
    }

    fn log(event: &Event, indexed: Vec<B256>, body: Vec<DynSolValue>) -> Log {
        let mut topics = vec![event.selector()];
        topics.extend(indexed);
        Log {
            inner: alloy::primitives::Log {
                address: Address::ZERO,
                data: LogData::new(
                    topics,
                    DynSolValue::Tuple(body).abi_encode_sequence().unwrap().into(),
                )
                .unwrap(),
            },
            ..Default::default()
        }
    }

    #[test]
    fn indexed_strings_arrays_and_tuples_are_hashes() {
        for signature in [
            "Changed(string,bool)",
            "Changed(uint256[],bool)",
            "Changed(uint256[2],bool)",
            "Changed((uint256,bool),bool)",
        ] {
            let event = Event::parse(signature).unwrap();
            let mut candidates = IndexedLayouts::new(&event).unwrap();
            candidates.observe(&log(
                &event,
                vec![keccak256("opaque hash")],
                vec![DynSolValue::Bool(true)],
            ));
            assert_eq!(candidates.unique(), Some([true, false].as_slice()), "{signature}");
        }
    }

    #[test]
    fn non_utf8_strings_preserve_ambiguous_layouts() {
        let raw = DynSolValue::Bytes(vec![0xff]);
        for (signature, body) in [
            ("Changed(string,bytes)", raw.clone()),
            ("Changed(string[],bytes[])", DynSolValue::Array(vec![raw.clone()])),
            ("Changed(string[1],bytes[1])", DynSolValue::FixedArray(vec![raw.clone()])),
            (
                "Changed((string,bool),(bytes,bool))",
                DynSolValue::Tuple(vec![raw.clone(), DynSolValue::Bool(true)]),
            ),
            (
                "Changed((string[1],bool)[],(bytes[1],bool)[])",
                DynSolValue::Array(vec![DynSolValue::Tuple(vec![
                    DynSolValue::FixedArray(vec![raw]),
                    DynSolValue::Bool(true),
                ])]),
            ),
        ] {
            let event = Event::parse(signature).unwrap();
            let mut candidates = IndexedLayouts::new(&event).unwrap();
            // bytes and string have the same ABI encoding, even for non-UTF-8 payloads.
            candidates.observe(&log(&event, vec![keccak256("opaque hash")], vec![body]));
            assert!(candidates.layouts.contains(&vec![false, true]), "{signature}");
            assert!(candidates.layouts.contains(&vec![true, false]), "{signature}");
            assert!(candidates.unique().is_none(), "{signature}");
        }
    }

    #[tokio::test]
    async fn lookup_keeps_non_utf8_string_layout_ambiguous() {
        let event = Event::parse("Changed(string value, bytes indexed tag)").unwrap();
        let mut sample = log(&event, vec![keccak256([1u8])], vec![DynSolValue::Bytes(vec![0xff])]);
        sample.block_number = Some(1);
        let (url, server) = rpc_server(vec![serde_json::json!({"result": [sample]})]).await;
        let args = crate::interfaces::DecompilerArgsBuilder::new()
            .target(Address::ZERO.to_string())
            .rpc_url(url)
            .indexed_log_check(true)
            .indexed_log_from_block(Some(1))
            .indexed_log_to_block(Some(1))
            .build()
            .unwrap();
        let mut abi = JsonAbi::new();
        abi.events.insert(event.name.clone(), vec![event]);
        let original = abi.clone();
        recover_event_indexing(&mut abi, &args).await.unwrap();
        assert_eq!(abi, original);
        assert_eq!(server.await.unwrap().len(), 1);
    }

    #[test]
    fn rejects_noncanonical_primitive_topics_and_body_words() {
        for (ty, word) in [
            (DynSolType::Uint(8), U256::from(256).to_be_bytes::<32>()),
            (DynSolType::Int(8), U256::from(128).to_be_bytes::<32>()),
            (DynSolType::Bool, U256::from(2).to_be_bytes::<32>()),
            (DynSolType::Address, [0xff; 32]),
            (DynSolType::FixedBytes(1), [0xff; 32]),
            (DynSolType::Function, [0xff; 32]),
        ] {
            let event = Event::parse("Changed(bytes32)").unwrap();
            let topic_log = log(&event, vec![B256::from(word)], vec![]);
            assert!(!layout_matches(std::slice::from_ref(&ty), &[true], &topic_log), "topic {ty}");
            let mut body_log = log(&event, vec![], vec![]);
            body_log.inner.data.data = word.to_vec().into();
            assert!(!layout_matches(std::slice::from_ref(&ty), &[false], &body_log), "body {ty}");
        }
    }

    #[test]
    fn accepts_canonical_signed_width_and_rejects_nested_overflow() {
        let event = Event::parse("Changed(int8)").unwrap();
        let negative = log(&event, vec![B256::repeat_byte(0xff)], vec![]);
        assert!(layout_matches(&[DynSolType::Int(8)], &[true], &negative));
        let nested = DynSolValue::Tuple(vec![DynSolValue::Array(vec![DynSolValue::Uint(
            U256::from(256),
            8,
        )])]);
        assert!(!canonical_value(&nested));
    }

    #[test]
    fn rejects_trailing_data_bad_dynamic_offsets_and_conflicting_counts() {
        let event = Event::parse("Changed(bytes,bool)").unwrap();
        let mut sample = log(&event, vec![keccak256("hash")], vec![DynSolValue::Bool(true)]);
        let mut candidates = IndexedLayouts::new(&event).unwrap();
        candidates.observe(&sample);
        assert_eq!(candidates.unique(), Some([true, false].as_slice()));
        let mut trailing = sample.clone();
        let mut data = trailing.data().data.to_vec();
        data.extend_from_slice(&[0; 32]);
        trailing.inner.data.data = data.into();
        candidates.observe(&trailing);
        assert!(candidates.unique().is_none());

        let mut conflict = IndexedLayouts::new(&event).unwrap();
        conflict.observe(&sample);
        sample.inner.data = LogData::new(vec![event.selector()], [0; 64].into()).unwrap();
        conflict.observe(&sample);
        assert!(conflict.unique().is_none());

        let mut invalid = log(&event, vec![], vec![]);
        invalid.inner.data.data = [0xff; 64].into();
        assert!(!layout_matches(&[DynSolType::Bytes, DynSolType::Bool], &[false, false], &invalid));
    }

    #[test]
    fn removed_unrelated_anonymous_and_large_events_do_not_prove_layouts() {
        let event = Event::parse("Changed(address)").unwrap();
        let mut candidates = IndexedLayouts::new(&event).unwrap();
        let mut removed = log(&event, vec![B256::ZERO], vec![]);
        removed.removed = true;
        candidates.observe(&removed);
        candidates.observe(&log(
            &Event::parse("Other(address)").unwrap(),
            vec![B256::ZERO],
            vec![],
        ));
        assert_eq!(candidates.samples, 0);
        assert!(candidates.unique().is_none());
        let mut anonymous = event;
        anonymous.anonymous = true;
        assert!(IndexedLayouts::new(&anonymous).is_none());
        let big = Event::parse(&format!("Big({})", vec!["uint256"; 17].join(","))).unwrap();
        assert!(IndexedLayouts::new(&big).is_none());
    }

    #[test]
    fn block_ranges_are_bounded_without_underflow() {
        assert_eq!(block_range(None, 0).unwrap(), (0, 0));
        assert_eq!(block_range(None, 25_000).unwrap(), (15_001, 25_000));
        assert!(block_range(Some(0), 10_000).is_err());
        assert!(block_range(Some(2), 1).is_err());
        assert_eq!(block_range(Some(u64::MAX), u64::MAX).unwrap(), (u64::MAX, u64::MAX));
    }
    async fn rpc_server(
        responses: Vec<serde_json::Value>,
    ) -> (String, tokio::task::JoinHandle<Vec<serde_json::Value>>) {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for response in responses {
                let (mut stream, _) =
                    tokio::time::timeout(std::time::Duration::from_secs(10), listener.accept())
                        .await
                        .unwrap()
                        .unwrap();
                let mut bytes = Vec::new();
                let (header_end, length) = loop {
                    let mut buffer = [0; 4096];
                    let read = stream.read(&mut buffer).await.unwrap();
                    assert_ne!(read, 0);
                    bytes.extend_from_slice(&buffer[..read]);
                    if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                        let length: usize = header
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length:"))
                            .unwrap()
                            .trim()
                            .parse()
                            .unwrap();
                        break (end + 4, length);
                    }
                };
                while bytes.len() < header_end + length {
                    let mut buffer = [0; 4096];
                    let read = stream.read(&mut buffer).await.unwrap();
                    assert_ne!(read, 0);
                    bytes.extend_from_slice(&buffer[..read]);
                }
                let request: serde_json::Value =
                    serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap();
                let mut response = response;
                response["jsonrpc"] = "2.0".into();
                response["id"] = request["id"].clone();
                requests.push(request);
                let body = response.to_string();
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).as_bytes()).await.unwrap();
            }
            requests
        });
        (url, task)
    }

    #[tokio::test]
    async fn lookup_filters_emitter_ranges_removed_logs_and_updates_detailed_abi() {
        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../../core/tests/testdata/decompile/indexed-events/bayc-approval-for-all.json"
        ))
        .unwrap();
        let sample = fixture.logs[0].clone();
        let block = sample.block_number.unwrap();
        let mut foreign = sample.clone();
        foreign.inner.address = Address::ZERO;
        foreign.inner.data = LogData::new(vec![sample.topics()[0]], [0; 64].into()).unwrap();
        let mut removed = foreign.clone();
        removed.inner.address = fixture.address;
        removed.removed = true;
        let mut out_of_range = foreign.clone();
        out_of_range.inner.address = fixture.address;
        out_of_range.block_number = Some(block + 1);
        let (url, server) = rpc_server(vec![
            serde_json::json!({"result": [sample, foreign, removed, out_of_range]}),
        ])
        .await;
        let args = crate::interfaces::DecompilerArgsBuilder::new()
            .target(fixture.address.to_string())
            .rpc_url(url)
            .indexed_log_check(true)
            .indexed_log_from_block(Some(block))
            .indexed_log_to_block(Some(block))
            .build()
            .unwrap();
        let mut abi = JsonAbi::new();
        abi.events
            .insert("ApprovalForAll".to_string(), vec![Event::parse(&fixture.signature).unwrap()]);
        recover_event_indexing(&mut abi, &args).await.unwrap();
        assert_eq!(
            abi.events["ApprovalForAll"][0]
                .inputs
                .iter()
                .map(|input| input.indexed)
                .collect::<Vec<_>>(),
            vec![true, true, false]
        );
        let detailed = crate::core::out::build_abi_with_details(&abi, &[]).unwrap();
        assert_eq!(detailed[0]["inputs"][0]["indexed"], true);
        let requests = server.await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0]["method"], "eth_getLogs");
        assert_eq!(requests[0]["params"][0]["address"], fixture.address.to_string().to_lowercase());
        assert_eq!(requests[0]["params"][0]["fromBlock"], format!("0x{block:x}"));
        assert_eq!(requests[0]["params"][0]["toBlock"], format!("0x{block:x}"));
    }

    #[tokio::test]
    async fn rpc_failure_keeps_the_entire_abi_unchanged() {
        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../../core/tests/testdata/decompile/indexed-events/bayc-transfer.json"
        ))
        .unwrap();
        let block = fixture.logs[0].block_number.unwrap();
        let (url, server) = rpc_server(vec![
            serde_json::json!({"result": [fixture.logs[0]]}),
            serde_json::json!({"error":{"code":-32000,"message":"query rejected"}}),
        ])
        .await;
        let args = crate::interfaces::DecompilerArgsBuilder::new()
            .target(fixture.address.to_string())
            .rpc_url(url)
            .indexed_log_check(true)
            .indexed_log_from_block(Some(block - 200))
            .indexed_log_to_block(Some(block))
            .build()
            .unwrap();
        let mut abi = JsonAbi::new();
        abi.events.insert("Transfer".to_string(), vec![Event::parse(&fixture.signature).unwrap()]);
        let original = abi.clone();
        assert!(recover_event_indexing(&mut abi, &args).await.is_err());
        assert_eq!(abi, original);
        assert_eq!(server.await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn full_decompile_keeps_lookup_disabled_and_recovers_all_outputs_when_enabled() {
        use crate::{decompile, interfaces::DecompilerArgsBuilder};
        use heimdall_common::utils::strings::encode_hex;
        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../../core/tests/testdata/decompile/indexed-events/bayc-approval-for-all.json"
        ))
        .unwrap();
        let sample = fixture.logs[0].clone();
        let block = sample.block_number.unwrap();
        let function = alloy_json_abi::Function::parse("emitEvent()").unwrap();
        let mut bytecode = vec![0x60, 0, 0x35, 0x60, 0xe0, 0x1c, 0x63];
        bytecode.extend_from_slice(function.selector().as_slice());
        bytecode.extend_from_slice(&[0x14, 0x60, 0x10, 0x57, 0, 0x5b, 0x7f]);
        bytecode.extend_from_slice(&sample.data().data);
        bytecode.extend_from_slice(&[0x60, 0, 0x52]);
        for topic in sample.topics().iter().rev() {
            bytecode.push(0x7f);
            bytecode.extend_from_slice(topic.as_slice());
        }
        bytecode.extend_from_slice(&[0x60, 32, 0x60, 0, 0xa3, 0]);
        let abi_path = std::env::temp_dir()
            .join(format!("heimdall-indexed-log-abi-{}.json", std::process::id()));
        let event = Event::parse(&fixture.signature).unwrap();
        let mut local_abi = JsonAbi::new();
        local_abi.events.insert(event.name.clone(), vec![event]);
        local_abi.functions.insert(function.name.clone(), vec![function]);
        std::fs::write(&abi_path, serde_json::to_vec(&local_abi).unwrap()).unwrap();
        let args = DecompilerArgsBuilder::new()
            .target(encode_hex(&bytecode))
            .abi(Some(abi_path.to_string_lossy().into_owned()))
            .include_solidity(true)
            .rpc_url("http://127.0.0.1:1".to_string())
            .build()
            .unwrap();
        let disabled = decompile(args.clone()).await.unwrap();
        assert!(disabled.abi.events.contains_key("ApprovalForAll"), "{:?}", disabled.abi.events);
        assert!(disabled.abi.events["ApprovalForAll"][0].inputs.iter().all(|input| !input.indexed));
        assert!(disabled.source.unwrap().contains("event ApprovalForAll(address, address, bool);"));
        let (url, server) = rpc_server(vec![serde_json::json!({"result": [sample]})]).await;
        let mut enabled = args;
        enabled.rpc_url = url;
        enabled.indexed_log_check = true;
        enabled.indexed_log_address = Some(fixture.address);
        enabled.indexed_log_from_block = Some(block);
        enabled.indexed_log_to_block = Some(block);
        let recovered = decompile(enabled.clone()).await.unwrap();
        assert_eq!(
            recovered.abi.events["ApprovalForAll"][0]
                .inputs
                .iter()
                .map(|input| input.indexed)
                .collect::<Vec<_>>(),
            vec![true, true, false]
        );
        assert!(recovered
            .source
            .unwrap()
            .contains("event ApprovalForAll(address indexed, address indexed, bool);"));
        let detailed = recovered
            .abi_with_details
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["type"] == "event")
            .unwrap();
        assert_eq!(detailed["inputs"][0]["indexed"], true);
        assert_eq!(server.await.unwrap().len(), 1);
        let (url, server) = rpc_server(vec![
            serde_json::json!({"error":{"code":-32000,"message":"query rejected"}}),
        ])
        .await;
        enabled.rpc_url = url;
        let unchanged = decompile(enabled).await.unwrap();
        assert!(unchanged.abi.events["ApprovalForAll"][0]
            .inputs
            .iter()
            .all(|input| !input.indexed));
        assert_eq!(server.await.unwrap().len(), 1);
        std::fs::remove_file(abi_path).unwrap();
    }

    #[test]
    fn enabled_lookup_requires_rpc_and_an_emitting_address() {
        let mut args = crate::interfaces::DecompilerArgsBuilder::new()
            .target("0x00".to_string())
            .build()
            .unwrap();
        assert!(validate_lookup(&args).unwrap_err().to_string().contains("--rpc-url"));
        args.rpc_url = "http://127.0.0.1:1".to_string();
        assert!(validate_lookup(&args).unwrap_err().to_string().contains("--indexed-log-address"));
        args.indexed_log_address = Some(Address::ZERO);
        assert_eq!(validate_lookup(&args).unwrap(), Address::ZERO);
    }
    #[tokio::test]
    async fn latest_bound_is_fixed_and_pages_do_not_overlap() {
        let (url, server) = rpc_server(vec![
            serde_json::json!({"result":"0x12c"}),
            serde_json::json!({"result":[]}),
            serde_json::json!({"result":[]}),
        ])
        .await;
        let args = crate::interfaces::DecompilerArgsBuilder::new()
            .target(Address::ZERO.to_string())
            .rpc_url(url)
            .indexed_log_check(true)
            .indexed_log_from_block(Some(100))
            .build()
            .unwrap();
        let mut abi = JsonAbi::new();
        abi.events.insert("Changed".to_string(), vec![Event::parse("Changed(address)").unwrap()]);
        let original = abi.clone();
        recover_event_indexing(&mut abi, &args).await.unwrap();
        assert_eq!(abi, original);
        let requests = server.await.unwrap();
        assert_eq!(requests[0]["method"], "eth_blockNumber");
        assert_eq!(requests[1]["params"][0]["fromBlock"], "0x65");
        assert_eq!(requests[1]["params"][0]["toBlock"], "0x12c");
        assert_eq!(requests[2]["params"][0]["fromBlock"], "0x64");
        assert_eq!(requests[2]["params"][0]["toBlock"], "0x64");
        for request in &requests[1..] {
            assert_eq!(
                request["params"][0]["topics"][0],
                abi.events["Changed"][0].selector().to_string()
            );
        }
    }
}
