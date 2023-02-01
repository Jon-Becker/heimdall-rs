use fancy_regex::Regex;
use lazy_static::lazy_static;


lazy_static! {

    // The following regex is used as a detector for AND bitmasks
    pub static ref AND_BITMASK_REGEX: Regex = Regex::new(r"\(0x([a-fA-F0-9]{2}){1,32}\) & ").unwrap();
    pub static ref AND_BITMASK_REGEX_2: Regex = Regex::new(r" & \(0x([a-fA-F0-9]{2}){1,32}\)").unwrap();

    // used to detect non-zero bytes within a word
    pub static ref NON_ZERO_BYTE_REGEX: Regex = Regex::new(r"[a-fA-F0-9][a-fA-F1-9]").unwrap();

    // detects a parenthesis enclosed expression
    pub static ref ENCLOSED_EXPRESSION_REGEX: Regex = Regex::new(r"\(.*\)").unwrap();

    // detects a memory access
    pub static ref MEM_ACCESS_REGEX: Regex = Regex::new(r"memory\[.*\]").unwrap();
    
    pub static ref DECOMPILED_SOURCE_HEADER: String = 
"// SPDX-License-Identifier: MIT
pragma solidity >=0.8.0;

/// @title            Decompiled Contract
/// @author           Jonathan Becker <jonathan@jbecker.dev>
/// @custom:version   heimdall-rs v{}
///
/// @notice           This contract was decompiled using the heimdall-rs decompiler.
///                     It was generated directly by tracing the EVM opcodes from this contract.
///                     As a result, it may not compile or even be valid solidity code.
///                     Despite this, it should be obvious what each function does. Overall
///                     logic should have been preserved throughout decompiling.
///
/// @custom:github    You can find the open-source decompiler here:
///                       https://github.com/Jon-Becker/heimdall-rs
".to_string();

}