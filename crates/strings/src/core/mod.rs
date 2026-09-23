use std::io::{self, Write};

use crate::{Error, StringsArgs};

/// Extracts printable strings from a hex target, bytecode file, or contract address.
///
/// Writes one string per line into the caller's writer. The caller controls buffering
/// and flushing; this function never writes to stdout or loads CLI configuration.
/// Address targets use the RPC URL supplied in `args`.
pub async fn strings(args: &StringsArgs, output: &mut impl Write) -> Result<(), Error> {
    let bytecode = args.get_bytecode().await?;
    write_strings(&bytecode, args.min_length.get(), args.full_scan, output)?;
    Ok(())
}

/// Writes printable ASCII runs from PUSH data, one per line, in bytecode order.
///
/// Walks instructions linearly and scans each PUSH1–PUSH32 payload independently.
/// Truncated payloads use only the available bytes. Set `full_scan` to scan all bytes,
/// including opcodes and data outside PUSH payloads. Runs shorter than `min_length`
/// and empty runs are omitted. Matching slices are written directly without allocation.
/// Output errors are propagated to the caller.
pub fn write_strings(
    bytecode: &[u8],
    min_length: usize,
    full_scan: bool,
    output: &mut impl Write,
) -> io::Result<()> {
    if full_scan {
        return write_ascii_runs(bytecode, min_length, output);
    }

    let mut remaining = bytecode;
    while let Some((&opcode, rest)) = remaining.split_first() {
        remaining = rest;
        if (0x60..=0x7f).contains(&opcode) {
            let size = usize::from(opcode - 0x5f).min(remaining.len());
            let (payload, rest) = remaining.split_at(size);
            write_ascii_runs(payload, min_length, output)?;
            remaining = rest;
        }
    }
    Ok(())
}

fn write_ascii_runs(bytes: &[u8], min_length: usize, output: &mut impl Write) -> io::Result<()> {
    for string in bytes.split(|byte| !(b' '..=b'~').contains(byte)) {
        if !string.is_empty() && string.len() >= min_length {
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
