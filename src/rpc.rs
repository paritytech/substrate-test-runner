use crate::types;
use std::convert::TryFrom;
use jsonrpc_core_client::RpcChannel;

pub use jsonrpc_core::types::params::Params;
pub use jsonrpc_core_client::RawClient;

pub type ChainClient<T> = sc_rpc_api::chain::ChainClient<
    types::BlockNumber<T>,
    types::BlockHash<T>,
    types::Header<T>,
    types::SignedBlock<T>,
>;

