use colored::Colorize;
use ethers::abi::Token;

/// A helper function used by the decode module to pretty format decoded tokens.
pub fn display(inputs: Vec<Token>, prefix: &str) -> Vec<String> {
    let mut output = Vec::new();
    let prefix = prefix.to_string();

    for input in inputs {
        match input {
            Token::Address(_) => output.push(format!("{prefix}{} 0x{input}", "address".blue())),
            Token::Int(val) => output.push(format!("{prefix}{} {}", "int    ".blue(), val)),
            Token::Uint(val) => output.push(format!("{prefix}{} {}", "uint   ".blue(), val)),
            Token::String(val) => output.push(format!("{prefix}{} {val}", "string ".blue())),
            Token::Bool(val) => {
                if val {
                    output.push(format!("{prefix}{} true", "bool   ".blue()));
                } else {
                    output.push(format!("{prefix}{} false", "bool   ".blue()));
                }
            }
            Token::FixedBytes(_) | Token::Bytes(_) => {
                let bytes = input
                    .to_string()
                    .chars()
                    .collect::<Vec<char>>()
                    .chunks(64)
                    .map(|c| c.iter().collect::<String>())
                    .collect::<Vec<String>>();

                for (i, byte) in bytes.iter().enumerate() {
                    if i == 0 {
                        output.push(format!("{prefix}{} 0x{}", "bytes  ".blue(), byte));
                    } else {
                        output.push(format!("{prefix}{}   {}", "       ".blue(), byte));
                    }
                }
            }
            Token::FixedArray(val) | Token::Array(val) => {
                if val.is_empty() {
                    output.push(format!("{prefix}[]"));
                } else {
                    output.push(format!("{prefix}["));
                    output.extend(display(val.to_vec(), &format!("{prefix}   ")));
                    output.push(format!("{prefix}]"));
                }
            }
            Token::Tuple(val) => {
                if val.is_empty() {
                    output.push(format!("{prefix}()"));
                } else {
                    output.push(format!("{prefix}("));
                    output.extend(display(val.to_vec(), &format!("{prefix}   ")));
                    output.push(format!("{prefix})"));
                }
            }
        };
    }

    output
}
