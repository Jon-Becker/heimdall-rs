use std::collections::BTreeSet;

/// Tracks the selector-preserving branches of a conventional calldata dispatcher.
#[derive(Default)]
pub(super) struct Selectors {
    dispatch: bool,
    branch_targets: BTreeSet<usize>,
    resume_at: usize,
    selector_at: usize,
}

impl Selectors {
    pub(super) fn excludes(&mut self, code: &[u8], pc: usize) -> bool {
        if pc < self.resume_at {
            return pc == self.selector_at;
        }

        let tail = &code[pc..];
        if tail[0] == 0x5b {
            self.dispatch = self.branch_targets.remove(&pc);
            return false;
        }

        // CALLDATALOAD(0) >> 224 puts the function selector on top of the stack.
        let load_length = if tail.starts_with(&[0x5f, 0x35, 0x60, 0xe0, 0x1c]) {
            5
        } else if tail.starts_with(&[0x60, 0x00, 0x35, 0x60, 0xe0, 0x1c]) {
            6
        } else {
            0
        };
        if load_length != 0 {
            self.dispatch = true;
            self.resume_at = pc + load_length;
            self.selector_at = usize::MAX;
            return false;
        }

        if self.dispatch {
            // DUP1 PUSH4 selector EQ/LT/GT PUSHn destination JUMPI leaves the
            // original calldata selector on the stack along both outgoing edges.
            if let [0x80, 0x63, _, _, _, _, comparison @ (0x10 | 0x11 | 0x14), ..] = tail {
                if let Some((target, end)) = branch(code, pc + 7) {
                    if *comparison != 0x14 {
                        self.branch_targets.insert(target);
                    }
                    self.selector_at = pc + 1;
                    self.resume_at = end;
                    return false;
                }
            }
            // A zero-selector check also preserves the original selector.
            if tail.starts_with(&[0x80, 0x15]) {
                if let Some((_, end)) = branch(code, pc + 2) {
                    self.selector_at = usize::MAX;
                    self.resume_at = end;
                    return false;
                }
            }
            // Never carry inferred stack contents through unrecognized instructions.
            self.dispatch = false;
        }
        false
    }
}

fn branch(code: &[u8], pc: usize) -> Option<(usize, usize)> {
    let (target, end) = push(code, pc)?;
    (target > end && code.get(end) == Some(&0x57) && code.get(target) == Some(&0x5b))
        .then_some((target, end + 1))
}

/// Recognizes a complete, constant Panic(uint256) revert buffer. Matching the
/// selector's value alone would also suppress a perfectly valid "NH{q" string.
pub(super) fn is_panic(code: &[u8], pc: usize, payload: &[u8]) -> bool {
    let (mut next, offset) = match payload {
        [0x4e, 0x48, 0x7b, 0x71, padding @ ..]
            if padding.len() == 28 && padding.iter().all(|&byte| byte == 0) =>
        {
            (pc + 33, 0)
        }
        [0x4e, 0x48, 0x7b, 0x71] => {
            let next = pc + 5;
            if code.get(next..next + 3) == Some(&[0x60, 0xe0, 0x1b]) {
                (next + 3, 0)
            } else {
                (next, 28)
            }
        }
        _ => return false,
    };
    let Some((0, end)) = push(code, next) else { return false };
    if code.get(end) != Some(&0x52) {
        return false;
    }
    next = end + 1;
    let Some((0x00 | 0x01 | 0x11 | 0x12 | 0x21 | 0x22 | 0x31 | 0x32 | 0x41 | 0x51, end)) =
        push(code, next)
    else {
        return false;
    };
    let Some((argument_offset, end)) = push(code, end) else { return false };
    if argument_offset != offset + 4 || code.get(end) != Some(&0x52) {
        return false;
    }
    let Some((36, end)) = push(code, end + 1) else { return false };
    let Some((revert_offset, end)) = push(code, end) else { return false };
    revert_offset == offset && code.get(end) == Some(&0xfd)
}

/// Reads a complete constant PUSH, rejecting overflow and truncated operands.
fn push(code: &[u8], pc: usize) -> Option<(usize, usize)> {
    let opcode = *code.get(pc)?;
    if !(0x5f..=0x7f).contains(&opcode) {
        return None;
    }
    let end = pc + 1 + usize::from(opcode - 0x5f);
    let value = code
        .get(pc + 1..end)?
        .iter()
        .try_fold(0usize, |value, &byte| value.checked_mul(256)?.checked_add(usize::from(byte)))?;
    Some((value, end))
}
