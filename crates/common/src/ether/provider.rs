//! Create a custom data transport to use with a Provider.
use alloy::{
    network::Ethereum,
    providers::{ext::TraceApi, IpcConnect, Provider, ProviderBuilder, RootProvider, WsConnect},
    pubsub::PubSubFrontend,
    rpc::types::Transaction,
    transports::http::Http,
};
use eyre::Result;
use reqwest::{Client, Url};
use std::{fmt::Debug, str::FromStr};

/// [`MultiTransportProvider`] is a convenience wrapper around the different transport types
/// supported by the [`Provider`].
#[derive(Clone, Debug)]
pub enum MultiTransportProvider {
    Ws(RootProvider<PubSubFrontend, Ethereum>),
    Ipc(RootProvider<PubSubFrontend, Ethereum>),
    Http(RootProvider<Http<Client>, Ethereum>),
}

// We implement a convenience "constructor" method, to easily initialize the transport.
// This will connect to [`Http`] if the rpc_url contains 'http', to [`Ws`] if it contains 'ws',
// otherwise it'll default to [`Ipc`].
impl MultiTransportProvider {
    pub async fn connect(rpc_url: &str) -> Result<Self> {
        if rpc_url.is_empty() {
            return Err(eyre::eyre!("No RPC URL provided"));
        }

        let url = Url::from_str(rpc_url)?;

        let this = if rpc_url.to_lowercase().contains("http") {
            Self::Http(ProviderBuilder::new().on_http(url))
        } else if rpc_url.to_lowercase().contains("ws") {
            let ws = WsConnect::new(rpc_url);
            Self::Ws(ProviderBuilder::new().on_ws(ws).await?)
        } else {
            let ipc = IpcConnect::new(rpc_url.to_string());
            Self::Ipc(ProviderBuilder::new().on_ipc(ipc).await?)
        };
        Ok(this)
    }

    pub async fn get_chainid(&self) -> Result<u64> {
        Ok(match self {
            Self::Ws(provider) => provider.get_chain_id().await?,
            Self::Ipc(provider) => provider.get_chain_id().await?,
            Self::Http(provider) => provider.get_chain_id().await?,
        })
    }

    pub async fn get_block_number(&self) -> Result<u64> {
        Ok(match self {
            Self::Ws(provider) => provider.get_block_number().await?,
            Self::Ipc(provider) => provider.get_block_number().await?,
            Self::Http(provider) => provider.get_block_number().await?,
        })
    }

    pub async fn get_code_at(&self, address: &str) -> Result<Vec<u8>> {
        let address = address.parse()?;

        Ok(match self {
            Self::Ws(provider) => provider.get_code_at(address).await?,
            Self::Ipc(provider) => provider.get_code_at(address).await?,
            Self::Http(provider) => provider.get_code_at(address).await?,
        }
        .to_vec())
    }

    pub async fn get_transaction_by_hash(&self, tx_hash: &str) -> Result<Option<Transaction>> {
        let tx_hash = tx_hash.parse()?;

        Ok(match self {
            Self::Ws(provider) => provider.get_transaction_by_hash(tx_hash).await?,
            Self::Ipc(provider) => provider.get_transaction_by_hash(tx_hash).await?,
            Self::Http(provider) => provider.get_transaction_by_hash(tx_hash).await?,
        })
    }

    pub async fn trace_replay_transaction(&self, tx_hash: &str) -> Result<Option<Transaction>> {
        let tx_hash = tx_hash.parse()?;

        Ok(match self {
            Self::Ws(provider) => provider.trace_replay_transaction(tx_hash).await?,
            Self::Ipc(provider) => provider.trace_replay_transaction(tx_hash).await?,
            Self::Http(provider) => provider.trace_replay_transaction(tx_hash).await?,
        })
    }
}
