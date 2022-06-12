use colored::*;


pub struct Logger {
    pub level: u8
}

impl Logger {

    // create a new logger
    pub fn new(verbosity: &str) -> Logger {

        match verbosity {
            "ERROR" => Logger { level: 0 },
            "WARN" => Logger { level: 1 },
            "INFO" => Logger { level: 2 },
            "DEBUG" => Logger { level: 3 },
            "TRACE" => Logger { level: 4 },
            _  => Logger { level: 2 }
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
    pub fn trace (&self, message: &str) {
        if self.level >= 4 {
            println!("{} {}", "trace:".bright_blue().bold(), message);
        }
    }
}