use hashbrown::{HashMap, HashSet};

use alloy::primitives::Address;
use futures::future::try_join_all;

use crate::{error::Error, InspectArgs};
use heimdall_common::{resources::transpose::get_label, utils::hex::ToLowerHex};

#[derive(Debug, Clone)]
pub struct Contracts {
    pub contracts: HashMap<Address, String>,
    transpose_api_key: String,
    skip_resolving: bool,
}

#[allow(dead_code)]
impl Contracts {
    pub fn new(args: &InspectArgs) -> Self {
        Self {
            contracts: HashMap::new(),
            transpose_api_key: args.transpose_api_key.clone(),
            skip_resolving: args.skip_resolving,
        }
    }

    pub async fn add(&mut self, address: Address) -> Result<(), Error> {
        // if skip resolving, just add the address
        if self.skip_resolving {
            self.contracts.insert(address, address.to_lower_hex());
            return Ok(());
        }

        // if alias already exists, don't overwrite
        if self.contracts.contains_key(&address) {
            return Ok(());
        }

        if !self.transpose_api_key.is_empty() {
            self.contracts.insert(
                address,
                get_label(&address.to_lower_hex(), &self.transpose_api_key)
                    .await
                    .unwrap_or_else(|| address.to_lower_hex()),
            );
        } else {
            self.contracts.insert(address, address.to_lower_hex());
        }

        Ok(())
    }

    pub async fn extend(&mut self, addresses: HashSet<Address>) -> Result<(), Error> {
        // if skip resolving, just add the address
        if self.skip_resolving {
            self.contracts
                .extend(addresses.into_iter().map(|address| (address, address.to_lower_hex())));
            return Ok(());
        }

        // for each address, get the label
        if !self.transpose_api_key.is_empty() {
            let transpose_api_key = self.transpose_api_key.clone();
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

            self.contracts.extend(addresses.into_iter().zip(labels.into_iter()).map(
                |(address, label)| (address, label.unwrap_or_else(|| address.to_lower_hex())),
            ));
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
