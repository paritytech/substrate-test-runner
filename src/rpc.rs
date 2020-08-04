use crate::types;
use jsonrpc_core_client::RpcChannel;

use futures::compat::Future01CompatExt;
pub use jsonrpc_core::types::params::Params;
pub use jsonrpc_core_client::RawClient;
use jsonrpc_core_client::RpcError;

pub type ChainClient<T> =
	sc_rpc_api::chain::ChainClient<types::BlockNumber<T>, types::BlockHash<T>, types::Header<T>, types::SignedBlock<T>>;

pub type StateClient<T> = sc_rpc_api::state::StateClient<types::BlockHash<T>>;

pub trait RpcExtension {
	fn raw_rpc(&mut self) -> RawClient {
		self.rpc()
	}

	fn rpc<TClient: From<RpcChannel> + 'static>(&mut self) -> TClient;
}

pub async fn connect_ws<TClient: From<RpcChannel>>(url: &str) -> Result<TClient, RpcError> {
	let url = url::Url::parse(url).expect("URL is valid");
	jsonrpc_core_client::transports::ws::connect(&url).compat().await
}
