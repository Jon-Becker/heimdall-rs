use hashbrown::{HashMap, HashSet};

use alloy::primitives::Address;
use futures::future::try_join_all;
use std::future::Future;

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
        if self.skip_resolving {
            self.contracts.insert(address, address.to_lower_hex());
            return Ok(());
        }

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
        if self.skip_resolving {
            self.contracts
                .extend(addresses.into_iter().map(|address| (address, address.to_lower_hex())));
            return Ok(());
        }

        if !self.transpose_api_key.is_empty() {
            let api_key = self.transpose_api_key.clone();
            let labels = resolve_labels(addresses, move |address| {
                let api_key = api_key.clone();
                async move { get_label(&address.to_lower_hex(), &api_key).await }
            })
            .await?;
            self.contracts.extend(labels.into_iter().map(|(address, label)| {
                (address, label.unwrap_or_else(|| address.to_lower_hex()))
            }));
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

async fn resolve_labels<F, Fut>(
    addresses: HashSet<Address>,
    resolve: F,
) -> Result<Vec<(Address, Option<String>)>, Error>
where
    F: Fn(Address) -> Fut,
    Fut: Future<Output = Option<String>> + Send + 'static, {
    let handles = addresses.into_iter().map(|address| {
        let result = resolve(address);
        tokio::spawn(async move { (address, result.await) })
    });
    try_join_all(handles).await.map_err(|error| Error::TransposeError(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resolved_labels_stay_with_their_addresses() {
        let first = Address::from([1; 20]);
        let second = Address::from([2; 20]);
        let third = Address::from([3; 20]);
        let addresses = [first, second, third].into_iter().collect();
        let labels = resolve_labels(addresses, |address| async move {
            tokio::task::yield_now().await;
            Some(format!("label-{address}"))
        })
        .await
        .unwrap();
        let labels: HashMap<_, _> = labels.into_iter().collect();
        for address in [first, second, third] {
            assert_eq!(
                labels.get(&address).unwrap().as_deref(),
                Some(format!("label-{address}").as_str())
            );
        }
    }
}
