use ethers::abi::Token;

use crate::utils::{http::get_json_from_url, strings::replace_last};


#[derive(Debug, Clone)]
pub struct ResolvedFunction {
    pub name: String,
    pub signature: String,
    pub inputs: Vec<String>,
    pub decoded_inputs: Option<Vec<Token>>,
}


pub fn resolve_signature(signature: &String) -> Option<Vec<ResolvedFunction>> {

    // get function possibilities from 4byte
    let signatures = match get_json_from_url(format!("https://sig.eth.samczsun.com/api/v1/signatures?all=true&function=0x{}", &signature)) {
        Some(signatures) => signatures,
        None => return None
    };

    // convert the serde value into a vec of possible functions
    // AAAHAHHHHHH IM MATCHING
    let results = match signatures.get("result") {
        Some(result) => match result.get("function") {
            Some(function) => match function.get(format!("0x{signature}")) {
                Some(functions) => match functions.as_array() {
                    Some(functions) => functions.to_vec(),
                    None => return None
                },
                None => return None
            },
            None => return None
        },
        None => return None
    };

    let mut signature_list: Vec<ResolvedFunction> = Vec::new();

    for signature in results {

        // get the function text signature and unwrap it into a string
        let text_signature = match signature.get("name") {
            Some(text_signature) => text_signature.to_string().replace("\"", ""),
            None => continue
        };
        
        // safely split the text signature into name and inputs
        let function_parts = match text_signature.split_once("(") {
            Some(function_parts) => function_parts,
            None => continue
        };

        signature_list.push(ResolvedFunction {
            name: function_parts.0.to_string(),
            signature: text_signature.to_string(),
            inputs: replace_last(function_parts.1.to_string(), ")", "").split(",").map(|input| input.to_string()).collect(),
            decoded_inputs: None
        });

    }

    return match signature_list.len() {
        0 => None,
        _ => Some(signature_list)
    }

}