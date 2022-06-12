use colored::*;

pub fn error(message: &str){
    println!("{} {}", "error:".bright_red().bold(), message);
    std::process::exit(1);
}


pub fn success(message: &str){
    println!("{} {}", "success:".bright_green().bold(), message);
}


pub fn info(message: &str){
    println!("{} {}", "info:".bright_cyan().bold(), message);
}


pub fn warning(message: &str){
    println!("{} {}", "warn:".bright_yellow().bold(), message);
}


// TODO: in the future, possibly add a verbose flag to this function
// potentially make this a Logger struct, which stores the verbosity level
// and can be used to selectively print messages based on the verbosity level