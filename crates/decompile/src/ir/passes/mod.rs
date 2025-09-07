pub mod algebraic;
pub mod bitwise;
pub mod constant_fold;
pub mod control_flow;
pub mod copy_prop;
pub mod cse;
pub mod dce;
pub mod strength;
pub mod type_inference;

use crate::{ir::types::Function, Error};

pub fn run_all_passes(mut ir: Function) -> Result<Function, Error> {
    // Phase 1: Simplification
    ir = constant_fold::run(ir)?;
    ir = algebraic::run(ir)?;
    ir = bitwise::run(ir)?;
    ir = strength::run(ir)?;

    // Phase 2: Cleanup
    ir = dce::run(ir)?;
    ir = cse::run(ir)?;
    ir = copy_prop::run(ir)?;

    // Phase 3: Structuring
    ir = control_flow::run(ir)?;
    ir = type_inference::run(ir)?;

    Ok(ir)
}