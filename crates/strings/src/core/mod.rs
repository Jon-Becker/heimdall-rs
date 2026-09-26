use std::io::{self, Write};

use crate::{Error, StringsArgs};

mod selectors;

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
/// and empty runs are omitted. Recognized dispatcher and panic-revert selectors
/// are excluded in PUSH mode. Matching slices are written without allocating strings.
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

    let mut selectors = selectors::Selectors::default();
    let mut remaining = bytecode;
    while let Some((&opcode, rest)) = remaining.split_first() {
        let pc = bytecode.len() - remaining.len();
        let selector = selectors.excludes(bytecode, pc);
        remaining = rest;
        if (0x60..=0x7f).contains(&opcode) {
            let size = usize::from(opcode - 0x5f).min(remaining.len());
            let (payload, rest) = remaining.split_at(size);
            if !selector && !selectors::is_panic(bytecode, pc, payload) {
                write_ascii_runs(payload, min_length, output)?;
            }
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

    fn extract(code: &[u8], full_scan: bool) -> Vec<u8> {
        let mut output = Vec::new();
        write_strings(code, 4, full_scan, &mut output).unwrap();
        output
    }

    #[test]
    fn filters_dispatch_comparisons_and_preserves_identical_literal() {
        for comparison in [0x10, 0x11] {
            let mut code = vec![0x60, 0, 0x35, 0x60, 0xe0, 0x1c];
            code.extend_from_slice(b"\x80\x63jbxB");
            code.extend_from_slice(&[comparison, 0x60, 17, 0x57, 0, 0x5b]);
            code.extend_from_slice(b"\x80\x63jbxB\x14\x60\x1d\x57\x00\x5b\x63jbxB");
            assert_eq!(extract(&code, false), b"jbxB\n");
            assert_eq!(extract(&code, true).windows(4).filter(|s| *s == b"jbxB").count(), 3);
        }
    }

    #[test]
    fn preserves_comparisons_without_calldata_selector_provenance() {
        for prefix in [
            [0x60, 4, 0x35, 0x60, 0xe0, 0x1c], // argument word, not selector
            [0x60, 0, 0x35, 0x60, 0xc0, 0x1c], // different bit range
            [0x60, 0, 0x30, 0x60, 0xe0, 0x1c], // address, not calldata
        ] {
            let mut code = prefix.to_vec();
            code.extend_from_slice(b"\x80\x63jbxB\x14\x60\x11\x57\x00\x5b");
            assert_eq!(extract(&code, false), b"jbxB\n");
        }
        for barrier in [0x50, 0x5b, 0x56, 0x00] {
            let mut code = vec![0x60, 0, 0x35, 0x60, 0xe0, 0x1c, barrier];
            code.extend_from_slice(b"\x80\x63jbxB\x14\x60\x12\x57\x00\x5b");
            assert_eq!(extract(&code, false), b"jbxB\n");
        }
    }

    #[test]
    fn handles_push0_and_zero_selector_dispatch() {
        let code = b"\x5f\x35\x60\xe0\x1c\x80\x15\x60\x15\x57\x80\x63jbxB\x14\x60\x15\x57\x00\x5b";
        assert!(extract(code, false).is_empty());
    }

    #[test]
    fn preserves_selectors_in_malformed_branches() {
        let code = b"\x60\x00\x35\x60\xe0\x1c\x80\x63jbxB\x14\x60\x11\x57\x00\x5b";
        for end in 12..code.len() {
            assert_eq!(extract(&code[..end], false), b"jbxB\n");
        }
        let mut overflow = code[..13].to_vec();
        overflow.push(0x7f);
        overflow.extend_from_slice(&[0xff; 32]);
        overflow.extend_from_slice(b"\x57\x5b");
        assert_eq!(extract(&overflow, false), b"jbxB\n");
    }

    fn panic_revert() -> Vec<u8> {
        let mut code = b"\x7fNH{q".to_vec();
        code.extend_from_slice(&[0; 28]);
        code.extend_from_slice(b"\x5f\x52\x60\x11\x60\x04\x52\x60\x24\x5f\xfd");
        code
    }

    #[test]
    fn filters_complete_panic_reverts_with_both_selector_alignments() {
        let padded = panic_revert();
        let shifted = b"\x63NH{q\x60\xe0\x1b\x60\x00\x52\x60\x12\x60\x04\x52\x60\x24\x60\x00\xfd";
        let unshifted = b"\x63NH{q\x5f\x52\x60\x32\x60\x20\x52\x60\x24\x60\x1c\xfd";
        for code in [padded.as_slice(), shifted, unshifted] {
            assert!(extract(code, false).is_empty());
            assert_eq!(extract(code, true).windows(4).filter(|s| *s == b"NH{q").count(), 1);
        }
    }

    #[test]
    fn preserves_panic_values_without_complete_revert_context() {
        let code = panic_revert();
        for end in 5..code.len() {
            assert_eq!(extract(&code[..end], false), b"NH{q\n");
        }
        // Alter each structural requirement, including padding, offsets and REVERT.
        for (index, byte) in [
            (32, 1),
            (33, 0x50),
            (34, 0x53),
            (36, 0xff),
            (38, 8),
            (39, 0x53),
            (41, 32),
            (42, 0x50),
            (43, 0xf3),
        ] {
            let mut changed = code.clone();
            changed[index] = byte;
            assert_eq!(extract(&changed, false), b"NH{q\n", "mutation at {index}");
        }
        assert_eq!(extract(b"\x63NH{q\x00\x67\x07Seaport", false), b"NH{q\nSeaport\n");
    }
}
