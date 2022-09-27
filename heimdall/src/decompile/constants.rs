use regex::Regex;
use lazy_static::lazy_static;


lazy_static! {

    // The following regex is used as a detector for AND bitmasks
    pub static ref AND_BITMASK_REGEX: Regex = Regex::new(r"0x[0-9a-fA-F]* & ").unwrap();

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
///
/// @custom:donations Heimdall is open source and will always be free to use, so 
///                     donations are always appreciated if you find it helpful.
///                     0x6666666b0B46056247E7D6cbdb78287F4D12574d   OR   jbecker.eth
".to_string();

}