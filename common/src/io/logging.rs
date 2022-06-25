use colored::*;

use super::super::utils::replace_last;


pub struct Logger {
    pub level: u8,
    pub trace: TraceFactory,
}


#[derive(Clone, Debug)]
pub struct TraceFactory {
    pub traces: Vec<Trace>
}


#[derive(Clone, Debug)]
pub enum TraceCategory {
    Log,
    LogUnknown,
    Message,
    Call,
    Create
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
    pub fn new() -> TraceFactory {
        TraceFactory {
            traces: Vec::new()
        }
    }

    // adds a new trace to the factory
    pub fn add_trace(&mut self, category: &str, parent_index: u32, instruction: u32, message: Vec<String>) -> u32{
        
        // build the new trace
        let trace = Trace::new(
            category,
            parent_index,
            instruction,
            message,
        );
        let trace_index = self.traces.len() as u32 + 1;

        // add the children indices to the parent
        if trace.parent != 0 {

            // get the parent
            let parent = self.traces.get_mut(trace.parent as usize - 1).unwrap();
            
            // add the child index to the parent
            parent.children.push(trace_index as u32);
        }

        self.traces.push(trace);
        
        // return the index of the new trace
        trace_index
    }

    
    // pretty print the trace
    pub fn display(self) {
        for index in 0..self.traces.len() {
            
            // safe to unwrap because we just iterated over the traces
            let trace = self.traces.get(index).unwrap();

            // match only root traces and print them
            match trace.parent {
                0 => { self.print_trace("", index); },
                _ => {}
            }
        }
    }


    // recursive function which prints traces
    pub fn print_trace(&self, prefix: &str, index: usize) {
        let trace: &Trace = match self.traces.get(index) {
            Some(trace) => trace,
            None => return
        };

        // each category has slightly different formatting
        match trace.category {
            TraceCategory::Call => {
                
                // print the trace title
                println!(
                    "{}[{}] call {}",
                    replace_last(prefix.to_string(), "│ ", " ├─"),
                    trace.instruction,
                    trace.message.get(0).unwrap()
                );

                // print the children
                for child in &trace.children {
                    self.print_trace(
                        &format!("{}  │", prefix),
                        *child as usize - 1
                    );
                }

                // print the return value
                println!(
                    "{}  └─ ← {}",
                    prefix,
                    match trace.message.get(1) {
                        Some(message) => message,
                        None => "()"
                    }
                )

            },
            TraceCategory::Log => {
                println!(
                    "{} emit {}",
                    replace_last(prefix.to_string(), "│ ", " ├─"),
                    trace.message.get(0).unwrap()
                    
                );
            },
            TraceCategory::LogUnknown => {
                for message_index in 0..trace.message.len() {
                    let message = trace.message.get(message_index).unwrap();
                    println!(
                        "{} emit topic 0 {}",
                        if message_index == 0 {
                            replace_last(prefix.to_string(), "│ ", " ├─")
                        } else {
                            replace_last(prefix.to_string(), "│ ", " │ ")
                        },
                        message
                    );
                }
            },
            TraceCategory::Message => {
                for message_index in 0..trace.message.len() {
                    let message = trace.message.get(message_index).unwrap();
                    println!(
                        "{} {}",
                        if message_index == 0 {
                            replace_last(prefix.to_string(), "│ ", " ├─")
                        } else {
                            replace_last(prefix.to_string(), "│ ", " │ ")
                        },
                        message
                    );
                }
            },
            TraceCategory::Create => {
                println!(
                    "{}[{}] create → {}@{}",
                    replace_last(prefix.to_string(), "│ ", " ├─"),
                    trace.instruction,
                    trace.message.get(0).unwrap(),
                    trace.message.get(1).unwrap()
                );

                // print the children
                for child in &trace.children {
                    self.print_trace(
                        &format!("{}  │", prefix),
                        *child as usize - 1
                    );
                }

                // print the return value
                println!( "{}  └─ ← {} bytes", prefix, trace.message.get(2).unwrap())
            }
        }

    }

}

impl Trace {
    
    // create a new trace
    pub fn new(category: &str, parent_index: u32, instruction: u32, message: Vec<String>) -> Trace {
        Trace {
            category: match category {
                "log" => TraceCategory::Log,
                "log_unknown" => TraceCategory::LogUnknown,
                "info" => TraceCategory::Message,
                "call" => TraceCategory::Call,
                "create" => TraceCategory::Create,
                _ => TraceCategory::Message
            },
            instruction,
            message,
            parent: parent_index,
            children: Vec::new()
        }
    }

}

impl Logger {

    // create a new logger
    pub fn new(verbosity: &str) -> Logger {

        match verbosity {
            "ERROR" => Logger { level: 0, trace: TraceFactory::new() },
            "WARN" => Logger { level: 1, trace: TraceFactory::new() },
            "INFO" => Logger { level: 2, trace: TraceFactory::new() },
            "DEBUG" => Logger { level: 3,trace: TraceFactory::new() },
            "TRACE" => Logger {level: 4, trace: TraceFactory::new() },
            _  => Logger { level: 2, trace: TraceFactory::new() },
        }
    }
    

    pub fn error(&self, message: &str) {
        println!("{} {}", "error:".bright_red().bold(), message);
    }
    

    pub fn success(&self, message: &str) {
        println!("{} {}", "success:".bright_green().bold(), message);
    }


    pub fn warn (&self, message: &str) {
        if self.level >= 1 {
            println!("{} {}", "warn:".bright_yellow().bold(), message);
        }
    }


    pub fn info (&self, message: &str) {
        if self.level >= 2 {
            println!("{} {}", "info:".bright_cyan().bold(), message);
        }
    }


    pub fn debug (&self, message: &str) {
        if self.level >= 3 {
            println!("{} {}", "debug:".bright_magenta().bold(), message);
        }
    }

    // trace variables and functions available through
    // self.trace.*
}