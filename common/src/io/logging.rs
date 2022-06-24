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
    pub fn add_trace(&mut self, parent_index: u32, instruction: u32, message: Vec<String>) -> u32{
        
        // build the new trace
        let trace = Trace::new(
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
    pub fn print(self) {
        
    }
}

impl Trace {
    
    // create a new trace
    pub fn new(parent_index: u32, instruction: u32, message: Vec<String>) -> Trace {
        Trace {
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