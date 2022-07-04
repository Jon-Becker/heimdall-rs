use ethers::abi::{ParamType};

use crate::utils::strings::replace_last;


// decode a string into an ethereum type
pub fn to_abi_type(solidity_type: String) -> Option<ParamType> {
    
    // trim
    let solidity_type = solidity_type.trim();

    if solidity_type == "address" { return Some(ParamType::Address); }
    if solidity_type == "bytes" { return Some(ParamType::Bytes); }
    if solidity_type == "bool" { return Some(ParamType::Bool); }
    if solidity_type == "string" { return Some(ParamType::String); }

    if solidity_type.contains("(") {
        let mut params: Vec<ParamType> = Vec::new();
        let parts = replace_last(solidity_type.replacen("(", "", 1), ")", "");
        
        for part in parts.split(",") {
            let param_type = match to_abi_type(part.to_string()) {
                Some(type_) => type_,
                None => return None,
            };

            params.push(param_type);
        }

        return Some(ParamType::Tuple(params));
    }

    if solidity_type.contains("[]") {
        let array_type = match to_abi_type(solidity_type.replace("[]", "")) {
            Some(type_) => type_,
            None => return None,
        };

        return Some(ParamType::Array(Box::new(array_type)));
    }

    if solidity_type.contains("[") {
        let size = match solidity_type.split("[").nth(1) {
            Some(size) => match size.replace("]", "").parse::<usize>() {
                Ok(size) => size,
                Err(_) => return None,
            },
            None => return None,
        };
        let array_type = match solidity_type.split("[").nth(0) {
            Some(array_type) => match to_abi_type(array_type.to_string()) {
                Some(type_) => type_,
                None => return None,
            },
            None => return None,
        };

        return Some(ParamType::FixedArray(Box::new(array_type), size));
    }

    if solidity_type.starts_with("uint") {
        let size = match solidity_type.replace("uint", "").parse::<usize>() {
            Ok(size) => size,
            Err(_) => return Some(ParamType::Uint(256)),
        };
        
        return Some(ParamType::Uint(size));
    }

    if solidity_type.starts_with("int") {
        let size = match solidity_type.replace("int", "").parse::<usize>() {
            Ok(size) => size,
            Err(_) => return Some(ParamType::Uint(256)),
        };
        
        return Some(ParamType::Uint(size));
    }

    if solidity_type.starts_with("uint") {
        let size = match solidity_type.replace("uint", "").parse::<usize>() {
            Ok(size) => size,
            Err(_) => return Some(ParamType::Int(256)),
        };
        
        return Some(ParamType::Uint(size));
    }

    if solidity_type.starts_with("bytes") {
        let size = match solidity_type.replace("bytes", "").parse::<usize>() {
            Ok(size) => size,
            Err(_) => return Some(ParamType::Uint(256)),
        };
        
        return Some(ParamType::FixedBytes(size));
    }

    None

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uint_dyn() {
        let solidity_type = "uint256".to_string();
        let param_type = to_abi_type(solidity_type);
        assert_eq!(param_type, Some(ParamType::Uint(256)));
    }

    #[test]
    fn test_array() {
        let solidity_type = "uint256[]".to_string();
        let param_type = to_abi_type(solidity_type);
        assert_eq!(param_type, Some(ParamType::Array(Box::new(ParamType::Uint(256)))));
    }

    #[test]
    fn test_tuple() {
        let solidity_type = "(uint256,uint256[])".to_string();
        let param_type = to_abi_type(solidity_type);
        assert_eq!(param_type, Some(ParamType::Tuple(vec![ParamType::Uint(256), ParamType::Uint(256)])));
    }

    #[test]
    fn test_nested_tuple() {
        let solidity_type = "(address,uint256,uint256,address,address,address,uint256,uint256,uint8,uint256,uint256,bytes32,uint256,bytes32,bytes32,uint256,(uint256,address)[],bytes)".to_string();
        let param_type = to_abi_type(solidity_type);
        assert_eq!(param_type, Some(ParamType::Tuple(vec![ParamType::Uint(256), ParamType::Uint(256)])));
    }
}