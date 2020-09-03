use crate::node::TestRuntimeRequirements;
use crate::test::externalities::{TestExternalities, TxPoolExtApi};
use crate::{
	node::InternalNode,
	rpc::{self, RpcExtension},
};
use jsonrpc_core_client::RpcChannel;
use manual_seal::rpc::ManualSealClient;
use sp_core::{
	offchain::TransactionPoolExt,
	testing::{KeyStore, SR25519},
	traits::KeystoreExt,
};
use sp_externalities::{Externalities, ExternalitiesExt};
use sp_keyring::sr25519::Keyring;
use sp_runtime::traits::Block as BlockT;
use std::ops::{Deref, DerefMut};

/// A deterministic internal instance of substrate node.
pub struct Deterministic<Node: TestRuntimeRequirements> {
	node: InternalNode<Node>,
	externalities: TestExternalities<Node>,
}

impl<Node: TestRuntimeRequirements> rpc::RpcExtension for Deterministic<Node> {
	fn rpc<TClient: From<RpcChannel> + 'static>(&mut self) -> TClient {
		self.node.rpc_client()
	}
}

impl<Node: TestRuntimeRequirements + 'static> Deterministic<Node> {
	pub fn new(node: InternalNode<Node>) -> Self {
		let mut externalities = TestExternalities::<Node>::new(node.rpc_client());

		(&mut externalities as &mut dyn Externalities)
			.register_extension(TransactionPoolExt::new(TxPoolExtApi::<Node>::new(node.rpc_client())))
			.expect("Failed to transaction register");

		let keystore = KeyStore::new();

		// here we load all the test keys (which have initial balances)
		// in the keyring into the keystore
		for key in Keyring::iter() {
			keystore
				.write()
				.sr25519_generate_new(SR25519, Some(&format!("//{}", key)))
				.expect("failed to add key to keystore: ");
		}

		(&mut externalities as &mut dyn Externalities)
			.register_extension(KeystoreExt(keystore))
			.expect("Failed to transaction register");

		Self { node, externalities }
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
			self.node
				.compat_runtime()
				.block_on(client.create_block(true, false, None))
				.expect("block production failed: ");
		}

		log::info!("sealed {} blocks", num)
	}

	/// Execute closure in an externalities provided environment.
	pub fn with_state<R>(&mut self, closure: impl FnOnce() -> R) -> R {
		self.externalities.execute_with(closure)
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
