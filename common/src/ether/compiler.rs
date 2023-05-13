// returns the compiler version used to compile the contract.
// for example: (solc, 0.8.10) or (vyper, 0.2.16)
pub fn detect_compiler(bytecode: String) -> (String, String) {
    let mut compiler = "unknown".to_string();
    let mut version = "unknown".to_string();

    // perfom prefix check for rough version matching
    if bytecode.starts_with("363d3d373d3d3d363d73") {
        compiler = "proxy".to_string();
        version = "minimal".to_string();
    } else if bytecode.starts_with("366000600037611000600036600073") {
        compiler = "proxy".to_string();
        version = "vyper".to_string();
    } else if bytecode.starts_with("6004361015") {
        compiler = "vyper".to_string();
        version = "0.2.0-0.2.4,0.2.11-0.3.3".to_string();
    } else if bytecode.starts_with("341561000a") {
        compiler = "vyper".to_string();
        version = "0.2.5-0.2.8".to_string();
    } else if bytecode.starts_with("731bf797") {
        compiler = "solc".to_string();
        version = "0.4.10-0.4.24".to_string();
    } else if bytecode.starts_with("6080604052") {
        compiler = "solc".to_string();
        version = "0.4.22+".to_string();
    } else if bytecode.starts_with("6060604052") {
        compiler = "solc".to_string();
        version = "0.4.11-0.4.21".to_string();
    } else if bytecode.contains("7679706572") {
        compiler = "vyper".to_string();
    } else if bytecode.contains("736f6c63") {
        compiler = "solc".to_string();
    }

    // perform metadata check
    if compiler == "solc" {
        let compiler_version = bytecode.split("736f6c6343").collect::<Vec<&str>>();

        if compiler_version.len() > 1 {
            if let Some(encoded_version) = compiler_version[1].get(0..6) {
                let version_array = encoded_version
                    .chars()
                    .collect::<Vec<char>>()
                    .chunks(2)
                    .map(|c| c.iter().collect::<String>())
                    .collect::<Vec<String>>();

                version = String::new();
                for version_part in version_array {
                    version
                        .push_str(&format!("{}.", u8::from_str_radix(&version_part, 16).unwrap()));
                }
            }
        }
    } else if compiler == "vyper" {
        let compiler_version = bytecode.split("767970657283").collect::<Vec<&str>>();

        if compiler_version.len() > 1 {
            if let Some(encoded_version) = compiler_version[1].get(0..6) {
                let version_array = encoded_version
                    .chars()
                    .collect::<Vec<char>>()
                    .chunks(2)
                    .map(|c| c.iter().collect::<String>())
                    .collect::<Vec<String>>();

                version = String::new();
                for version_part in version_array {
                    version
                        .push_str(&format!("{}.", u8::from_str_radix(&version_part, 16).unwrap()));
                }
            }
        }
    }

    (compiler, version.trim_end_matches('.').to_string())
}
