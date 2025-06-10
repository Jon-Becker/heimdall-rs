use colored::*;

use super::super::strings::replace_last;

/// The trace factory is used to build a trace of the program's execution.
/// Has several helper functions to add different types of traces.
#[derive(Clone, Debug)]
pub struct TraceFactory {
    /// The level of the trace. Higher numbers mean more verbose output.
    pub level: i8,
    /// The collection of traces gathered during execution.
    pub traces: Vec<Trace>,
}

/// The trace category is used to determine how the trace is formatted.
#[derive(Clone, Debug)]
pub enum TraceCategory {
    /// Standard log message.
    Log,
    /// Log message with unknown source.
    LogUnknown,
    /// General message.
    Message,
    /// Function call trace.
    Call,
    /// Contract creation trace.
    Create,
    /// Empty trace (placeholder).
    Empty,
    /// Contract self-destruct trace.
    Suicide,
}

/// Individual trace, which is added to the trace factory.
#[derive(Clone, Debug)]
pub struct Trace {
    /// The category of the trace, determining its formatting and interpretation.
    pub category: TraceCategory,
    /// The instruction number or identifier for this trace.
    pub instruction: u32,
    /// The message content of the trace, potentially multiple lines.
    pub message: Vec<String>,
    /// The parent trace identifier (if this is a child trace).
    pub parent: u32,
    /// Child trace identifiers that are nested under this trace.
    pub children: Vec<u32>,
}

impl TraceFactory {
    /// creates a new empty trace factory
    pub fn new(level: i8) -> TraceFactory {
        TraceFactory { level, traces: Vec::new() }
    }

    /// adds a new trace to the factory
    pub fn add(
        &mut self,
        category: &str,
        parent_index: u32,
        instruction: u32,
        message: Vec<String>,
    ) -> u32 {
        // build the new trace
        let trace = Trace::new(category, parent_index, instruction, message);
        let trace_index = self.traces.len() as u32 + 1;

        // add the children indices to the parent
        if trace.parent != 0 {
            // get the parent
            let parent =
                self.traces.get_mut(trace.parent as usize - 1).expect("Failed to build trace.");

            // add the child index to the parent
            parent.children.push(trace_index);
        }

        self.traces.push(trace);

        // return the index of the new trace
        trace_index
    }

    /// display the trace to the console if the verbosity is high enough
    pub fn display(&self) {
        for index in 0..self.traces.len() {
            // safe to unwrap because we just iterated over the traces
            let trace = self.traces.get(index).expect("Failed to build trace.");

            // match only root traces and print them
            if trace.parent == 0 {
                self.print_trace(" ", index);
            }
        }
    }

    /// recursive function used to print traces to the console correctly
    pub fn print_trace(&self, prefix: &str, index: usize) {
        let trace: &Trace = match self.traces.get(index) {
            Some(trace) => trace,
            None => return,
        };

        // each category has slightly different formatting
        match trace.category {
            TraceCategory::Call => {
                // print the trace title
                println!(
                    "{} {} {}",
                    replace_last(prefix, "│ ", " ├─").bold().bright_white(),
                    format!("[{}]", trace.instruction).bold().bright_white(),
                    trace.message.first().expect("Failed to build trace.")
                );

                // print the children
                for child in &trace.children {
                    self.print_trace(
                        &format!("{prefix}   │").bold().bright_white(),
                        *child as usize - 1,
                    );
                }

                // print the return value
                println!(
                    "{} ← {}",
                    format!("{prefix}   └─").bold().bright_white(),
                    match trace.message.get(1) {
                        Some(message) => format!(
                            "{}",
                            if message == "()" { message.dimmed() } else { message.green() }
                        ),
                        None => "()".dimmed().to_string(),
                    }
                )
            }
            TraceCategory::Log => {
                println!(
                    "{} emit {} {}",
                    replace_last(prefix, "│ ", " ├─").bold().bright_white(),
                    trace.message.first().expect("Failed to build trace."),
                    format!("[log index: {}]", trace.instruction).dimmed(),
                );
            }
            TraceCategory::LogUnknown => {
                let log_size = trace.message.len();
                if log_size > 1 {
                    for message_index in 0..trace.message.len() - 1 {
                        let message =
                            trace.message.get(message_index).expect("Failed to build trace.");
                        println!(
                            "{}      {}: {}",
                            replace_last(prefix, "│ ", " │ ").bold().bright_white(),
                            format!("topic {message_index}").purple(),
                            message
                        );
                    }
                    println!(
                        "{}         {}: {}",
                        replace_last(prefix, "│ ", " │ ").bold().bright_white(),
                        "data".purple(),
                        trace.message.last().expect("Failed to build trace.")
                    );
                } else {
                    println!(
                        "{} emit {}: {}",
                        replace_last(prefix, "│ ", " ├─").bold().bright_white(),
                        "data".purple(),
                        trace.message.last().expect("Failed to build trace.")
                    );
                }
            }
            TraceCategory::Message => {
                for message_index in 0..trace.message.len() {
                    let message = trace.message.get(message_index).expect("Failed to build trace.");
                    println!(
                        "{} {}",
                        if prefix.ends_with("└─") {
                            prefix.to_string().bold().bright_white()
                        } else if message_index == 0 {
                            replace_last(prefix, "│ ", " ├─").bold().bright_white()
                        } else {
                            replace_last(prefix, "│ ", " │ ").bold().bright_white()
                        },
                        message
                    );
                }

                // print the children
                for (i, child) in trace.children.iter().enumerate() {
                    if i == trace.children.len() - 1 {
                        self.print_trace(
                            &format!("{prefix}   └─").bold().bright_white(),
                            *child as usize - 1,
                        );
                    } else {
                        self.print_trace(
                            &format!("{prefix}   │").bold().bright_white(),
                            *child as usize - 1,
                        );
                    }
                }
            }
            TraceCategory::Empty => {
                println!("{}", replace_last(prefix, "│ ", " │ ").bold().bright_white());
            }
            TraceCategory::Create => {
                println!(
                    "{} {} create → {}",
                    replace_last(prefix, "│ ", " ├─").bold().bright_white(),
                    format!("[{}]", trace.instruction).bold().bright_white(),
                    trace.message.first().expect("Failed to build trace.")
                );

                // print the children
                for child in &trace.children {
                    self.print_trace(
                        &format!("{prefix}   │").bold().bright_white(),
                        *child as usize - 1,
                    );
                }

                // print the return value
                println!(
                    "{} ← {}",
                    format!("{prefix}   └─").bold().bright_white(),
                    trace.message.get(1).expect("Failed to build trace.").bold().green()
                )
            }
            TraceCategory::Suicide => {
                println!(
                    "{} {} {} selfdestruct → {}",
                    replace_last(prefix, "│ ", " ├─").bold().bright_white(),
                    format!("[{}]", trace.instruction).bold().bright_white(),
                    trace.message.first().expect("Failed to build trace."),
                    trace.message.get(1).expect("Failed to build trace.")
                );
            }
        }
    }

    ////////////////////////////////////////////////////////////////////////////////
    //                                TRACE HELPERS                               //
    ////////////////////////////////////////////////////////////////////////////////

    /// adds a function call trace
    pub fn add_call(
        &mut self,
        parent_index: u32,
        instruction: u32,
        origin: String,
        function_name: String,
        args: Vec<String>,
        returns: String,
    ) -> u32 {
        let title = format!(
            "{}::{}({})",
            origin.bright_cyan(),
            function_name.bright_cyan(),
            args.join(", ")
        );
        self.add("call", parent_index, instruction, vec![title, returns])
    }

    /// Adds a function call trace with extra information.
    ///
    /// This method creates a trace entry for a function call and includes additional context
    /// information.
    ///
    /// # Arguments
    ///
    /// * `parent_index` - The index of the parent trace
    /// * `instruction` - The instruction identifier
    /// * `origin` - The origin context (e.g., contract name)
    /// * `function_name` - The name of the function being called
    /// * `args` - The arguments passed to the function
    /// * `returns` - The return value(s) of the function
    /// * `extra` - Additional context information to display
    ///
    /// # Returns
    ///
    /// * `u32` - The index of the newly added trace
    #[allow(clippy::too_many_arguments)]
    pub fn add_call_with_extra(
        &mut self,
        parent_index: u32,
        instruction: u32,
        origin: String,
        function_name: String,
        args: Vec<String>,
        returns: String,
        extra: Vec<String>,
    ) -> u32 {
        let title = format!(
            "{}::{}({}) {}",
            origin.bright_cyan(),
            function_name.bright_cyan(),
            args.join(", "),
            extra.iter().map(|s| format!("[{s}]")).collect::<Vec<String>>().join(" ").dimmed()
        );
        self.add("call", parent_index, instruction, vec![title, returns])
    }

    /// adds a contract creation trace
    pub fn add_creation(
        &mut self,
        parent_index: u32,
        instruction: u32,
        name: String,
        pointer: String,
        size: u128,
    ) -> u32 {
        let contract = format!("{}@{}", name.green(), pointer.green(),);
        self.add("create", parent_index, instruction, vec![contract, format!("{size} bytes")])
    }

    /// adds a suicide event
    pub fn add_suicide(
        &mut self,
        parent_index: u32,
        instruction: u32,
        address: String,
        refund_address: String,
        refund_amount: f64,
    ) -> u32 {
        self.add(
            "suicide",
            parent_index,
            instruction,
            vec![
                address,
                format!("{} {}", refund_address, format!("[{refund_amount} ether]").dimmed()),
            ],
        )
    }

    /// adds a known log trace
    pub fn add_emission(
        &mut self,
        parent_index: u32,
        instruction: u32,
        name: &str,
        args: &[String],
    ) -> u32 {
        let log = format!("{}({})", name.purple(), args.join(", "));
        self.add("log", parent_index, instruction, vec![log])
    }

    /// adds an unknown or raw log trace
    pub fn add_raw_emission(
        &mut self,
        parent_index: u32,
        instruction: u32,
        mut topics: Vec<String>,
        data: String,
    ) -> u32 {
        topics.push(data);
        self.add("log_unknown", parent_index, instruction, topics)
    }

    /// add info message to the trace
    pub fn add_info(&mut self, parent_index: u32, instruction: u32, message: &str) -> u32 {
        let message = format!("{} {}", "info:".bright_cyan().bold(), message);
        self.add("message", parent_index, instruction, vec![message])
    }

    /// add debug message to the trace
    pub fn add_debug(&mut self, parent_index: u32, instruction: u32, message: &str) -> u32 {
        let message = format!("{} {}", "debug:".bright_magenta().bold(), message);
        self.add("message", parent_index, instruction, vec![message])
    }

    /// add error message to the trace
    pub fn add_error(&mut self, parent_index: u32, instruction: u32, message: &str) -> u32 {
        let message = format!("{} {}", "error:".bright_red().bold(), message);
        self.add("message", parent_index, instruction, vec![message])
    }

    /// add warn message to the trace
    pub fn add_warn(&mut self, parent_index: u32, instruction: u32, message: &str) -> u32 {
        let message = format!("{} {}", "warn:".bright_yellow().bold(), message);
        self.add("message", parent_index, instruction, vec![message])
    }

    /// add a vector of messages to the trace
    pub fn add_message(
        &mut self,
        parent_index: u32,
        instruction: u32,
        message: Vec<String>,
    ) -> u32 {
        self.add("message", parent_index, instruction, message)
    }

    /// add a line break to the trace
    pub fn br(&mut self, parent_index: u32) -> u32 {
        self.add("empty", parent_index, 0, vec!["".to_string()])
    }
}

impl Trace {
    /// create a new raw trace with the given parameters
    pub fn new(category: &str, parent_index: u32, instruction: u32, message: Vec<String>) -> Trace {
        Trace {
            category: match category {
                "log" => TraceCategory::Log,
                "log_unknown" => TraceCategory::LogUnknown,
                "message" => TraceCategory::Message,
                "call" => TraceCategory::Call,
                "create" => TraceCategory::Create,
                "empty" => TraceCategory::Empty,
                "suicide" => TraceCategory::Suicide,
                _ => TraceCategory::Message,
            },
            instruction,
            message,
            parent: parent_index,
            children: Vec::new(),
        }
    }
}

impl Default for TraceFactory {
    fn default() -> Self {
        // get the environment variable RUST_LOG and parse it
        let level = match std::env::var("RUST_LOG") {
            Ok(level) => match level.to_lowercase().as_str() {
                "silent" => -1,
                "error" => 0,
                "warn" => 1,
                "info" => 2,
                "debug" => 3,
                "trace" => 4,
                "all" => 5,
                "max" => 6,
                _ => 1,
            },
            Err(_) => 2,
        };

        TraceFactory::new(level)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_raw_trace() {
        let mut trace = TraceFactory::new(4);

        let parent = trace.add("call", 0, 123123, vec!["Test::test_trace()".to_string()]);
        trace.add(
            "log",
            parent,
            234234,
            vec!["ContractCreated(contractAddress: 0x0000000000000000000000000000000000000000)"
                .to_string()],
        );
        let inner = trace.add(
            "create",
            parent,
            121234,
            vec![
                "TestContract".to_string(),
                "0x0000000000000000000000000000000000000000".to_string(),
                "917".to_string(),
            ],
        );
        trace.add(
            "log_unknown",
            inner,
            12344,
            vec!["0x0000000000000000000000000000000000000000000000000000000000000000".to_string()],
        );
        let deeper = trace.add(
            "call",
            inner,
            12344,
            vec![
                "Test::transfer(to: 0x0000000000000000000000000000000000000000, amount: 1)"
                    .to_string(),
                "true".to_string(),
            ],
        );
        trace.add("log", deeper, 12344, vec!["Transfer(from: 0x0000000000000000000000000000000000000000, to: 0x0000000000000000000000000000000000000000, amount: 1)".to_string()]);
        trace.add("message", inner, 12344, vec!["warn: Transfer to the zero address!".to_string()]);
        trace.add(
            "message",
            parent,
            12344,
            vec![
                "Execution Reverted: Out of Gas.".to_string(),
                "Execution Reverted: Out of Gas.".to_string(),
            ],
        );

        trace.display();
    }

    #[test]
    fn test_helper_functions() {
        let mut trace = TraceFactory::new(4);

        let parent = trace.add_call(
            0,
            123,
            "Test".to_string(),
            "test_trace".to_string(),
            vec!["arg1: 0x0".to_string(), "arg2: 0x1".to_string()],
            "()".to_string(),
        );
        trace.add_creation(
            parent,
            124,
            "TestContract".to_string(),
            "0x0000000000000000000000000000000000000000".to_string(),
            1232,
        );
        trace.add_emission(
            parent,
            125,
            "ContractCreated",
            &["contractAddress: 0x0000000000000000000000000000000000000000".to_string()],
        );
        trace.add_raw_emission(
            parent,
            125,
            vec![
                "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            ],
            "0x".to_string(),
        );
        trace.add_error(parent, 126, "Testing errors");
        trace.add_info(parent, 127, "Testing info");
        trace.add_message(
            parent,
            128,
            vec!["test multiple".to_string(), "lines".to_string(), "to tracing".to_string()],
        );

        trace.display();
    }
}
