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
use std::ops::{Deref, DerefMut};

/// A deterministic internal instance of substrate node.
pub struct Deterministic<Runtime: frame_system::Trait> {
	node: InternalNode<Runtime>,
	externalities: TestExternalities<Runtime>,
}

impl<Runtime: frame_system::Trait> rpc::RpcExtension for Deterministic<Runtime> {
	fn rpc<TClient: From<RpcChannel> + 'static>(&mut self) -> TClient {
		self.node.rpc_client()
	}
}

impl<Runtime: frame_system::Trait> Deterministic<Runtime> {
	pub fn new(node: InternalNode<Runtime>) -> Self {
		let mut externalities = TestExternalities::<Runtime>::new(node.rpc_client());

		(&mut externalities as &mut dyn Externalities)
			.register_extension(TransactionPoolExt::new(TxPoolExtApi::<Runtime>::new(node.rpc_client())))
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

impl<Runtime: frame_system::Trait + Send + Sync> Deterministic<Runtime> {
	pub fn assert_log_line(&self, module: &str, content: &str) {
		if let Some(logs) = self.node.logs().read().get(module) {
			for log in logs {
				if log.contains(content) {
					return;
				}
			}
			panic!("Could not find {} in logs: {:?}", content, logs)
		} else {
			panic!("No logs from {} module.", module);
		}
	}

	pub fn produce_blocks(&mut self, num: usize) {
		log::info!("produce blocks");

		let client = self.rpc::<ManualSealClient<Runtime::Hash>>();
		log::info!("produce blocks");

		for _ in 0..num {
			self.node
				.tokio_runtime()
				.block_on(client.create_block(true, true, None))
				.expect("block production failed: ");
		}
		log::info!("sealed {} blocks", num)
	}

	/// Execute closure in an externalities provided environment.
	pub fn with_state<R>(&mut self, closure: impl FnOnce() -> R) -> R {
		self.externalities.execute_with(closure)
	}
}

impl<T: frame_system::Trait> Deref for Deterministic<T> {
	type Target = InternalNode<T>;

	fn deref(&self) -> &Self::Target {
		&self.node
	}
}

impl<T: frame_system::Trait> DerefMut for Deterministic<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.node
	}
}
