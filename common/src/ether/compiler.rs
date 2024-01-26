use std::fmt::Display;

use crate::{debug_max, utils::iter::ByteSliceExt};

#[derive(Debug, PartialEq, Clone)]
pub enum Compiler {
    Solc,
    Vyper,
    Proxy,
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

// returns the compiler version used to compile the contract.
// for example: (solc, 0.8.10) or (vyper, 0.2.16)
pub fn detect_compiler(bytecode: &[u8]) -> (Compiler, String) {
    let mut compiler = Compiler::Unknown;
    let mut version = "unknown".to_string();

    // perfom prefix check for rough version matching
    if bytecode.starts_with(&[0x36, 0x3d, 0x3d, 0x37, 0x3d, 0x3d, 0x3d, 0x36, 0x3d, 0x73]) ||
        bytecode.starts_with(&[0x5f, 0x5f, 0x36, 0x5f, 0x5f, 0x37])
    {
        compiler = Compiler::Proxy;
        version = "minimal".to_string();
    } else if bytecode.starts_with(&[
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

    // TODO: add more heuristics for compiler version detection

    // check for cbor encoded compiler metadata
    // https://cbor.io
    if compiler == Compiler::Solc {
        let compiler_version = bytecode.split_by_slice(&[0x73, 0x6f, 0x6c, 0x63, 0x43]);

        if compiler_version.len() > 1 {
            if let Some(encoded_version) = compiler_version.get(1).and_then(|last| last.get(0..3)) {
                version = encoded_version
                    .into_iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(".");
            }

            debug_max!(
                "exact compiler version match found due to cbor encoded metadata: {}",
                version
            );
        }
    } else if compiler == Compiler::Vyper {
        let compiler_version = bytecode.split_by_slice(&[0x76, 0x79, 0x70, 0x65, 0x72, 0x83]);

        if compiler_version.len() > 1 {
            if let Some(encoded_version) = compiler_version.get(1).and_then(|last| last.get(0..3)) {
                version = encoded_version
                    .into_iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(".");
            }

            debug_max!(
                "exact compiler version match found due to cbor encoded metadata: {}",
                version
            );
        }
    }

    (compiler, version.trim_end_matches('.').to_string())
}

#[cfg(test)]
mod test_compiler {
    use super::*;

    #[test]
    fn test_detect_compiler_proxy_minimal() {
        let bytecode = &[0x36, 0x3d, 0x3d, 0x37, 0x3d, 0x3d, 0x3d, 0x36, 0x3d, 0x73];
        let expected_result = (Compiler::Proxy, "minimal".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

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
