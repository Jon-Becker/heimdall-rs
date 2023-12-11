use std::collections::{HashMap, HashSet};

use ethers::types::Address;
use futures::future::try_join_all;

use crate::{error::Error, inspect::InspectArgs};
use heimdall_common::{resources::transpose::get_label, utils::hex::ToLowerHex};

#[derive(Debug, Clone)]
pub struct Contracts {
    pub contracts: HashMap<Address, String>,
    transpose_api_key: Option<String>,
}

#[allow(dead_code)]
impl Contracts {
    pub fn new(args: &InspectArgs) -> Self {
        Self { contracts: HashMap::new(), transpose_api_key: args.transpose_api_key.clone() }
    }

    pub async fn add(&mut self, address: Address) -> Result<(), Error> {
        // if alias already exists, don't overwrite
        if self.contracts.contains_key(&address) {
            return Ok(());
        }

        if let Some(transpose_api_key) = &self.transpose_api_key {
            self.contracts.insert(
                address,
                get_label(&address.to_lower_hex(), transpose_api_key)
                    .await
                    .unwrap_or(address.to_lower_hex()),
            );
        } else {
            self.contracts.insert(address, address.to_lower_hex());
        }

        Ok(())
    }

    pub async fn extend(&mut self, addresses: HashSet<Address>) -> Result<(), Error> {
        // for each address, get the label
        if let Some(transpose_api_key) = &self.transpose_api_key {
            let handles: Vec<_> = addresses
                .clone()
                .into_iter()
                .map(move |address| {
                    let transpose_api_key = transpose_api_key.clone();
                    tokio::spawn(async move {
                        get_label(&address.to_lower_hex(), &transpose_api_key).await
                    })
                })
                .collect();

            let labels =
                try_join_all(handles).await.map_err(|e| Error::TransposeError(e.to_string()))?;

            self.contracts.extend(
                addresses
                    .into_iter()
                    .zip(labels.into_iter())
                    .map(|(address, label)| (address, label.unwrap_or(address.to_lower_hex()))),
            );
            // replace None
        } else {
            self.contracts
                .extend(addresses.into_iter().map(|address| (address, address.to_lower_hex())));
        }

        Ok(())
    }

    pub fn get(&self, address: Address) -> Option<&String> {
        self.contracts.get(&address)
    }
}
