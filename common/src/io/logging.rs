use std::io::{stdout, Write, stdin};

use colored::*;

use super::super::{
    utils::{
        strings::replace_last,
    },
};


pub struct Logger {
    pub level: u8,
}


#[derive(Clone, Debug)]
pub struct TraceFactory {
    pub level: u8,
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
    pub fn new(level: u8) -> TraceFactory {
        TraceFactory {
            level: level,
            traces: Vec::new()
        }
    }


    // adds a new trace to the factory
    pub fn add(&mut self, category: &str, parent_index: u32, instruction: u32, message: Vec<String>) -> u32{
        
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


    // updates a trace
    pub fn update(&mut self, index: usize, message: Vec<String>) {
        println!("{:#?}", self);
        let trace = self.traces.get_mut(index - 1).unwrap();
        trace.message = message;
    }
    
    // pretty print the trace
    pub fn display(&self) {
        if self.level >= 3 {
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
                let log_size = trace.message.len();
                if log_size > 1 {
                    for message_index in 0..trace.message.len()-1 {
                        let message = trace.message.get(message_index).unwrap();
                        println!(
                            "{} {} {}: {}",
                            if message_index == 0 { replace_last(prefix.to_string(), "│ ", " ├─") }
                            else { replace_last(prefix.to_string(), "│ ", " │ ") },
                            if message_index == 0 { "emit" } else { "    " },
                            format!("topic {}", message_index).purple(),
                            message
                        );
                    }
                    println!(
                        "{}         {}: {}",
                        replace_last(prefix.to_string(), "│ ", " │ "),
                        "data".purple(),
                        trace.message.last().unwrap()
                    );
                }
                else {
                    println!(
                        "{} emit {}: {}",
                        replace_last(prefix.to_string(), "│ ", " ├─"),
                        "data".purple(),
                        trace.message.last().unwrap()
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
                    "{}[{}] create → {}",
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
                println!( "{}  └─ ← {} bytes", prefix, trace.message.get(1).unwrap())
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
        size: u128
    ) -> u32 {
        let contract = format!(
            "{}@{}", 
            name.green(),
            pointer.green(),
        );
        self.add("create", parent_index, instruction, vec![contract, size.to_string()])
    }


    // adds a known log trace
    pub fn add_emission(
        &mut self,
        parent_index: u32,
        instruction: u32,
        name: String,
        args: Vec<String>
    ) -> u32 {
        let log = format!(
            "{}({})", 
            name.purple(),
            args.join(", ")
        );
        self.add("log", parent_index, instruction, vec![log])
    }


    // adds an unknown or raw log trace
    pub fn add_raw_emission(
        &mut self,
        parent_index: u32,
        instruction: u32,
        mut topics: Vec<String>,
        data: String
    ) -> u32 {
        topics.push(data);
        self.add("log_unknown", parent_index, instruction, topics)
    }


    // add info to the trace
    pub fn add_info(
        &mut self,
        parent_index: u32,
        instruction: u32,
        message: String
    ) -> u32 {
        let message = format!("{} {}", "info:".bright_cyan().bold(), message);
        self.add("message", parent_index, instruction, vec![message])
    }


    // add error to the trace
    pub fn add_error(
        &mut self,
        parent_index: u32,
        instruction: u32,
        message: String
    ) -> u32 {
        let message = format!("{} {}", "error:".bright_red().bold(), message);
        self.add("message", parent_index, instruction, vec![message])
    }


    // add warn to the trace
    pub fn add_warn(
        &mut self,
        parent_index: u32,
        instruction: u32,
        message: String
    ) -> u32 {
        let message = format!("{} {}", "warn:".bright_yellow().bold(), message);
        self.add("message", parent_index, instruction, vec![message])
    }


    // add a vector of strings to the trace
    pub fn add_message(
        &mut self,
        parent_index: u32,
        instruction: u32,
        message: Vec<String>
    ) -> u32 {
        self.add("message", parent_index, instruction, message)
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
    pub fn new(verbosity: &str) -> (Logger, TraceFactory) {

        match verbosity {
            "ERROR" => (Logger { level: 0, }, TraceFactory::new(0)),
            "WARN" => (Logger { level: 1, }, TraceFactory::new(1)),
            "INFO" => (Logger { level: 2, }, TraceFactory::new(2)),
            "DEBUG" => (Logger { level: 3, }, TraceFactory::new(3)),
            "TRACE" => (Logger { level: 4, }, TraceFactory::new(4)),
            _  => (Logger { level: 1, }, TraceFactory::new(1)),
        }
        
    }
    

    pub fn error(&self, message: &str) {
        println!("{} {}", "error:".bright_red().bold(), message);
    }
    

    pub fn success(&self, message: &str) {
        println!("{} {}", "success:".bright_green().bold(), message);
    }


    pub fn info (&self, message: &str) {
        if self.level >= 1 {
            println!("{} {}", "info:".bright_cyan().bold(), message);
        }
    }


    pub fn warn (&self, message: &str) {
        println!("{} {}", "warn:".bright_yellow().bold(), message);

    }


    pub fn debug (&self, message: &str) {
        if self.level >= 2 {
            println!("{} {}", "debug:".bright_magenta().bold(), message);
        }
    }

    pub fn option (&self, function: &str, message: &str, options: Vec<String>, default: Option<u8>) -> u8 {
        
        // log the message with the given class
        match function {
            "error" => self.error(message),
            "success" => self.success(message),
            "warn" => self.warn(message),
            "debug" => self.debug(message),
            _ => self.info(message)
        }

        // print the option tree
        for (i, option) in options.iter().enumerate() {
            println!(
                "  {} {}: {}",
                if i == options.len() - 1 { "└─" } else { "├─" },
                i.to_string(),
                option
            );
        }
        
        // flush output print prompt
        let mut selection = String::new();
        print!(
            "\nSelect an option {}: ",
            if default.is_some() {
                format!("(default: {})", default.unwrap())
            } else {
                "".to_string()
            }
        );
        let _ = stdout().flush();

        // get input
        match stdin().read_line(&mut selection) {
            Ok(_) => {
                // check if default was selected
                if selection.trim() == "" {
                    if default.is_some() {
                        return default.unwrap();
                    } else {
                        self.error("Invalid selection.");
                        return self.option(function, message, options, default);
                    }
                }

                // check if the input is a valid option
                let selected_index = match selection.trim().parse::<u8>() {
                    Ok(i) => i,
                    Err(_) => {
                        self.error("Invalid selection.");
                        return self.option(function, message, options, default);
                    }
                };

                if match options.get(selected_index as usize) {
                    Some(_) => true,
                    None => false
                } {
                    return selected_index;
                } else {
                    self.error("Invalid selection.");
                    return self.option(function, message, options, default);
                }
            },
            Err(_) => {
                self.error("Invalid selection.");
                return self.option(function, message, options, default);
            }
        };
    }

}



#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;


    #[test]
    fn test_raw_trace() {
        let start_time = Instant::now();
        let (logger, mut trace)= Logger::new("TRACE");
        
        let parent = trace.add("call", 0, 123123, vec!["Test::test_trace()".to_string()]);
        trace.add("log", parent, 234234, vec!["ContractCreated(contractAddress: 0x0000000000000000000000000000000000000000)".to_string()]);
        let inner = trace.add("create", parent, 121234, vec!["TestContract".to_string(), "0x0000000000000000000000000000000000000000".to_string(), "917".to_string()]);
        trace.add("log_unknown", inner, 12344, vec!["0x0000000000000000000000000000000000000000000000000000000000000000".to_string()]);
        let deeper = trace.add("call", inner, 12344, vec!["Test::transfer(to: 0x0000000000000000000000000000000000000000, amount: 1)".to_string(), "true".to_string()]);
        trace.add("log", deeper, 12344, vec!["Transfer(from: 0x0000000000000000000000000000000000000000, to: 0x0000000000000000000000000000000000000000, amount: 1)".to_string()]);
        trace.add("message", inner, 12344, vec!["warn: Transfer to the zero address!".to_string()]);
        trace.add("message", parent, 12344, vec!["Execution Reverted: Out of Gas.".to_string(), "Execution Reverted: Out of Gas.".to_string()]);

        trace.display();
        logger.info(&format!("Tracing took {}", start_time.elapsed().as_secs_f64()));
    }

    
    #[test]
    fn test_helper_functions() {
        let start_time = Instant::now();
        let (logger, mut trace)= Logger::new("TRACE");
        
        let parent = trace.add_call(0, 123, "Test".to_string(), "test_trace".to_string(), vec!["arg1: 0x0".to_string(), "arg2: 0x1".to_string(),], "()".to_string());
        trace.add_creation(parent, 124, "TestContract".to_string(), "0x0000000000000000000000000000000000000000".to_string(), 1232);
        trace.add_emission(parent, 125, "ContractCreated".to_string(), vec!["contractAddress: 0x0000000000000000000000000000000000000000".to_string()]);
        trace.add_raw_emission(parent, 125, vec!["0x0000000000000000000000000000000000000000000000000000000000000000".to_string(), "0x0000000000000000000000000000000000000000000000000000000000000000".to_string()], "0x".to_string());
        trace.add_error(parent, 126, "Testing errors".to_string());
        trace.add_info(parent, 127, "Testing info".to_string());
        trace.add_message(parent, 128, vec!["test multiple".to_string(), "lines".to_string(), "to tracing".to_string()]);

        trace.display();
        logger.info(&format!("Tracing took {}", start_time.elapsed().as_secs_f64()));
    }


    #[test]
    fn test_option() {
        let (logger, _)= Logger::new("TRACE");

        logger.option("warn", "multiple possibilities", vec!["option 1".to_string(), "option 2".to_string(), "option 3".to_string()], Some(0));
    }

}