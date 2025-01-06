//! Module for compiler detection.

use std::fmt::Display;

use crate::{ether::bytecode::remove_pushbytes_from_bytecode, utils::iter::ByteSliceExt};
use tracing::{debug, trace, warn};

/// Compiler enum to represent the compiler used to compile the contract.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Compiler {
    /// Indicates that the contract was compiled using the Solidity compiler.
    Solc,
    /// Indicates that the contract was compiled using the Vyper compiler.
    Vyper,
    /// Indicates that the contract is a minimal proxy.
    Proxy,
    /// Indicates that the compiler could not be detected.
    Unknown,
}

impl Display for Compiler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Compiler::Solc => write!(f, "solc"),
            Compiler::Vyper => write!(f, "vyper"),
            Compiler::Proxy => write!(f, "proxy"),
            Compiler::Unknown => write!(f, "unknown"),
        }
    }
}

/// returns the compiler version used to compile the contract.
/// for example: (solc, 0.8.10) or (vyper, 0.2.16)
///
/// Research:
/// https://jbecker.dev/research/evm-compiler-fingerprinting
pub fn detect_compiler(bytecode: &[u8]) -> (Compiler, String) {
    let mut compiler = Compiler::Unknown;
    let mut version = "unknown".to_string();

    // Previously known heuristic: perform prefix check for rough version matching
    if bytecode.starts_with(&[
        0x36, 0x60, 0x00, 0x60, 0x00, 0x37, 0x61, 0x10, 0x00, 0x60, 0x00, 0x36, 0x60, 0x00, 0x73,
    ]) {
        compiler = Compiler::Proxy;
        version = "vyper".to_string();
    } else if bytecode.starts_with(&[0x60, 0x04, 0x36, 0x10, 0x15]) {
        compiler = Compiler::Vyper;
        version = "0.2.0-0.2.4,0.2.11-0.3.3".to_string();
    } else if bytecode.starts_with(&[0x34, 0x15, 0x61, 0x00, 0x0a]) {
        compiler = Compiler::Vyper;
        version = "0.2.5-0.2.8".to_string();
    } else if bytecode.starts_with(&[0x73, 0x1b, 0xf7, 0x97]) {
        compiler = Compiler::Solc;
        version = "0.4.10-0.4.24".to_string();
    } else if bytecode.starts_with(&[0x60, 0x80, 0x60, 0x40, 0x52]) {
        compiler = Compiler::Solc;
        version = "0.4.22+".to_string();
    } else if bytecode.starts_with(&[0x60, 0x60, 0x60, 0x40, 0x52]) {
        compiler = Compiler::Solc;
        version = "0.4.11-0.4.21".to_string();
    } else if bytecode.contains_slice(&[0x76, 0x79, 0x70, 0x65, 0x72]) {
        compiler = Compiler::Vyper;
    } else if bytecode.contains_slice(&[0x73, 0x6f, 0x6c, 0x63]) {
        compiler = Compiler::Solc;
    }

    // Remove `PUSHN [u8; n]` bytes so we are left with only operations
    let pruned_bytecode =
        remove_pushbytes_from_bytecode(alloy::primitives::Bytes::from_iter(bytecode))
            .expect("invalid bytecode");

    // detect minimal proxies
    if pruned_bytecode.eq(&vec![
        0x36, 0x3d, 0x3d, 0x37, 0x3d, 0x3d, 0x3d, 0x36, 0x3d, 0x73, 0x5a, 0xf4, 0x3d, 0x82, 0x80,
        0x3e, 0x90, 0x3d, 0x91, 0x60, 0x57, 0xfd, 0x5b, 0xf3,
    ]) {
        compiler = Compiler::Proxy;
        version = "minimal".to_string();
    }

    // heuristics are in the form of (sequence, solc confidence, vyper confidence)
    let heuristics = [
        ([0x80, 0x63, 0x14, 0x61, 0x57], 0.9447, 0.0),
        ([0x14, 0x61, 0x57, 0x80, 0x63], 0.9371, 0.0),
        ([0x61, 0x57, 0x80, 0x63, 0x14], 0.9371, 0.0),
        ([0x57, 0x80, 0x63, 0x14, 0x61], 0.9371, 0.0),
        ([0x54, 0x60, 0x52, 0x60, 0x60], 0.00, 0.3103),
        ([0x60, 0x54, 0x60, 0x52, 0x60], 0.00, 0.3054),
        ([0x61, 0x52, 0x61, 0x51, 0x61], 0.00, 0.2894),
        ([0x61, 0x51, 0x61, 0x52, 0x60], 0.00, 0.2816),
        ([0x61, 0x52, 0x60, 0x61, 0x52], 0.00, 0.2734),
        ([0x90, 0x50, 0x90, 0x50, 0x81], 0.00, 0.2727),
        ([0x61, 0x52, 0x7f, 0x61, 0x52], 0.00, 0.2656),
    ];

    // for each heuristic, check if the bytecode contains the sequence and increment the confidence
    // for that compiler. the compiler with the highest confidence is chosen
    let (mut solc_confidence, mut vyper_confidence) = (0.0, 0.0);
    for (sequence, solc, vyper) in heuristics.iter() {
        if pruned_bytecode.contains_slice(sequence) {
            solc_confidence += solc;
            vyper_confidence += vyper;
        }
    }

    if solc_confidence != 0.0 && solc_confidence > vyper_confidence {
        compiler = Compiler::Solc;
    } else if vyper_confidence != 0.0 && vyper_confidence > solc_confidence {
        compiler = Compiler::Vyper;
    }

    // Previously known heuristic: check for cbor encoded compiler metadata
    // check for cbor encoded compiler metadata
    // https://cbor.io
    if bytecode.contains_slice(&[0x73, 0x6f, 0x6c, 0x63, 0x43]) {
        let compiler_version = bytecode.split_by_slice(&[0x73, 0x6f, 0x6c, 0x63, 0x43]);

        if compiler_version.len() > 1 {
            if let Some(encoded_version) = compiler_version.get(1).and_then(|last| last.get(0..3)) {
                version = encoded_version
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(".");
                compiler = Compiler::Solc;
            }

            trace!("exact compiler version match found due to cbor encoded metadata: {}", version);
        }
    } else if bytecode.contains_slice(&[0x76, 0x79, 0x70, 0x65, 0x72, 0x83]) {
        let compiler_version = bytecode.split_by_slice(&[0x76, 0x79, 0x70, 0x65, 0x72, 0x83]);

        if compiler_version.len() > 1 {
            if let Some(encoded_version) = compiler_version.get(1).and_then(|last| last.get(0..3)) {
                version = encoded_version
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(".");
                compiler = Compiler::Vyper;
            }

            trace!("exact compiler version match found due to cbor encoded metadata");
        }
    }

    debug!("detected compiler {compiler} {version}.");

    // if not Solidity, warn
    if compiler != Compiler::Solc {
        warn!("{} is not fully supported by heimdall", compiler);
    }

    (compiler, version.trim_end_matches('.').to_string())
}

#[cfg(test)]
mod test_compiler {

    use super::*;

    #[test]
    fn test_detect_compiler_proxy_vyper() {
        let bytecode = &[
            0x36, 0x60, 0x00, 0x60, 0x00, 0x37, 0x61, 0x10, 0x00, 0x60, 0x00, 0x36, 0x60, 0x00,
            0x73,
        ];
        let expected_result = (Compiler::Proxy, "vyper".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_vyper_range_1() {
        let bytecode = &[0x60, 0x04, 0x36, 0x10, 0x15];
        let expected_result = (Compiler::Vyper, "0.2.0-0.2.4,0.2.11-0.3.3".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_vyper_range_2() {
        let bytecode = &[0x34, 0x15, 0x61, 0x00, 0x0a];
        let expected_result = (Compiler::Vyper, "0.2.5-0.2.8".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_solc_range_1() {
        let bytecode = &[0x73, 0x1b, 0xf7, 0x97];
        let expected_result = (Compiler::Solc, "0.4.10-0.4.24".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_solc_range_2() {
        let bytecode = &[0x60, 0x80, 0x60, 0x40, 0x52];
        let expected_result = (Compiler::Solc, "0.4.22+".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_solc_range_3() {
        let bytecode = &[0x60, 0x60, 0x60, 0x40, 0x52];
        let expected_result = (Compiler::Solc, "0.4.11-0.4.21".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_vyper() {
        let bytecode = &[0x76, 0x79, 0x70, 0x65, 0x72];
        let expected_result = (Compiler::Vyper, "unknown".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_solc() {
        let bytecode = &[0x73, 0x6f, 0x6c, 0x63];
        let expected_result = (Compiler::Solc, "unknown".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_solc_metadata_unknown() {
        let bytecode = &[0x73, 0x6f, 0x6c, 0x63, 0x43, 0x4d, 0x4e];
        let expected_result = (Compiler::Solc, "unknown".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_solc_metadata_known() {
        let bytecode = &[0x73, 0x6f, 0x6c, 0x63, 0x43, 0x4d, 0x4e, 0x69, 0x30, 0x30];
        let expected_result = (Compiler::Solc, "77.78.105".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_vyper_metadata() {
        let bytecode = &[0x76, 0x79, 0x70, 0x65, 0x72, 0x83, 0x31, 0x35, 0x35, 0x30, 0x30];
        let expected_result = (Compiler::Vyper, "49.53.53".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }
}
