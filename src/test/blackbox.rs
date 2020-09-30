use crate::{
	rpc::{self, RpcExtension},
	types, node::InternalNode,
	test::externalities::TestExternalities
};
use jsonrpc_core_client::{transports::local, RpcChannel};
use crate::node::TestRuntimeRequirements;

/// A black box node, either runs a background node,
/// or connects via ws to a running node.
pub enum BlackBoxNode<N: TestRuntimeRequirements> {
	/// Connects to an external node.
	External(String),
	/// Spawns a pristine node.
	Internal(InternalNode<N>),
}

/// A black box test.
pub struct BlackBox<N: TestRuntimeRequirements> {
	node: BlackBoxNode<N>,
}

impl<N> BlackBox<N>
	where
		N: TestRuntimeRequirements,
{
	/// Execute provided `Fn` in an externalities provided environment.
	pub async fn with_state<T>(&mut self, func: impl FnOnce() -> T) -> T {
		TestExternalities::<N>::new(self.rpc()).execute_with(func)
	}
	
	/// Wait `number` of blocks.
	pub fn wait_blocks(&self, _number: impl Into<types::BlockNumber<N::Runtime>>) {
		// TODO: no-op
	}
}

impl<N: TestRuntimeRequirements> rpc::RpcExtension for BlackBox<N> {
	fn rpc<TClient: From<RpcChannel> + 'static>(&mut self) -> TClient {
		let client = match self.node {
			BlackBoxNode::External(ref url) => futures::executor::block_on(rpc::connect_ws(&url)).unwrap(),
			BlackBoxNode::Internal(ref mut node) => {
				use futures01::Future;
				let (client, fut) = local::connect_with_middleware::<TClient, _, _, _>(node.rpc_handler());
				node.compat_runtime().borrow().spawn(fut.map_err(|_| ()));

				client
			}
		};
		client
	}
}

impl<N: TestRuntimeRequirements> BlackBox<N>{
	/// Create an instance of `BlackBox`.
	pub fn new(node: BlackBoxNode<N>) -> Self {
		Self { node }
	}
}
