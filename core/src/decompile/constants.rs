use fancy_regex::Regex;
use lazy_static::lazy_static;

lazy_static! {

    // The following regex is used as a detector for AND bitmasks
    pub static ref AND_BITMASK_REGEX: Regex = Regex::new(r"\(0x([a-fA-F0-9]{2}){1,32}\) & ").unwrap();
    pub static ref AND_BITMASK_REGEX_2: Regex = Regex::new(r" & \(0x([a-fA-F0-9]{2}){1,32}\)").unwrap();

    // used to detect constant values
    pub static ref CONSTANT_REGEX: Regex = Regex::new(r"^(?:(?![memorystorage\[\]]).)*$").unwrap();

    // used to detect non-zero bytes within a word
    pub static ref NON_ZERO_BYTE_REGEX: Regex = Regex::new(r"[a-fA-F0-9][a-fA-F1-9]").unwrap();

    // detects a parenthesis enclosed expression
    pub static ref ENCLOSED_EXPRESSION_REGEX: Regex = Regex::new(r"\(.*\)").unwrap();

    // detects a memory access
    pub static ref MEM_ACCESS_REGEX: Regex = Regex::new(r"memory\[.*\]").unwrap();

    // detects a storage access
    pub static ref STORAGE_ACCESS_REGEX: Regex = Regex::new(r"storage\[.*\]").unwrap();

    // detects division by 1
    pub static ref DIV_BY_ONE_REGEX: Regex = Regex::new(r" \/ 0x01(?!\d)").unwrap();

    // detects multiplication by 1
    pub static ref MUL_BY_ONE_REGEX: Regex = Regex::new(r"\b0x01\b\s*\*\s*| \*\s*\b0x01\b").unwrap();

    // memory variable regex
    pub static ref MEM_VAR_REGEX: Regex = Regex::new(r"^var_[a-zA-Z]{1,2}$").unwrap();

    // extracts commas within a certain expression, not including commas within parentheses
    pub static ref ARGS_SPLIT_REGEX: Regex = Regex::new(r",\s*(?![^()]*\))").unwrap();

    // used to detect compiler size checks
    pub static ref VARIABLE_SIZE_CHECK_REGEX: Regex = Regex::new(r"!?\(?0(x01)? < [a-zA-Z0-9_\[\]]+\.length\)?").unwrap();

    pub static ref DECOMPILED_SOURCE_HEADER_SOL: String =
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
///                       https://heimdall.rs
".to_string();

pub static ref DECOMPILED_SOURCE_HEADER_YUL: String =
"/// @title            Decompiled Contract
/// @author           Jonathan Becker <jonathan@jbecker.dev>
/// @custom:version   heimdall-rs v{}
///
/// @notice           This contract was decompiled using the heimdall-rs decompiler.
///                     It was generated directly by tracing the EVM opcodes from this contract.
///                     As a result, it may not compile or even be valid yul code.
///                     Despite this, it should be obvious what each function does. Overall
///                     logic should have been preserved throughout decompiling.
///
/// @custom:github    You can find the open-source decompiler here:
///                       https://heimdall.rs

object \"DecompiledContract\" {
object \"runtime\" {
code {

function selector() -> s {
s := div(calldataload(0), 0x100000000000000000000000000000000000000000000000000000000)
}

function castToAddress(x) -> a {
a := and(x, 0xffffffffffffffffffffffffffffffffffffffff)
}

switch selector()".to_string();

}
