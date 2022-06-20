use colored::*;


#[derive(Clone, Debug)]
pub struct Logger {
    pub level: u8,
    pub last_depth: u8
}

impl Logger {

    // create a new logger
    pub fn new(verbosity: &str) -> Logger {

        match verbosity {
            "ERROR" => Logger { level: 0, last_depth: 0 },
            "WARN" => Logger { level: 1, last_depth: 0 },
            "INFO" => Logger { level: 2, last_depth: 0 },
            "DEBUG" => Logger { level: 3, last_depth: 0 },
            "TRACE" => Logger { level: 4, last_depth: 0 },
            _  => Logger { level: 2, last_depth: 0 },
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

    // TODO: trace is a different beast, needs to be implemented with
    // a format that includes line number, method, depht, and message.
    //pub fn trace (&self, depth: usize, message: &str, value: &str) {
        
    //}
}