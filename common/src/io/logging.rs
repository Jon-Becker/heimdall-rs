use indicatif::ProgressStyle;
use std::io::{stdin, stdout};

use colored::*;

use super::super::utils::strings::replace_last;

pub struct Logger {
    pub level: i8,
}

#[derive(Clone, Debug)]
pub struct TraceFactory {
    pub level: i8,
    pub traces: Vec<Trace>,
}

#[derive(Clone, Debug)]
pub enum TraceCategory {
    Log,
    LogUnknown,
    Message,
    Call,
    Create,
    Empty,
}

#[derive(Clone, Debug)]
pub struct Trace {
    pub category: TraceCategory,
    pub instruction: u32,
    pub message: Vec<String>,
    pub parent: u32,
    pub children: Vec<u32>,
}

impl TraceFactory {
    // creates a new empty trace factory
    pub fn new(level: i8) -> TraceFactory {
        TraceFactory { level: level, traces: Vec::new() }
    }

    // adds a new trace to the factory
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
            let parent = self.traces.get_mut(trace.parent as usize - 1).unwrap();

            // add the child index to the parent
            parent.children.push(trace_index);
        }

        self.traces.push(trace);

        // return the index of the new trace
        trace_index
    }

    // pretty print the trace
    pub fn display(&self) {
        if self.level >= 3 {
            println!("{}:", "trace".bright_blue().bold());
            for index in 0..self.traces.len() {
                // safe to unwrap because we just iterated over the traces
                let trace = self.traces.get(index).unwrap();

                // match only root traces and print them
                if trace.parent == 0 {
                    self.print_trace(" ", index);
                }
            }
        }
    }

    // recursive function which prints traces
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
                    replace_last(prefix.to_string(), "│ ", " ├─").bold().bright_white(),
                    format!("[{}]", trace.instruction).bold().bright_white(),
                    trace.message.get(0).unwrap()
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
                    "{} emit {}",
                    replace_last(prefix.to_string(), "│ ", " ├─").bold().bright_white(),
                    trace.message.get(0).unwrap()
                );
            }
            TraceCategory::LogUnknown => {
                let log_size = trace.message.len();
                if log_size > 1 {
                    for message_index in 0..trace.message.len() - 1 {
                        let message = trace.message.get(message_index).unwrap();
                        println!(
                            "{} {} {}: {}",
                            if message_index == 0 {
                                replace_last(prefix.to_string(), "│ ", " ├─").bold().bright_white()
                            } else {
                                replace_last(prefix.to_string(), "│ ", " │ ").bold().bright_white()
                            },
                            if message_index == 0 { "emit" } else { "    " },
                            format!("topic {message_index}").purple(),
                            message
                        );
                    }
                    println!(
                        "{}         {}: {}",
                        replace_last(prefix.to_string(), "│ ", " │ ").bold().blue(),
                        "data".purple(),
                        trace.message.last().unwrap()
                    );
                } else {
                    println!(
                        "{} emit {}: {}",
                        replace_last(prefix.to_string(), "│ ", " ├─").bold().bright_white(),
                        "data".purple(),
                        trace.message.last().unwrap()
                    );
                }
            }
            TraceCategory::Message => {
                for message_index in 0..trace.message.len() {
                    let message = trace.message.get(message_index).unwrap();
                    println!(
                        "{} {}",
                        if prefix.ends_with("└─") {
                            prefix.to_string().bold().bright_white()
                        } else if message_index == 0 {
                            replace_last(prefix.to_string(), "│ ", " ├─").bold().bright_white()
                        } else {
                            replace_last(prefix.to_string(), "│ ", " │ ").bold().bright_white()
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
                println!("{}", replace_last(prefix.to_string(), "│ ", " │ ").bold().bright_white());
            }
            TraceCategory::Create => {
                println!(
                    "{} {} create → {}",
                    replace_last(prefix.to_string(), "│ ", " ├─").bold().bright_white(),
                    format!("[{}]", trace.instruction).bold().bright_white(),
                    trace.message.get(0).unwrap()
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
                    trace.message.get(1).unwrap().bold().green()
                )
            }
        }
    }

    ////////////////////////////////////////////////////////////////////////////////
    //                                TRACE HELPERS                               //
    ////////////////////////////////////////////////////////////////////////////////

    // adds a function call trace
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

    // adds a contract creation trace
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

    // adds a known log trace
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

    // adds an unknown or raw log trace
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

    // add info to the trace
    pub fn add_info(&mut self, parent_index: u32, instruction: u32, message: String) -> u32 {
        let message = format!("{} {}", "info:".bright_cyan().bold(), message);
        self.add("message", parent_index, instruction, vec![message])
    }

    // add debug to the trace
    pub fn add_debug(&mut self, parent_index: u32, instruction: u32, message: String) -> u32 {
        let message = format!("{} {}", "debug:".bright_magenta().bold(), message);
        self.add("message", parent_index, instruction, vec![message])
    }

    // add error to the trace
    pub fn add_error(&mut self, parent_index: u32, instruction: u32, message: String) -> u32 {
        let message = format!("{} {}", "error:".bright_red().bold(), message);
        self.add("message", parent_index, instruction, vec![message])
    }

    // add warn to the trace
    pub fn add_warn(&mut self, parent_index: u32, instruction: u32, message: String) -> u32 {
        let message = format!("{} {}", "warn:".bright_yellow().bold(), message);
        self.add("message", parent_index, instruction, vec![message])
    }

    // add a vector of strings to the trace
    pub fn add_message(
        &mut self,
        parent_index: u32,
        instruction: u32,
        message: Vec<String>,
    ) -> u32 {
        self.add("message", parent_index, instruction, message)
    }

    // add a line break
    pub fn br(&mut self, parent_index: u32) -> u32 {
        self.add("empty", parent_index, 0, vec!["".to_string()])
    }
}

impl Trace {
    // create a new trace
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

impl Logger {
    // create a new logger
    pub fn new(verbosity: &str) -> (Logger, TraceFactory) {
        match verbosity {
            "SILENT" => (Logger { level: -1 }, TraceFactory::new(-1)),
            "ERROR" => (Logger { level: 0 }, TraceFactory::new(0)),
            "WARN" => (Logger { level: 1 }, TraceFactory::new(1)),
            "INFO" => (Logger { level: 2 }, TraceFactory::new(2)),
            "DEBUG" => (Logger { level: 3 }, TraceFactory::new(3)),
            "TRACE" => (Logger { level: 4 }, TraceFactory::new(4)),
            _ => (Logger { level: 1 }, TraceFactory::new(1)),
        }
    }

    pub fn error(&self, message: &str) {
        println!("{}: {}", "error".bright_red().bold(), message);
    }

    pub fn fatal(&self, message: &str) {
        println!("{}: {}", "fatal".bright_white().on_bright_red().bold(), message);
    }

    pub fn success(&self, message: &str) {
        if self.level >= 0 {
            println!("{}: {}", "success".bright_green().bold(), message);
        }
    }

    pub fn info(&self, message: &str) {
        if self.level >= 1 {
            println!("{}: {}", "info".bright_cyan().bold(), message);
        }
    }

    pub fn warn(&self, message: &str) {
        println!("{}: {}", "warn".bright_yellow().bold(), message);
    }

    pub fn debug(&self, message: &str) {
        if self.level >= 2 {
            println!("{}: {}", "debug".bright_magenta().bold(), message);
        }
    }

    pub fn trace(&self, message: &str) {
        if self.level >= 4 {
            println!("{}: {}", "trace".bright_blue().bold(), message);
        }
    }

    pub fn info_spinner(&self) -> ProgressStyle {
        ProgressStyle::with_template(&format!(
            "{}: {}",
            "info".bright_cyan().bold(),
            "{spinner} {msg}"
        ))
        .unwrap()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
    }

    pub fn debug_spinner(&self) -> ProgressStyle {
        ProgressStyle::with_template(&format!(
            "{}: {}",
            "debug".bright_magenta().bold(),
            "{spinner} {msg}"
        ))
        .unwrap()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
    }

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
            return default.unwrap()
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
                "  {} {}: {}",
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
            "\n  Select an option {}: ",
            if let Some(..) = default {
                format!("(default: {})", default.unwrap())
            } else {
                "".to_string()
            }
        );
        let _ = std::io::Write::flush(&mut stdout());

        if skip {
            if let Some(..) = default {
                println!("{}", default.unwrap());
            } else {
                println!();
            }
            return default.unwrap()
        }

        // get input
        match stdin().read_line(&mut selection) {
            Ok(_) => {
                // check if default was selected
                if selection.trim() == "" {
                    if let Some(..) = default {
                        return default.unwrap()
                    } else {
                        self.error("invalid selection.");
                        return self.option(function, message, options, default, skip)
                    }
                }

                // check if the input is a valid option
                let selected_index = match selection.trim().parse::<u8>() {
                    Ok(i) => i,
                    Err(_) => {
                        self.error("invalid selection.");
                        return self.option(function, message, options, default, skip)
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
