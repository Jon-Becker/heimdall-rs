use heimdall_core::decode::DecodeArgsBuilder;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::collections::HashMap;
use serde_yaml;
use serde::{Deserialize, Serialize};


// Define a struct to represent the value in the YAML file
#[derive(Debug, Serialize, Deserialize)]
struct FunctionInfo {
    signature: Option<String>,
    source: String,
}
fn load_data() -> HashMap<String, String> {
    // file from https://docs.openchain.xyz/#/default/get_signature_database_v1_export
    let file = File::open("all_selectors.csv").unwrap();
    let reader = BufReader::new(file);
    let mut data = HashMap::new();
    for line in reader.lines() {
        let line = line.unwrap();
        let mut parts = line.split(",");
        let selector = parts.next().unwrap().to_string();
        let text = parts.next().unwrap().to_string();
        data.insert(selector, text);
    }
    // replace keys with those in canonical
    // file from https://github.com/openchainxyz/canonical-signatures/blob/main/canonical.yaml
    let filters_file = File::open("canonical.yaml").unwrap();
    let filters: HashMap<String, FunctionInfo> = serde_yaml::from_reader(filters_file).unwrap();
    // for any key, replace the previous hashMap value with it's value under signature
    for (key, value) in filters {
        // skip fields without .signature field
        if let Some(signature) = value.signature {
            data.insert(key, signature);
        }
    }
    data
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Loading selectors...");
    let selectors_db = load_data();
    // print data length
    println!("selectors_db: {:?}", selectors_db.len());

    // save current time
    let start = std::time::Instant::now();

    let result = heimdall_core::decode::decode(
        DecodeArgsBuilder::new()
            .target("0x23b872dd0000000000000000000000000eb4b371bf89b641538243de10e0feceae60719800000000000000000000000071b5759d73262fbb223956913ecf4ecc5105764100000000000000000000000000000000000000000000ac826898ac76a279dc4b".to_string())
            .skip_resolving(true)
            .build()?,
    )
    .await?;
    // for every found function, get the name
    for function in result {
        // get the selector
        let selector = function.name;
        // we need to remove "Unresolved_" from the selector and replace it with 0x
        let selector = selector.replace("Unresolved_", "0x");
        // get the name from the selector hashmap
        let name = selectors_db.get(&selector);
        // print the name
        println!("name: {:?}", name);
        // print decoded inputs if any
        println!("inputs: {:?}", function.inputs);
        // print decode_inputs if any
        if function.decoded_inputs.is_some() {
            println!("decoded_inputs: {:?}", function.decoded_inputs);
        }
    }
    
    // print time elapsed
    println!("Time elapsed: {:?}", start.elapsed());

    Ok(())
}