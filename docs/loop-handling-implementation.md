# Loop Handling Implementation for Heimdall-rs

This document outlines the implementation plan for improving loop detection and reconstruction in the heimdall-rs decompiler.

## Problem Statement

Currently, when the decompiler detects a loop during symbolic execution, it terminates that execution path entirely (`return Ok(None)`). This causes:

1. Loss of loop structure in decompiled output
2. Inclusion of overflow check code as spurious `require` statements
3. Incorrect function mutability detection (e.g., `view` instead of state-modifying)

## Implementation Overview

The fix requires changes across multiple crates:

```
heimdall-vm/src/ext/exec/
├── mod.rs           # Capture loop info instead of discarding
├── loop_analysis.rs # NEW: Loop detection utilities
└── jump_frame.rs    # Extend with loop metadata

heimdall-cfg/src/core/
└── graph.rs         # Add back-edge detection

heimdall-decompile/src/
├── core/analyze.rs  # Pass loop info to heuristics
└── utils/heuristics/
    ├── mod.rs       # Register loop heuristic
    └── loops.rs     # NEW: Loop reconstruction heuristic
```

---

## Phase 1: Data Structures

### 1.1 Loop Information Structure

**File:** `crates/vm/src/ext/exec/loop_analysis.rs` (new file)

```rust
use alloy::primitives::U256;
use crate::core::vm::State;

/// Represents a detected loop in the control flow
#[derive(Clone, Debug, Default)]
pub struct LoopInfo {
    /// Program counter of the loop header (JUMPDEST target)
    pub header_pc: u128,

    /// Program counter of the conditional jump (JUMPI)
    pub condition_pc: u128,

    /// The solidified loop condition (e.g., "i < arg0")
    pub condition: String,

    /// The negated condition for while-loop form
    pub exit_condition: String,

    /// Detected induction variable name, if any
    pub induction_var: Option<InductionVariable>,

    /// Operations captured from one iteration of the loop body
    pub body_operations: Vec<State>,

    /// Whether this appears to be a bounded loop (for) vs unbounded (while)
    pub is_bounded: bool,

    /// Storage slots modified within the loop
    pub modified_storage: Vec<U256>,

    /// Memory locations modified within the loop
    pub modified_memory: Vec<U256>,
}

/// Represents a loop induction variable (counter)
#[derive(Clone, Debug)]
pub struct InductionVariable {
    /// The variable identifier (e.g., "var_a" or stack position)
    pub name: String,

    /// Initial value expression
    pub init: String,

    /// Step expression (usually "+ 1" or "- 1")
    pub step: String,

    /// Bound expression (e.g., "arg0")
    pub bound: Option<String>,

    /// Whether counting up or down
    pub direction: InductionDirection,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InductionDirection {
    Ascending,   // i++
    Descending,  // i--
    Unknown,
}

impl LoopInfo {
    pub fn new(header_pc: u128, condition_pc: u128, condition: String) -> Self {
        Self {
            header_pc,
            condition_pc,
            condition: condition.clone(),
            exit_condition: negate_condition(&condition),
            induction_var: None,
            body_operations: Vec::new(),
            is_bounded: false,
            modified_storage: Vec::new(),
            modified_memory: Vec::new(),
        }
    }

    /// Generate Solidity loop construct
    pub fn to_solidity(&self) -> String {
        match &self.induction_var {
            Some(iv) if self.is_bounded => {
                // for-loop form
                let init = format!("uint256 {} = {}", iv.name, iv.init);
                let cond = self.condition.replace(&iv.name, &iv.name);
                let step = match iv.direction {
                    InductionDirection::Ascending => format!("{}++", iv.name),
                    InductionDirection::Descending => format!("{}--", iv.name),
                    InductionDirection::Unknown => format!("{} {}", iv.name, iv.step),
                };
                format!("for ({init}; {cond}; {step}) {{")
            }
            _ => {
                // while-loop form
                format!("while ({}) {{", self.condition)
            }
        }
    }
}

/// Negate a boolean condition for loop exit
fn negate_condition(condition: &str) -> String {
    let trimmed = condition.trim();

    // Handle already-negated conditions
    if trimmed.starts_with('!') && !trimmed.starts_with("!=") {
        // Remove the negation
        return trimmed[1..].trim_start_matches('(').trim_end_matches(')').to_string();
    }

    // Handle comparison operators
    if trimmed.contains(">=") {
        return trimmed.replace(">=", "<");
    }
    if trimmed.contains("<=") {
        return trimmed.replace("<=", ">");
    }
    if trimmed.contains("==") {
        return trimmed.replace("==", "!=");
    }
    if trimmed.contains("!=") {
        return trimmed.replace("!=", "==");
    }
    if trimmed.contains(" > ") {
        return trimmed.replace(" > ", " <= ");
    }
    if trimmed.contains(" < ") {
        return trimmed.replace(" < ", " >= ");
    }

    // Default: wrap with negation
    format!("!({})", condition)
}
```

### 1.2 Extend VMTrace

**File:** `crates/vm/src/ext/exec/mod.rs`

```rust
// Add to imports
mod loop_analysis;
pub use loop_analysis::{LoopInfo, InductionVariable, InductionDirection};

// Modify VMTrace struct
#[derive(Clone, Debug, Default)]
pub struct VMTrace {
    pub instruction: u128,
    pub gas_used: u128,
    pub operations: Vec<State>,
    pub children: Vec<VMTrace>,

    /// Loops detected during symbolic execution
    pub detected_loops: Vec<LoopInfo>,
}
```

---

## Phase 2: Loop Detection During Symbolic Execution

### 2.1 Capture Loop Instead of Discarding

**File:** `crates/vm/src/ext/exec/mod.rs`

Replace the loop-termination logic with loop-capture logic:

```rust
impl VM {
    fn recursive_map(
        &mut self,
        branch_count: &mut u32,
        handled_jumps: &mut HashMap<JumpFrame, Vec<Stack>>,
        timeout_at: &Instant,
    ) -> Result<Option<VMTrace>> {
        let vm = self;

        let mut vm_trace = VMTrace {
            instruction: vm.instruction,
            gas_used: 0,
            operations: Vec::new(),
            children: Vec::new(),
            detected_loops: Vec::new(),  // Initialize loop storage
        };

        while vm.bytecode.len() >= vm.instruction as usize {
            if Instant::now() >= *timeout_at {
                return Ok(None);
            }

            let state = match vm.step() {
                Ok(state) => state,
                Err(e) => {
                    warn!("executing branch failed during step: {:?}", e);
                    return Ok(None);
                }
            };
            let last_instruction = state.last_instruction.clone();

            vm_trace.operations.push(state);
            vm_trace.gas_used = vm.gas_used;

            if last_instruction.opcode == 0x57 {
                let jump_condition: Option<String> =
                    last_instruction.input_operations.get(1).map(|op| op.solidify());
                let jump_taken =
                    last_instruction.inputs.get(1).map(|op| !op.is_zero()).unwrap_or(true);

                let jump_frame = JumpFrame::new(
                    last_instruction.instruction,
                    last_instruction.inputs[0],
                    vm.stack.size(),
                    jump_taken,
                );

                // Check for loop patterns and CAPTURE instead of discard
                let loop_detected = self.check_loop_heuristics(
                    &vm.stack,
                    &jump_frame,
                    &jump_condition,
                    handled_jumps,
                );

                if let Some(loop_info) = loop_detected {
                    // Capture the loop information
                    vm_trace.detected_loops.push(loop_info);

                    // Continue execution past the loop (take the exit branch)
                    // This simulates the loop completing
                    trace!("loop captured at PC {}, continuing on exit path",
                           last_instruction.instruction);

                    // Take the non-loop branch (exit condition)
                    if jump_taken {
                        // Jump was taken = loop continues, so we take the fall-through
                        vm.instruction = last_instruction.instruction + 1;
                    } else {
                        // Jump not taken = loop exit, continue normally
                    }
                    continue;
                }

                // ... rest of existing JUMPI handling for non-loop branches ...
            }

            if vm.exitcode != 255 || !vm.returndata.is_empty() {
                break;
            }
        }

        Ok(Some(vm_trace))
    }

    /// Check all loop heuristics and return LoopInfo if a loop is detected
    fn check_loop_heuristics(
        &self,
        stack: &Stack,
        jump_frame: &JumpFrame,
        jump_condition: &Option<String>,
        handled_jumps: &HashMap<JumpFrame, Vec<Stack>>,
    ) -> Option<LoopInfo> {
        // Quick checks first
        if stack_contains_too_many_items(stack) {
            return Some(self.build_loop_info(jump_frame, jump_condition, "stack overflow"));
        }

        if stack_contains_too_many_of_the_same_item(stack) {
            return Some(self.build_loop_info(jump_frame, jump_condition, "repeated items"));
        }

        if stack_item_source_depth_too_deep(stack) {
            return Some(self.build_loop_info(jump_frame, jump_condition, "depth overflow"));
        }

        if jump_stack_depth_less_than_max_stack_depth(jump_frame, handled_jumps) {
            return Some(self.build_loop_info(jump_frame, jump_condition, "stack depth pattern"));
        }

        // Historical stack analysis
        if let Some(historical_stacks) = handled_jumps.get(jump_frame) {
            if let Some(condition) = jump_condition {
                for hist_stack in historical_stacks {
                    if hist_stack == stack {
                        return Some(self.build_loop_info(jump_frame, jump_condition, "exact match"));
                    }

                    let diff = stack_diff(stack, hist_stack);
                    if diff.is_empty() {
                        return Some(self.build_loop_info(jump_frame, jump_condition, "empty diff"));
                    }

                    if jump_condition_appears_recursive(&diff, condition) {
                        return Some(self.build_loop_info_with_induction(
                            jump_frame, jump_condition, &diff
                        ));
                    }

                    if jump_condition_contains_mutated_storage_access(&diff, condition) {
                        return Some(self.build_loop_info_with_storage(
                            jump_frame, jump_condition, &diff
                        ));
                    }

                    if jump_condition_contains_mutated_memory_access(&diff, condition) {
                        return Some(self.build_loop_info(jump_frame, jump_condition, "memory mutation"));
                    }
                }

                if stack_position_shows_pattern(stack, historical_stacks) {
                    return Some(self.build_loop_info_with_induction(
                        jump_frame,
                        jump_condition,
                        &stack_diff(stack, historical_stacks.last().unwrap_or(stack))
                    ));
                }

                if historical_diffs_approximately_equal(stack, historical_stacks) {
                    return Some(self.build_loop_info(jump_frame, jump_condition, "equal diffs"));
                }
            }
        }

        None
    }

    fn build_loop_info(
        &self,
        jump_frame: &JumpFrame,
        jump_condition: &Option<String>,
        _reason: &str,
    ) -> LoopInfo {
        LoopInfo::new(
            jump_frame.jumpdest.try_into().unwrap_or(0),
            jump_frame.pc,
            jump_condition.clone().unwrap_or_default(),
        )
    }

    fn build_loop_info_with_induction(
        &self,
        jump_frame: &JumpFrame,
        jump_condition: &Option<String>,
        stack_diff: &[StackFrame],
    ) -> LoopInfo {
        let mut info = self.build_loop_info(jump_frame, jump_condition, "induction");

        // Try to detect induction variable from stack diff
        info.induction_var = detect_induction_variable(stack_diff, jump_condition);
        info.is_bounded = info.induction_var.is_some();

        info
    }

    fn build_loop_info_with_storage(
        &self,
        jump_frame: &JumpFrame,
        jump_condition: &Option<String>,
        stack_diff: &[StackFrame],
    ) -> LoopInfo {
        let mut info = self.build_loop_info(jump_frame, jump_condition, "storage");

        // Extract modified storage slots
        info.modified_storage = extract_modified_storage(stack_diff);

        info
    }
}
```

### 2.2 Induction Variable Detection

**File:** `crates/vm/src/ext/exec/loop_analysis.rs` (add to existing)

```rust
use crate::core::stack::StackFrame;
use regex::Regex;
use lazy_static::lazy_static;

lazy_static! {
    // Match patterns like "var_a + 0x01" or "(something) + 1"
    static ref INCREMENT_PATTERN: Regex = Regex::new(
        r"^(.+?)\s*\+\s*(?:0x0*1|1)$"
    ).unwrap();

    // Match patterns like "var_a - 0x01" or "(something) - 1"
    static ref DECREMENT_PATTERN: Regex = Regex::new(
        r"^(.+?)\s*-\s*(?:0x0*1|1)$"
    ).unwrap();

    // Match comparison patterns: "var < bound" or "var >= bound"
    static ref COMPARISON_PATTERN: Regex = Regex::new(
        r"^(!)?(.+?)\s*(<|>|<=|>=|==|!=)\s*(.+)$"
    ).unwrap();
}

/// Attempt to detect an induction variable from the stack diff
pub fn detect_induction_variable(
    stack_diff: &[StackFrame],
    jump_condition: &Option<String>,
) -> Option<InductionVariable> {
    // Look for increment/decrement patterns in the stack diff
    for frame in stack_diff {
        let solidified = frame.operation.solidify();

        // Check for increment pattern
        if let Some(caps) = INCREMENT_PATTERN.captures(&solidified) {
            let var_name = caps.get(1)?.as_str().to_string();
            let bound = extract_bound_from_condition(jump_condition, &var_name);

            return Some(InductionVariable {
                name: simplify_var_name(&var_name),
                init: "0".to_string(),
                step: "+ 1".to_string(),
                bound,
                direction: InductionDirection::Ascending,
            });
        }

        // Check for decrement pattern
        if let Some(caps) = DECREMENT_PATTERN.captures(&solidified) {
            let var_name = caps.get(1)?.as_str().to_string();
            let bound = extract_bound_from_condition(jump_condition, &var_name);

            return Some(InductionVariable {
                name: simplify_var_name(&var_name),
                init: bound.clone().unwrap_or_else(|| "?".to_string()),
                step: "- 1".to_string(),
                bound: Some("0".to_string()),
                direction: InductionDirection::Descending,
            });
        }
    }

    None
}

/// Extract the loop bound from a condition like "i < loops"
fn extract_bound_from_condition(
    condition: &Option<String>,
    var_name: &str,
) -> Option<String> {
    let cond = condition.as_ref()?;

    if let Some(caps) = COMPARISON_PATTERN.captures(cond) {
        let lhs = caps.get(2)?.as_str().trim();
        let rhs = caps.get(4)?.as_str().trim();

        // Check if var_name appears on either side
        if lhs.contains(var_name) || similar_var(lhs, var_name) {
            return Some(rhs.to_string());
        }
        if rhs.contains(var_name) || similar_var(rhs, var_name) {
            return Some(lhs.to_string());
        }
    }

    None
}

/// Check if two variable references might be the same
fn similar_var(a: &str, b: &str) -> bool {
    // Handle cases where one is a simplified form of the other
    let a_simple = simplify_var_name(a);
    let b_simple = simplify_var_name(b);
    a_simple == b_simple
}

/// Simplify variable names for comparison
fn simplify_var_name(name: &str) -> String {
    // Remove common wrapper patterns
    name.trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim()
        .to_string()
}

/// Extract storage slots that are modified in the loop
pub fn extract_modified_storage(stack_diff: &[StackFrame]) -> Vec<U256> {
    let mut slots = Vec::new();

    for frame in stack_diff {
        let solidified = frame.operation.solidify();

        // Look for storage[X] patterns
        if solidified.contains("storage[") {
            // Extract the slot - this is simplified, real impl needs proper parsing
            if let Some(start) = solidified.find("storage[") {
                if let Some(end) = solidified[start..].find(']') {
                    let slot_str = &solidified[start + 8..start + end];
                    if let Ok(slot) = U256::from_str_radix(slot_str.trim_start_matches("0x"), 16) {
                        slots.push(slot);
                    }
                }
            }
        }
    }

    slots
}
```

---

## Phase 3: Loop-Aware Analysis

### 3.1 New Loop Heuristic

**File:** `crates/decompile/src/utils/heuristics/loops.rs` (new file)

```rust
use futures::future::BoxFuture;
use heimdall_vm::core::vm::State;
use heimdall_vm::ext::exec::LoopInfo;

use crate::{
    core::analyze::AnalyzerState,
    interfaces::AnalyzedFunction,
    Error,
};

/// Analyzer state extension for loop tracking
#[derive(Debug, Clone, Default)]
pub struct LoopAnalyzerState {
    /// Stack of active loops (for nested loops)
    pub active_loops: Vec<LoopInfo>,

    /// Set of PCs that are loop headers
    pub loop_headers: std::collections::HashSet<u128>,

    /// Set of PCs that are loop exit points
    pub loop_exits: std::collections::HashSet<u128>,

    /// Current nesting depth
    pub depth: usize,
}

pub(crate) fn loop_heuristic<'a>(
    function: &'a mut AnalyzedFunction,
    state: &'a State,
    analyzer_state: &'a mut AnalyzerState,
    detected_loops: &'a [LoopInfo],
) -> BoxFuture<'a, Result<(), Error>> {
    Box::pin(async move {
        let instruction = &state.last_instruction;
        let current_pc = instruction.instruction;

        // Check if we're at a loop header
        for loop_info in detected_loops {
            if current_pc == loop_info.header_pc {
                // Emit loop opening
                function.logic.push(loop_info.to_solidity());

                // Track that we're in a loop for closing brace
                analyzer_state.loop_state.active_loops.push(loop_info.clone());
                analyzer_state.loop_state.depth += 1;

                return Ok(());
            }

            // Check if we're at the loop's JUMPI (end of iteration)
            if current_pc == loop_info.condition_pc {
                // Don't emit the if-statement for this JUMPI - it's the loop condition
                analyzer_state.skip_next_jumpi = true;

                // If this is the loop we're currently in, close it
                if analyzer_state.loop_state.active_loops.last()
                    .map(|l| l.condition_pc == current_pc)
                    .unwrap_or(false)
                {
                    function.logic.push("}".to_string());
                    analyzer_state.loop_state.active_loops.pop();
                    analyzer_state.loop_state.depth -= 1;
                }

                return Ok(());
            }
        }

        Ok(())
    })
}

/// Check if an operation is part of loop overhead (should be suppressed)
pub fn is_loop_overhead(state: &State, loop_info: &LoopInfo) -> bool {
    let instruction = &state.last_instruction;

    // Suppress induction variable updates (they're in the for-loop header)
    if let Some(ref iv) = loop_info.induction_var {
        let solidified = instruction.input_operations
            .first()
            .map(|op| op.solidify())
            .unwrap_or_default();

        // Check if this is the increment/decrement of the induction var
        if solidified.contains(&iv.name) &&
           (solidified.contains("+ 1") || solidified.contains("- 1")) {
            return true;
        }
    }

    false
}

/// Filter out Solidity 0.8+ overflow check panic paths
pub fn is_overflow_panic(state: &State) -> bool {
    let instruction = &state.last_instruction;

    // Check for Panic(0x11) - arithmetic overflow
    if instruction.opcode == 0xfd {  // REVERT
        // Check if memory contains panic selector
        let offset: usize = instruction.inputs[0].try_into().unwrap_or(0);
        let size: usize = instruction.inputs[1].try_into().unwrap_or(0);

        if size >= 4 {
            // The panic selector is 0x4e487b71
            // and code 0x11 indicates arithmetic overflow
            // This should be filtered out for Solidity 0.8+ contracts
            return true;  // Simplified - real impl checks memory content
        }
    }

    false
}
```

### 3.2 Integrate Loop Heuristic into Analyzer

**File:** `crates/decompile/src/core/analyze.rs`

```rust
// Add to AnalyzerState
#[derive(Debug, Clone)]
pub(crate) struct AnalyzerState {
    pub jumped_conditional: Option<String>,
    pub conditional_stack: Vec<String>,
    pub analyzer_type: AnalyzerType,
    pub skip_resolving: bool,

    // NEW: Loop-related state
    pub loop_state: LoopAnalyzerState,
    pub skip_next_jumpi: bool,  // Flag to skip JUMPI that's a loop condition
}

// Modify analyze_inner to pass loop info
fn analyze_inner<'a>(
    &'a mut self,
    branch: &'a VMTrace,
    analyzer_state: &'a mut AnalyzerState,
) -> BoxFuture<'a, Result<(), Error>> {
    Box::pin(async move {
        analyzer_state.jumped_conditional = None;

        for operation in &branch.operations {
            // Run loop heuristic FIRST
            if !branch.detected_loops.is_empty() {
                loop_heuristic(
                    &mut self.function,
                    operation,
                    analyzer_state,
                    &branch.detected_loops
                ).await?;
            }

            // Skip overflow panic paths
            if is_overflow_panic(operation) {
                continue;
            }

            // Check if we should skip this JUMPI (it's a loop condition)
            if analyzer_state.skip_next_jumpi &&
               operation.last_instruction.opcode == 0x57 {
                analyzer_state.skip_next_jumpi = false;
                continue;
            }

            // Run other heuristics
            for heuristic in &self.heuristics {
                heuristic.run(&mut self.function, operation, analyzer_state).await?;
            }
        }

        // Recurse, propagating detected loops
        for child in &branch.children {
            self.analyze_inner(child, analyzer_state).await?;
        }

        // Handle closing braces...
        // ...existing logic...

        Ok(())
    })
}
```

### 3.3 Register Loop Heuristic

**File:** `crates/decompile/src/utils/heuristics/mod.rs`

```rust
mod loops;  // NEW

pub(crate) use loops::{loop_heuristic, LoopAnalyzerState, is_overflow_panic};

// Update Heuristic registration in Analyzer
impl Analyzer {
    pub(crate) fn register_heuristics(&mut self) -> Result<(), Error> {
        match self.typ {
            AnalyzerType::Solidity => {
                // Loop heuristic runs separately (needs detected_loops)
                self.heuristics.push(Heuristic::new(event_heuristic));
                self.heuristics.push(Heuristic::new(solidity_heuristic));
                self.heuristics.push(Heuristic::new(argument_heuristic));
                self.heuristics.push(Heuristic::new(modifier_heuristic));
                self.heuristics.push(Heuristic::new(extcall_heuristic));
            }
            // ... other types
        };
        Ok(())
    }
}
```

---

## Phase 4: CFG Back-Edge Detection

### 4.1 Dominator-Based Analysis

**File:** `crates/cfg/src/core/graph.rs`

```rust
use petgraph::algo::dominators::simple_fast;
use petgraph::visit::EdgeRef;

/// Represents a back-edge in the CFG (indicates a loop)
#[derive(Debug, Clone)]
pub struct BackEdge {
    /// Source node (end of loop body)
    pub source: NodeIndex<u32>,
    /// Target node (loop header)
    pub target: NodeIndex<u32>,
    /// The condition expression on this edge
    pub condition: Option<String>,
}

/// Represents a natural loop in the CFG
#[derive(Debug, Clone)]
pub struct NaturalLoop {
    /// The loop header node
    pub header: NodeIndex<u32>,
    /// All nodes in the loop body
    pub body: Vec<NodeIndex<u32>>,
    /// The back-edge that forms this loop
    pub back_edge: BackEdge,
    /// Exit edges from the loop
    pub exit_edges: Vec<(NodeIndex<u32>, NodeIndex<u32>)>,
}

/// Detect all back-edges in the CFG using dominator analysis
pub fn detect_back_edges(graph: &Graph<String, String>) -> Vec<BackEdge> {
    if graph.node_count() == 0 {
        return Vec::new();
    }

    let root = NodeIndex::new(0);
    let dominators = simple_fast(graph, root);
    let mut back_edges = Vec::new();

    for edge in graph.edge_references() {
        let source = edge.source();
        let target = edge.target();

        // A back-edge exists when target dominates source
        // (i.e., we can only reach source by going through target)
        let target_dominates_source = dominators
            .dominators(source)
            .map(|mut doms| doms.any(|dom| dom == target))
            .unwrap_or(false);

        if target_dominates_source {
            back_edges.push(BackEdge {
                source,
                target,
                condition: Some(edge.weight().clone()),
            });
        }
    }

    back_edges
}

/// Find all natural loops in the CFG
pub fn find_natural_loops(graph: &Graph<String, String>) -> Vec<NaturalLoop> {
    let back_edges = detect_back_edges(graph);
    let mut loops = Vec::new();

    for back_edge in back_edges {
        // Find all nodes in the loop body
        let body = find_loop_body(graph, back_edge.target, back_edge.source);

        // Find exit edges (edges leaving the loop)
        let exit_edges = find_exit_edges(graph, &body);

        loops.push(NaturalLoop {
            header: back_edge.target,
            body,
            back_edge,
            exit_edges,
        });
    }

    loops
}

/// Find all nodes in a loop body given the header and back-edge source
fn find_loop_body(
    graph: &Graph<String, String>,
    header: NodeIndex<u32>,
    back_edge_source: NodeIndex<u32>,
) -> Vec<NodeIndex<u32>> {
    let mut body = vec![header];
    let mut stack = vec![back_edge_source];
    let mut visited = std::collections::HashSet::new();
    visited.insert(header);

    while let Some(node) = stack.pop() {
        if visited.insert(node) {
            body.push(node);

            // Add all predecessors (nodes with edges TO this node)
            for edge in graph.edges_directed(node, petgraph::Direction::Incoming) {
                let pred = edge.source();
                if !visited.contains(&pred) {
                    stack.push(pred);
                }
            }
        }
    }

    body
}

/// Find edges that exit the loop
fn find_exit_edges(
    graph: &Graph<String, String>,
    loop_body: &[NodeIndex<u32>],
) -> Vec<(NodeIndex<u32>, NodeIndex<u32>)> {
    let body_set: std::collections::HashSet<_> = loop_body.iter().copied().collect();
    let mut exits = Vec::new();

    for &node in loop_body {
        for edge in graph.edges(node) {
            let target = edge.target();
            if !body_set.contains(&target) {
                exits.push((node, target));
            }
        }
    }

    exits
}
```

---

## Phase 5: Postprocessor Cleanup

### 5.1 Loop Variable Naming

**File:** `crates/decompile/src/utils/postprocessors/loops.rs` (new file)

```rust
use crate::interfaces::AnalyzedFunction;
use regex::Regex;
use lazy_static::lazy_static;

lazy_static! {
    // Match loop counter variable patterns
    static ref LOOP_VAR_PATTERN: Regex = Regex::new(
        r"\bvar_([a-f0-9]+)\b"
    ).unwrap();
}

/// Postprocess loop constructs for cleaner output
pub fn loop_postprocessor(function: &mut AnalyzedFunction) -> Result<(), crate::Error> {
    let mut loop_counter = 0;

    for line in &mut function.logic {
        // Rename loop variables to i, j, k, etc.
        if line.starts_with("for (") || line.starts_with("while (") {
            let var_name = match loop_counter {
                0 => "i",
                1 => "j",
                2 => "k",
                3 => "l",
                _ => continue,
            };

            // Replace the first variable in the for-loop with a clean name
            if let Some(caps) = LOOP_VAR_PATTERN.captures(line) {
                let old_var = caps.get(0).unwrap().as_str();
                *line = line.replace(old_var, var_name);
            }

            loop_counter += 1;
        }
    }

    Ok(())
}

/// Remove redundant overflow checks from loop bodies
pub fn remove_overflow_checks(function: &mut AnalyzedFunction) -> Result<(), crate::Error> {
    function.logic.retain(|line| {
        // Remove lines that are just overflow panic setup
        !line.contains("0x4e487b71") &&
        !line.contains("Panic(") &&
        // Remove tautological requires from loop bounds checking
        !is_tautological_require(line)
    });

    Ok(())
}

/// Check if a require statement is always true (tautological)
fn is_tautological_require(line: &str) -> bool {
    if !line.starts_with("require(") {
        return false;
    }

    // Pattern: require(x == x)
    if line.contains("==") {
        let parts: Vec<&str> = line.split("==").collect();
        if parts.len() == 2 {
            let lhs = parts[0].trim_start_matches("require(").trim();
            let rhs = parts[1].trim().trim_end_matches(");").trim();
            if lhs == rhs {
                return true;
            }
        }
    }

    // Pattern: require(!0 < x) which is always true for unsigned
    if line.contains("!0 <") || line.contains("!0x0 <") {
        return true;
    }

    false
}
```

---

## Testing

### Test Case: SimpleLoop

```rust
#[test]
async fn test_simple_loop_decompilation() {
    let bytecode = "..."; // SimpleLoop bytecode

    let result = decompile(DecompilerArgsBuilder::new()
        .target(bytecode)
        .include_solidity(true)
        .build()
        .unwrap()
    ).await.unwrap();

    let source = result.source.unwrap();

    // Should contain a for-loop
    assert!(source.contains("for (") || source.contains("while ("));

    // Should NOT contain tautological requires
    assert!(!source.contains("require(arg0 == arg0)"));

    // Should NOT contain panic code
    assert!(!source.contains("0x4e487b71"));

    // Should NOT be marked as view (modifies storage)
    assert!(!source.contains("public view"));

    // Should contain storage modification
    assert!(source.contains("number") || source.contains("storage[0]"));
}
```

---

## Migration Path

1. **Phase 1-2**: Implement LoopInfo and capture during symbolic execution
   - This is the foundation - loops are captured instead of discarded
   - Backward compatible: existing code still works, just gets more info

2. **Phase 3**: Implement loop heuristic
   - Uses captured LoopInfo to emit proper loop constructs
   - Filters overflow panics

3. **Phase 4**: CFG back-edge detection
   - Provides alternative loop detection path
   - Can validate/improve loop detection from symbolic execution

4. **Phase 5**: Postprocessing cleanup
   - Polish output with clean variable names
   - Remove remaining artifacts

## Complexity Estimate

| Phase | Files Changed | New Files | Complexity |
|-------|--------------|-----------|------------|
| 1 | 2 | 1 | Medium |
| 2 | 1 | 0 | High |
| 3 | 3 | 1 | Medium |
| 4 | 1 | 0 | Medium |
| 5 | 2 | 1 | Low |

**Total: ~8 files changed, 3 new files**
