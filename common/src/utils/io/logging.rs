use indicatif::ProgressStyle;
use std::io::{stdin, stdout};

use colored::*;

use crate::utils::time::pretty_timestamp;

use super::super::strings::replace_last;

/// A logger which can be used to log messages to the console
/// in a standardized format.
#[derive(Clone)]
pub struct Logger {
    pub level: i8,
}

/// The trace factory is used to build a trace of the program's execution.
/// Has several helper functions to add different types of traces.
#[derive(Clone, Debug)]
pub struct TraceFactory {
    pub level: i8,
    pub traces: Vec<Trace>,
}

/// The trace category is used to determine how the trace is formatted.
#[derive(Clone, Debug)]
pub enum TraceCategory {
    Log,
    LogUnknown,
    Message,
    Call,
    Create,
    Empty,
}

/// Individual trace, which is added to the trace factory.
#[derive(Clone, Debug)]
pub struct Trace {
    pub category: TraceCategory,
    pub instruction: u32,
    pub message: Vec<String>,
    pub parent: u32,
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
        if self.level >= 3 {
            println!("{}:", "trace".bright_blue().bold());
            for index in 0..self.traces.len() {
                // safe to unwrap because we just iterated over the traces
                let trace = self.traces.get(index).expect("Failed to build trace.");

                // match only root traces and print them
                if trace.parent == 0 {
                    self.print_trace(" ", index);
                }
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
                        replace_last(prefix, "│ ", " │ ").bold().blue(),
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
            extra.iter().map(|s| format!("[{}]", s)).collect::<Vec<String>>().join(" ").dimmed()
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

    /// adds a known log trace
    pub fn add_emission(
        &mut self,
        parent_index: u32,
        instruction: u32,
        name: String,
        args: Vec<String>,
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
                _ => TraceCategory::Message,
            },
            instruction,
            message,
            parent: parent_index,
            children: Vec::new(),
        }
    }
}

impl Default for Logger {
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

        Logger { level }
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

impl Logger {
    /// create a new logger with the given verbosity
    pub fn new(verbosity: &str) -> (Logger, TraceFactory) {
        match verbosity.to_uppercase().as_str() {
            "SILENT" => (Logger { level: -1 }, TraceFactory::new(-1)),
            "ERROR" => (Logger { level: 0 }, TraceFactory::new(0)),
            "WARN" => (Logger { level: 1 }, TraceFactory::new(1)),
            "INFO" => (Logger { level: 2 }, TraceFactory::new(2)),
            "DEBUG" => (Logger { level: 3 }, TraceFactory::new(3)),
            "TRACE" => (Logger { level: 4 }, TraceFactory::new(4)),
            "ALL" => (Logger { level: 5 }, TraceFactory::new(5)),
            "MAX" => (Logger { level: 6 }, TraceFactory::new(6)),
            _ => (Logger { level: 1 }, TraceFactory::new(1)),
        }
    }

    /// log an error message
    pub fn error(&self, message: &str) {
        if self.level >= 0 {
            println!(
                "{}  {}: {}",
                pretty_timestamp().dimmed(),
                "error".bright_red().bold(),
                message
            );
        }
    }

    /// log a fatal error, typically an unhanded exception which causes the program to exit
    pub fn fatal(&self, message: &str) {
        println!(
            "{}  {}: {}",
            pretty_timestamp().dimmed(),
            "fatal".bright_white().on_bright_red().bold(),
            message
        );
    }

    /// log a success message
    pub fn success(&self, message: &str) {
        if self.level >= 0 {
            println!(
                "{}  {}: {}",
                pretty_timestamp().dimmed(),
                "success".bright_green().bold(),
                message
            );
        }
    }

    /// log an info message
    pub fn info(&self, message: &str) {
        if self.level >= 1 {
            println!(
                "{}  {}: {}",
                pretty_timestamp().dimmed(),
                "info".bright_cyan().bold(),
                message
            );
        }
    }

    /// log a warning message
    pub fn warn(&self, message: &str) {
        if self.level >= 0 {
            println!(
                "{}  {}: {}",
                pretty_timestamp().dimmed(),
                "warn".bright_yellow().bold(),
                message
            );
        }
    }

    /// log a debug message
    pub fn debug(&self, message: &str) {
        if self.level >= 2 {
            println!(
                "{}  {}: {}",
                pretty_timestamp().dimmed(),
                "debug".bright_magenta().bold(),
                message
            );
        }
    }

    /// log a trace message
    pub fn trace(&self, message: &str) {
        if self.level >= 4 {
            println!(
                "{}  {}: {}",
                pretty_timestamp().dimmed(),
                "trace".bright_blue().bold(),
                message
            );
        }
    }

    /// log a max message
    pub fn debug_max(&self, message: &str) {
        if self.level >= 6 {
            println!(
                "{}  {}: {}",
                pretty_timestamp().dimmed(),
                "debug".bright_white().bold(),
                message.replace('\n', &("\n".to_owned() + &" ".repeat(31)))
            );
        }
    }

    /// get a formatted spinner for the given function
    pub fn info_spinner(&self) -> ProgressStyle {
        ProgressStyle::with_template(&format!(
            "{}  {}: {}",
            pretty_timestamp().dimmed(),
            "info".bright_cyan().bold(),
            "{spinner} {msg}"
        ))
        .expect("Failed to create spinner.")
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
    }

    /// get a formatted spinner for the given function
    pub fn debug_spinner(&self) -> ProgressStyle {
        ProgressStyle::with_template(&format!(
            "{}  {}: {}",
            pretty_timestamp().dimmed(),
            "debug".bright_magenta().bold(),
            "{spinner} {msg}"
        ))
        .expect("Failed to create spinner.")
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
    }

    /// prompt the user to select an option from the given list, or return the default
    pub fn option(
        &self,
        function: &str,
        message: &str,
        options: Vec<String>,
        default: Option<u8>,
        skip: bool,
    ) -> u8 {
        // if silent, return the default
        if self.level == -1 {
            return default.expect("Failed to get default option.");
        }

        // log the message with the given class
        match function {
            "error" => self.error(message),
            "success" => self.success(message),
            "warn" => self.warn(message),
            "debug" => self.debug(message),
            _ => self.info(message),
        }

        // print the option tree
        for (i, option) in options.iter().enumerate() {
            println!(
                "                                      {} {}: {}",
                if i == options.len() - 1 {
                    "└─".bold().bright_white()
                } else {
                    "├─".bold().bright_white()
                },
                i,
                option
            );
        }

        // flush output print prompt
        let mut selection = String::new();
        print!(
            "\n                                      Select an option {}: ",
            if default.is_some() {
                format!("(default: {})", default.expect("Failed to get default option."))
            } else {
                "".to_string()
            }
        );
        let _ = std::io::Write::flush(&mut stdout());

        if skip {
            if default.is_some() {
                println!("{}", default.expect("Failed to get default option."));
            } else {
                println!();
            }
            return default.expect("Failed to get default option.");
        }

        // get input
        match stdin().read_line(&mut selection) {
            Ok(_) => {
                // check if default was selected
                if selection.trim() == "" {
                    if let Some(default) = default {
                        return default;
                    } else {
                        self.error("invalid selection.");
                        return self.option(function, message, options, default, skip);
                    }
                }

                // check if the input is a valid option
                let selected_index = match selection.trim().parse::<u8>() {
                    Ok(i) => i,
                    Err(_) => {
                        self.error("invalid selection.");
                        return self.option(function, message, options, default, skip);
                    }
                };

                if options.get(selected_index as usize).is_some() {
                    selected_index
                } else {
                    self.error("invalid selection.");
                    self.option(function, message, options, default, skip)
                }
            }
            Err(_) => {
                self.error("invalid selection.");
                self.option(function, message, options, default, skip)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn test_raw_trace() {
        let start_time = Instant::now();
        let (logger, mut trace) = Logger::new("TRACE");

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
        logger.info(&format!("Tracing took {}", start_time.elapsed().as_secs_f64()));
    }

    #[test]
    fn test_helper_functions() {
        let start_time = Instant::now();
        let (logger, mut trace) = Logger::new("TRACE");

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
            "ContractCreated".to_string(),
            vec!["contractAddress: 0x0000000000000000000000000000000000000000".to_string()],
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
        logger.info(&format!("Tracing took {}", start_time.elapsed().as_secs_f64()));
    }

    #[test]
    fn test_option() {
        let (logger, _) = Logger::new("TRACE");

        logger.option(
            "warn",
            "multiple possibilities",
            vec!["option 1".to_string(), "option 2".to_string(), "option 3".to_string()],
            Some(0),
            true,
        );
    }

    #[test]
    fn test_warn() {
        let (logger, _) = Logger::new("SILENT");
        logger.warn("log");

        let (logger, _) = Logger::new("ERROR");
        logger.warn("log");

        let (logger, _) = Logger::new("WARN");
        logger.warn("log");

        let (logger, _) = Logger::new("INFO");
        logger.warn("log");

        let (logger, _) = Logger::new("DEBUG");
        logger.warn("log");

        let (logger, _) = Logger::new("TRACE");
        logger.warn("log");

        let (logger, _) = Logger::new("ALL");
        logger.warn("log");

        let (logger, _) = Logger::new("MAX");
        logger.warn("log");
    }

    #[test]
    fn test_error() {
        let (logger, _) = Logger::new("SILENT");
        logger.error("log");

        let (logger, _) = Logger::new("ERROR");
        logger.error("log");

        let (logger, _) = Logger::new("WARN");
        logger.error("log");

        let (logger, _) = Logger::new("INFO");
        logger.error("log");

        let (logger, _) = Logger::new("DEBUG");
        logger.error("log");

        let (logger, _) = Logger::new("TRACE");
        logger.error("log");

        let (logger, _) = Logger::new("ALL");
        logger.error("log");

        let (logger, _) = Logger::new("MAX");
        logger.error("log");
    }

    #[test]
    fn test_info() {
        let (logger, _) = Logger::new("SILENT");
        logger.info("log");

        let (logger, _) = Logger::new("ERROR");
        logger.info("log");

        let (logger, _) = Logger::new("WARN");
        logger.info("log");

        let (logger, _) = Logger::new("INFO");
        logger.info("log");

        let (logger, _) = Logger::new("DEBUG");
        logger.info("log");

        let (logger, _) = Logger::new("TRACE");
        logger.info("log");

        let (logger, _) = Logger::new("ALL");
        logger.info("log");

        let (logger, _) = Logger::new("MAX");
        logger.info("log");
    }

    #[test]
    fn test_success() {
        let (logger, _) = Logger::new("SILENT");
        logger.success("log");

        let (logger, _) = Logger::new("ERROR");
        logger.success("log");

        let (logger, _) = Logger::new("WARN");
        logger.success("log");

        let (logger, _) = Logger::new("INFO");
        logger.success("log");

        let (logger, _) = Logger::new("DEBUG");
        logger.success("log");

        let (logger, _) = Logger::new("TRACE");
        logger.success("log");

        let (logger, _) = Logger::new("ALL");
        logger.success("log");

        let (logger, _) = Logger::new("MAX");
        logger.success("log");
    }

    #[test]
    fn test_debug() {
        let (logger, _) = Logger::new("SILENT");
        logger.debug("log");

        let (logger, _) = Logger::new("ERROR");
        logger.debug("log");

        let (logger, _) = Logger::new("WARN");
        logger.debug("log");

        let (logger, _) = Logger::new("INFO");
        logger.debug("log");

        let (logger, _) = Logger::new("DEBUG");
        logger.debug("log");

        let (logger, _) = Logger::new("TRACE");
        logger.debug("log");

        let (logger, _) = Logger::new("ALL");
        logger.debug("log");

        let (logger, _) = Logger::new("MAX");
        logger.debug("log");
    }

    #[test]
    fn test_max() {
        let (_logger, _) = Logger::new("SILENT");
        use crate::debug_max;
        debug_max!("log");

        let (_logger, _) = Logger::new("ERROR");
        debug_max!("log");

        let (_logger, _) = Logger::new("WARN");
        debug_max!("log");

        let (_logger, _) = Logger::new("INFO");
        debug_max!("log");

        let (_logger, _) = Logger::new("DEBUG");
        debug_max!("log");

        let (_logger, _) = Logger::new("TRACE");
        debug_max!("log");

        let (_logger, _) = Logger::new("ALL");
        debug_max!("log");

        let (_logger, _) = Logger::new("MAX");
        debug_max!("log");
    }
}
