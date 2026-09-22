use std::{
    io::{self, BufWriter, Write},
    num::NonZeroUsize,
};

use alloy::primitives::Address;
use clap::Args;
use eyre::{Result, WrapErr};
use heimdall_common::ether::{bytecode::get_bytecode_from_target, rpc::get_code};
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
    match write_strings(&bytecode, args.min_length.get(), &mut output).and_then(|()| output.flush())
    {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result.wrap_err("failed to write strings"),
    }
}

fn write_strings(bytecode: &[u8], min_length: usize, output: &mut impl Write) -> io::Result<()> {
    for string in bytecode.split(|byte| !(b' '..=b'~').contains(byte)) {
        if string.len() >= min_length {
            output.write_all(string)?;
            output.write_all(b"\n")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_runs_in_order_including_duplicates_and_final_run() {
        let mut output = Vec::new();
        write_strings(b"first\0abc\xffwith spaces!\x7ffirst\nlast", 4, &mut output).unwrap();
        assert_eq!(output, b"first\nwith spaces!\nfirst\nlast\n");
    }

    #[test]
    fn respects_minimum_length_and_ascii_boundaries() {
        let mut output = Vec::new();
        write_strings(b"\x1f !~\x7f\x80a\tb\rc\nd", 3, &mut output).unwrap();
        assert_eq!(output, b" !~\n");
        output.clear();
        write_strings(b"a\0bc", 1, &mut output).unwrap();
        assert_eq!(output, b"a\nbc\n");
    }

    #[test]
    fn empty_or_short_runs_produce_no_output() {
        for bytecode in [b"".as_slice(), b"\0\xff\x7f", b"abc\0def"] {
            let mut output = Vec::new();
            write_strings(bytecode, 4, &mut output).unwrap();
            assert!(output.is_empty());
        }
    }

    #[test]
    fn propagates_output_errors() {
        let mut output = [0; 2];
        let error = write_strings(b"hello", 4, &mut output.as_mut_slice()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WriteZero);
    }
}
