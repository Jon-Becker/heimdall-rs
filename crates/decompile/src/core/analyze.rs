use std::{fmt::Display, time::Instant};

use heimdall_vm::ext::exec::VMTrace;
use tracing::debug;

use crate::{
    interfaces::AnalyzedFunction,
    utils::heuristics::{
        argument_heuristic, event_heuristic, extcall_heuristic, modifier_heuristic,
        solidity_heuristic, yul_heuristic, Heuristic,
    },
    Error,
};

/// The type of analyzer to use. This will determine which heuristics are used when analyzing a
/// [`VMTrace`] generated by symbolic execution.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AnalyzerType {
    /// Analyze the trace using Solidity heuristics, which will generate high-level Solidity code
    Solidity,
    /// Analyze the trace using Yul heuristics, which will generate verbose Yul code
    Yul,
    /// Analyze the trace using bare ABI heuristics, which will only generate ABI definitions
    Abi,
}

impl AnalyzerType {
    pub fn from_args(solidity: bool, yul: bool) -> Self {
        if solidity {
            return AnalyzerType::Solidity;
        }
        if yul {
            return AnalyzerType::Yul;
        }

        AnalyzerType::Abi
    }
}

impl Display for AnalyzerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalyzerType::Solidity => write!(f, "Solidity"),
            AnalyzerType::Yul => write!(f, "Yul"),
            AnalyzerType::Abi => write!(f, "ABI (bare)"),
        }
    }
}

/// State shared between heuristics
#[derive(Debug, Clone)]
pub(crate) struct AnalyzerState {
    /// If we reach a JUMPI, this will hold the conditional for scope tracking
    pub jumped_conditional: Option<String>,
    /// Tracks a stack of conditionals, used for scope tracking
    pub conditional_stack: Vec<String>,
    /// Tracks which analyzer type we are using
    pub analyzer_type: AnalyzerType,
}

/// The analyzer, which will analyze a [`VMTrace`] generated by symbolic execution and build an
/// [`AnalyzedFunction`] based on trace heuristics and opcode analysis.
///
/// Depending on [`AnalyzerType`], the analyzer will use different heuristics to analyze the trace.
pub struct Analyzer {
    /// The type of analyzer to use
    typ: AnalyzerType,
    /// The function to build during analysis
    function: AnalyzedFunction,
    /// A list of registered heuristics with the Heuristic Trait
    heuristics: Vec<Heuristic>,
}

impl Analyzer {
    /// Build a new analyzer with the given type, function, and trace
    pub fn new(typ: AnalyzerType, function: AnalyzedFunction) -> Self {
        Self { typ, function, heuristics: Vec::new() }
    }

    /// Register heuristics for the given function and trace
    pub fn register_heuristics(&mut self) -> Result<(), Error> {
        match self.typ {
            AnalyzerType::Solidity => {
                self.heuristics.push(Heuristic::new(event_heuristic));
                self.heuristics.push(Heuristic::new(solidity_heuristic));
                self.heuristics.push(Heuristic::new(argument_heuristic));
                self.heuristics.push(Heuristic::new(modifier_heuristic));
                self.heuristics.push(Heuristic::new(extcall_heuristic));
            }
            AnalyzerType::Yul => {
                self.heuristics.push(Heuristic::new(event_heuristic));
                self.heuristics.push(Heuristic::new(yul_heuristic));
                self.heuristics.push(Heuristic::new(argument_heuristic));
                self.heuristics.push(Heuristic::new(modifier_heuristic));
            }
            AnalyzerType::Abi => {
                self.heuristics.push(Heuristic::new(event_heuristic));
                self.heuristics.push(Heuristic::new(argument_heuristic));
                self.heuristics.push(Heuristic::new(modifier_heuristic));
            }
        };

        Ok(())
    }

    /// Performs analysis
    pub fn analyze(&mut self, trace_root: VMTrace) -> Result<AnalyzedFunction, Error> {
        debug!(
            "analzying symbolic execution trace for '{}' with the {} analyzer",
            self.function.selector, self.typ
        );
        self.function.analyzer_type = self.typ;
        let start_analysis_time = Instant::now();

        // Register heuristics
        self.register_heuristics()?;

        // get the analyzer state
        let mut analyzer_state = AnalyzerState {
            jumped_conditional: None,
            conditional_stack: Vec::new(),
            analyzer_type: self.typ,
        };

        // Perform analysis
        self.analyze_inner(&trace_root, &mut analyzer_state)?;

        debug!(
            "analysis for '{}' completed in {:?}",
            self.function.selector,
            start_analysis_time.elapsed()
        );

        Ok(self.function.clone())
    }

    /// Inner analysis implementation
    fn analyze_inner(
        &mut self,
        branch: &VMTrace,
        analyzer_state: &mut AnalyzerState,
    ) -> Result<(), Error> {
        // reset jumped conditional, we dont propagate conditionals across branches
        analyzer_state.jumped_conditional = None;

        // for each operation in the current trace branch, peform analysis with registerred
        // heuristics
        for operation in &branch.operations {
            for heuristic in &self.heuristics {
                heuristic.run(&mut self.function, operation, analyzer_state)?;
            }
        }

        // recurse into the children of the current trace branch
        for child in &branch.children {
            self.analyze_inner(child, analyzer_state)?;
        }

        // check if the ending brackets are needed
        if analyzer_state.jumped_conditional.is_some()
            && analyzer_state.conditional_stack.contains(
                analyzer_state
                    .jumped_conditional
                    .as_ref()
                    .expect("impossible case: should have short-circuited in previous conditional"),
            )
        {
            // remove the conditional
            for (i, conditional) in analyzer_state.conditional_stack.iter().enumerate() {
                if conditional
                    == analyzer_state.jumped_conditional.as_ref().expect(
                        "impossible case: should have short-circuited in previous conditional",
                    )
                {
                    analyzer_state.conditional_stack.remove(i);
                    break;
                }
            }

            self.function.logic.push("}".to_string());
        }

        Ok(())
    }
}
