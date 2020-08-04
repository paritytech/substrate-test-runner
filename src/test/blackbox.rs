use crate::test::externalities::TestExternalities;
use crate::{
	node::InternalNode,
	rpc::{self, RpcExtension},
	types,
};
use jsonrpc_core_client::{transports::local, RpcChannel};

pub enum BlackBoxNode<Runtime> {
	/// Connects to an external node.
	External(String),
	/// Spawns a pristine node.
	Internal(InternalNode<Runtime>),
}

/// A black box test.
pub struct BlackBox<Runtime> {
	node: BlackBoxNode<Runtime>,
}

impl<Runtime> BlackBox<Runtime>
where
	Runtime: frame_system::Trait,
{
	pub async fn with_state<R>(&mut self, closure: impl FnOnce() -> R) -> R {
		TestExternalities::<Runtime>::new(self.rpc()).execute_with(closure)
	}
}

impl<Runtime> rpc::RpcExtension for BlackBox<Runtime> {
	fn rpc<TClient: From<RpcChannel> + 'static>(&mut self) -> TClient {
		let client = match self.node {
			BlackBoxNode::External(ref url) => futures::executor::block_on(rpc::connect_ws(&url)).unwrap(),
			BlackBoxNode::Internal(ref mut node) => {
				use futures01::Future;
				let (client, fut) = local::connect::<TClient, _, _>(node.rpc_handler());
				node.tokio_runtime().spawn(fut.map_err(|_| ()));

				client
			}
		};
		client
	}
}

impl<Runtime> BlackBox<Runtime> {
	pub fn new(node: BlackBoxNode<Runtime>) -> Self {
		Self { node }
	}
}

impl<Runtime: frame_system::Trait> BlackBox<Runtime> {
	/// Wait `number` of blocks.
	pub fn wait_blocks(&self, _number: impl Into<types::BlockNumber<Runtime>>) {
		todo!()
	}
}
