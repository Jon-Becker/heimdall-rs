use std::borrow::BorrowMut;

use colored::*;


pub struct Logger {
    pub level: u8,
    pub trace: TraceFactory,
}

#[derive(Clone, Debug)]
pub struct TraceFactory {
    pub traces: Vec<Trace>
}

#[derive(Clone, Debug)]
pub struct Trace {
    pub instruction: u32,
    pub message: Vec<String>,
    pub children: Vec<Trace>,
    pub trace_node: Vec<u32>
}

impl Trace {
    
    // create a new trace
    pub fn new(instruction: u32, message: Vec<String>, trace_node: Vec<u32>) -> Trace {
        // TODO CLASS LOGIC
        Trace {
            instruction,
            message,
            children: Vec::new(),
            trace_node: trace_node
        }
    }

    // adds a new trace as a child of this trace
    pub fn add_child_trace(&mut self, instruction: u32, message: Vec<String>) -> Trace {
        let mut parent_node = self.trace_node.clone();
        parent_node.push(self.children.len() as u32);

        let trace = Trace::new(instruction, message, parent_node);
        self.children.push(trace);

        // rebuild the parent trace and add it to the trace factory
        self.children.last().unwrap().clone()
    }
}

impl TraceFactory {

    // creates a new empty trace factory
    pub fn new() -> TraceFactory {
        TraceFactory {
            traces: Vec::new()
        }
    }

    // adds a new trace to the factory
    pub fn add_trace(&mut self, instruction: u32, message: Vec<String>) -> Trace {
        let mut parent_node = Vec::new();
        parent_node.push(self.traces.len() as u32);

        let trace = Trace::new(instruction, message, parent_node);
        self.traces.push(trace);
        self.traces.last().unwrap().clone()
    }

    // pretty print the trace
    pub fn print() {
        // TODO
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