use crate::node::TestRuntimeRequirements;
use crate::test::externalities::TestExternalities;
use crate::{
	node::InternalNode,
	rpc::{self, RpcExtension},
};
use jsonrpc_core_client::RpcChannel;
use manual_seal::rpc::ManualSealClient;
use sp_externalities::{Externalities, ExternalitiesExt};
use sp_keyring::sr25519::Keyring;
use sp_runtime::traits::Block as BlockT;
use std::ops::{Deref, DerefMut};

/// A deterministic internal instance of substrate node.
pub struct Deterministic<Node: TestRuntimeRequirements> {
	node: InternalNode<Node>,
}

impl<Node: TestRuntimeRequirements> rpc::RpcExtension for Deterministic<Node> {
	fn rpc<TClient: From<RpcChannel> + 'static>(&mut self) -> TClient {
		self.node.rpc_client()
	}
}

impl<Node: TestRuntimeRequirements + 'static> Deterministic<Node> {
	pub fn new(node: InternalNode<Node>) -> Self {
		Self { node }
	}
}

impl<Node: TestRuntimeRequirements> Deterministic<Node> {
	pub fn assert_log_line(&mut self, content: &str) {
		futures::executor::block_on(async {
			use futures::StreamExt;

			while let Some(log_line) = self.node.log_stream().next().await {
				if log_line.contains(content) {
					return;
				}
			}

			panic!("Could not find {} in logs content", content);
		});
	}

	pub fn produce_blocks(&mut self, num: usize) {
		let client = self.rpc::<ManualSealClient<<Node::Block as BlockT>::Hash>>();

		for _ in 0..num {
			self.node.compat_runtime()
				.borrow_mut()
				.block_on(client.create_block(true, false, None))
				.expect("block production failed: ");
		}

		log::info!("sealed {} blocks", num)
	}
}



impl<T: TestRuntimeRequirements> Deref for Deterministic<T> {
	type Target = InternalNode<T>;

	fn deref(&self) -> &Self::Target {
		&self.node
	}
}

impl<T: TestRuntimeRequirements> DerefMut for Deterministic<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.node
	}
}
