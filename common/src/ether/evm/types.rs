use colored::Colorize;
use ethers::abi::{ParamType, Token};

use crate::utils::strings::replace_last;

// decode a string into an ethereum type
pub fn parse_function_parameters(function_signature: String) -> Option<Vec<ParamType>> {

    let mut function_inputs = Vec::new();
    
    // get only the function input body, removing the name and input wrapping parentheses
    let string_inputs = match function_signature.split_once("(") {
        Some((_, inputs)) => replace_last(inputs.to_string(), ")", ""),
        None => replace_last(function_signature, ")", ""),
    };

    // split into individual inputs
    let temp_inputs: Vec<String> = string_inputs.split(",").map(|s| s.to_string()).collect();
    let mut inputs: Vec<String> = Vec::new();

    // if the input contains complex types, rejoin them. for nested types, this function will recurse.
    if string_inputs.contains("(") {
        let mut tuple_depth = 0;
        let mut complex_input: Vec<String> = Vec::new();

        for input in temp_inputs {
            if input.contains("(") {
                tuple_depth += 1;
            }

            if tuple_depth > 0 { complex_input.push(input.to_string()); }
            else { inputs.push(input.to_string()); }

            if input.contains(")") {
                tuple_depth -= 1;

                if tuple_depth == 0 {
                    inputs.push(complex_input.join(","));
                    complex_input = Vec::new();
                }
            }
        }
    }
    else {
        inputs = temp_inputs;
    }

    // parse each input into an ethereum type, recusing if necessary
    for solidity_type in inputs {
        if solidity_type == "address" { function_inputs.push(ParamType::Address); continue }
        if solidity_type == "bytes" { function_inputs.push(ParamType::Bytes); continue }
        if solidity_type == "bool" { function_inputs.push(ParamType::Bool); continue }
        if solidity_type == "string" { function_inputs.push(ParamType::String); continue }
        if solidity_type.starts_with("(") && !solidity_type.ends_with("]") {
            let complex_inputs = match parse_function_parameters(solidity_type.clone()) {
                Some(inputs) => inputs,
                None => continue,
            };
            function_inputs.push(ParamType::Tuple(complex_inputs));
            continue
        }
        if solidity_type.ends_with("[]") {
            let array_type = match parse_function_parameters(solidity_type.replace("[]", "")) {
                Some(types_) => types_,
                None => continue,
            };

            if array_type.len() == 1 {
                function_inputs.push(ParamType::Array(Box::new(array_type[0].clone())));
            }
            else {
                function_inputs.push(ParamType::Array(Box::new(ParamType::Tuple(array_type))));
            }
            continue
        }
        if solidity_type.ends_with("]") {
            let size = match solidity_type.split("[").nth(1) {
                Some(size) => match size.replace("]", "").parse::<usize>() {
                    Ok(size) => size,
                    Err(_) => continue,
                },
                None => continue,
            };
            let array_type = match parse_function_parameters(solidity_type.replace("[]", "")) {
                Some(types_) => types_,
                None => continue,
            };

            if array_type.len() == 1 {
                function_inputs.push(ParamType::FixedArray(Box::new(array_type[0].clone()), size));
            }
            else {
                function_inputs.push(ParamType::FixedArray(Box::new(ParamType::Tuple(array_type)), size));
            }
            continue
        }
        if solidity_type.starts_with("int") {
            let size = match solidity_type.replace("int", "").parse::<usize>() {
                Ok(size) => size,
                Err(_) => 256,
            };
            function_inputs.push(ParamType::Int(size));
            continue
        }
        if solidity_type.starts_with("uint") {
            let size = match solidity_type.replace("uint", "").parse::<usize>() {
                Ok(size) => size,
                Err(_) => 256,
            };
            
            function_inputs.push(ParamType::Uint(size));
            continue
        }
        if solidity_type.starts_with("bytes") {
            let size = match solidity_type.replace("bytes", "").parse::<usize>() {
                Ok(size) => size,
                Err(_) => 32,
            };
        
            function_inputs.push(ParamType::FixedBytes(size));
            continue
        }
    }    

    
    match function_inputs.len() {
        0 => None,
        _ => Some(function_inputs)
    }
}


// returns a vec of beautified types for a given vec of tokens
pub fn display(inputs: Vec<Token>, prefix: &str) -> Vec<String> {
    let mut output = Vec::new();
    let prefix = prefix.to_string();

    for input in inputs {
        match input {
            Token::Address(_) => output.push(format!("{prefix}{} 0x{input}", "address".dimmed().blue())),
            Token::Int(val) => output.push(format!("{prefix}{} {}", "int    ".dimmed().blue(), val.to_string())),
            Token::Uint(val) => output.push(format!("{prefix}{} {}", "uint   ".dimmed().blue(), val.to_string())),
            Token::String(val) => output.push(format!("{prefix}{} {val}", "string ".dimmed().blue())),
            Token::Bool(val) => {
                if val { output.push(format!("{prefix}{} true", "bool   ".dimmed().blue())); }
                else { output.push(format!("{prefix}{} false",  "bool   ".dimmed().blue())); }
            },
            Token::FixedBytes(_) | Token::Bytes(_) => {
                let bytes = input.to_string().chars().collect::<Vec<char>>().chunks(64).map(|c| c.iter().collect::<String>()).collect::<Vec<String>>();

                for (i, byte) in bytes.iter().enumerate() {
                    if i == 0 {
                        output.push(format!("{prefix}{} 0x{}",  "bytes  ".dimmed().blue(), byte));
                    }
                    else {
                        output.push(format!("{prefix}{}   {}",  "       ".dimmed().blue(), byte));
                    }
                }
            },
            Token::FixedArray(val) | Token::Array(val) => {
                if val.len() == 0 {
                    output.push(format!("{prefix}[]"));
                }
                else {
                    output.push(format!("{prefix}["));
                    output.extend(display(val.to_vec(), &format!("{prefix}   ")));
                    output.push(format!("{prefix}]"));
                }
            },
            Token::Tuple(val) => {
                if val.len() == 0 {
                    output.push(format!("{prefix}()"));
                }
                else {
                    output.push(format!("{prefix}("));
                    output.extend(display(val.to_vec(), &format!("{prefix}   ")));
                    output.push(format!("{prefix})"));
                }  
                
            },
        };
    }

    output
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple() {
        let solidity_type = "test(uint256)".to_string();
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(
                vec![
                    ParamType::Uint(256)
                ]
            )
        );
    }

    #[test]
    fn test_mul() {
        let solidity_type = "test(uint256,string)".to_string();
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(
                vec![
                    ParamType::Uint(256),
                    ParamType::String
                ]
            )
        );
    }

    #[test]
    fn test_array() {
        let solidity_type = "test(uint256,string[],uint256)".to_string();
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(
                vec![
                    ParamType::Uint(256),
                    ParamType::Array(
                        Box::new(ParamType::String)
                    ),
                    ParamType::Uint(256)
                ]
            )
        );
    }

    #[test]
    fn test_complex() {
        let solidity_type = "test(uint256,string,(address,address,uint24,address,uint256,uint256,uint256,uint160))".to_string();
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(
                vec![
                    ParamType::Uint(256),
                    ParamType::String,
                    ParamType::Tuple(
                        vec![
                            ParamType::Address,
                            ParamType::Address,
                            ParamType::Uint(24),
                            ParamType::Address,
                            ParamType::Uint(256),
                            ParamType::Uint(256),
                            ParamType::Uint(256),
                            ParamType::Uint(160)
                        ]
                    )
                ]
            )
        );
    }

    #[test]
    fn test_tuple() {
        let solidity_type = "exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))".to_string();
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(
                vec![
                    ParamType::Tuple(
                        vec![
                            ParamType::Address,
                            ParamType::Address,
                            ParamType::Uint(24),
                            ParamType::Address,
                            ParamType::Uint(256),
                            ParamType::Uint(256),
                            ParamType::Uint(256),
                            ParamType::Uint(160)
                        ]
                    )
                ]
            )
        );
    }

    #[test]
    fn test_nested_tuple() {
        let solidity_type = "exactInputSingle((address,address,uint24,address,uint256,(uint256,uint256)[],uint160))".to_string();
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(
                vec![
                    ParamType::Tuple(
                        vec![
                            ParamType::Address,
                            ParamType::Address,
                            ParamType::Uint(24),
                            ParamType::Address,
                            ParamType::Uint(256),
                            ParamType::Array(
                                Box::new(ParamType::Tuple(
                                    vec![
                                        ParamType::Uint(256),
                                        ParamType::Uint(256)
                                    ]
                                ))
                            ),
                            ParamType::Uint(160)
                        ]
                    )
                ]
            )
        );
    }

    #[test]
    fn test_wtf() {
        let solidity_type = "marketBuyOrdersWithEth((address,address,address,address,uint256,uint256,uint256,uint256,uint256,uint256,bytes,bytes,bytes,bytes)[],uint256,bytes[],uint256[],address[])".to_string();
        let param_type = parse_function_parameters(solidity_type);

        println!("{:#?}", param_type)
    }

}