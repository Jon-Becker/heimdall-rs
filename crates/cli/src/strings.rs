use std::{
    io::{self, BufWriter, Write},
    num::NonZeroUsize,
};

use alloy::primitives::Address;
use clap::Args;
use eyre::{Result, WrapErr};
use heimdall_common::ether::{
    bytecode::{get_bytecode_from_target, write_strings},
    rpc::get_code,
};
use heimdall_config::{parse_url_arg, Configuration};

#[derive(Debug, Args)]
pub(crate) struct StringsArgs {
    /// Hex bytecode, a file containing hex bytecode, or a contract address.
    target: String,

    /// The RPC provider to use for fetching contract bytecode (URL or MESC endpoint).
    #[clap(long, short, value_parser = parse_url_arg, default_value = "", hide_default_value = true)]
    rpc_url: String,

    /// Minimum number of consecutive printable ASCII characters.
    #[clap(long, short = 'n', default_value = "4")]
    min_length: NonZeroUsize,

    /// Scan all bytecode instead of only PUSH instruction data.
    #[clap(long)]
    full_scan: bool,
}

pub(crate) async fn run(args: &StringsArgs) -> Result<()> {
    let bytecode = if let Ok(address) = args.target.parse::<Address>() {
        let rpc_url = if args.rpc_url.is_empty() {
            Configuration::load().wrap_err("failed to load configuration")?.rpc_url
        } else {
            args.rpc_url.clone()
        };
        get_code(address, &rpc_url).await
    } else {
        get_bytecode_from_target(&args.target, "", "").await
    }
    .wrap_err("failed to load bytecode")?;
    let mut output = BufWriter::new(io::stdout().lock());
    match write_strings(&bytecode, args.min_length.get(), args.full_scan, &mut output)
        .and_then(|()| output.flush())
    {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result.wrap_err("failed to write strings"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_runs_in_order_including_duplicates_and_final_run() {
        let mut output = Vec::new();
        write_strings(b"first\0abc\xffwith spaces!\x7ffirst\nlast", 4, true, &mut output).unwrap();
        assert_eq!(output, b"first\nwith spaces!\nfirst\nlast\n");
    }

    #[test]
    fn respects_minimum_length_and_ascii_boundaries() {
        let mut output = Vec::new();
        write_strings(b"\x1f !~\x7f\x80a\tb\rc\nd", 3, true, &mut output).unwrap();
        assert_eq!(output, b" !~\n");
        output.clear();
        write_strings(b"a\0bc", 1, true, &mut output).unwrap();
        assert_eq!(output, b"a\nbc\n");
    }

    #[test]
    fn empty_or_short_runs_produce_no_output() {
        for bytecode in [b"".as_slice(), b"\0\xff\x7f", b"abc\0def"] {
            let mut output = Vec::new();
            write_strings(bytecode, 4, true, &mut output).unwrap();
            assert!(output.is_empty());
        }
    }

    #[test]
    fn propagates_output_errors() {
        for full_scan in [false, true] {
            let mut output = [0; 2];
            let error =
                write_strings(b"\x64hello", 4, full_scan, &mut output.as_mut_slice()).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::WriteZero);
        }
    }

    #[test]
    fn push_scan_excludes_opcodes_and_keeps_payloads_separate() {
        let mut output = Vec::new();
        write_strings(b"RP@\x63abcd\x63wxyz", 4, false, &mut output).unwrap();
        assert_eq!(output, b"abcd\nwxyz\n");
        output.clear();
        write_strings(b"\x61ab\x61cd", 4, false, &mut output).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn push_scan_handles_every_width_and_skips_push0() {
        for width in 1..=32 {
            let mut bytecode = vec![0x5f, 0x5f + width];
            bytecode.extend(std::iter::repeat_n(b'x', usize::from(width)));
            bytecode.extend_from_slice(b"\x5f\x63tail");
            let mut output = Vec::new();
            write_strings(&bytecode, 1, false, &mut output).unwrap();
            assert_eq!(output, format!("{}\ntail\n", "x".repeat(usize::from(width))).as_bytes());
        }
    }

    #[test]
    fn push_scan_handles_empty_and_truncated_payloads() {
        for bytecode in [b"".as_slice(), b"\x7f", b"\x5f"] {
            let mut output = Vec::new();
            write_strings(bytecode, 1, false, &mut output).unwrap();
            assert!(output.is_empty());
        }
        let mut output = Vec::new();
        write_strings(b"\x7fhello", 4, false, &mut output).unwrap();
        assert_eq!(output, b"hello\n");
    }
}
