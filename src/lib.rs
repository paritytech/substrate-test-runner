#![deny(unused_crate_dependencies)]
use sc_executor::NativeExecutionDispatch;
use sp_api::{ConstructRuntimeApi, TransactionFor};
use sc_service::{TFullClient, ChainSpec, Configuration, TFullBackend, TaskManager};
use sp_consensus::{SelectChain, BlockImport};
use sp_runtime::traits::{SignedExtension, Block as BlockT};
use std::sync::Arc;
use sp_keystore::SyncCryptoStorePtr;
use manual_seal::consensus::ConsensusDataProvider;
use sp_inherents::InherentDataProviders;

mod node;
mod utils;

pub use node::*;

/// Wrapper trait for concrete type required by this testing framework.
pub trait TestRequirements: Sized {
	/// Opaque block type
	type Block: BlockT;

	/// Executor type
	type Executor: NativeExecutionDispatch + 'static;

	/// Runtime
	type Runtime: frame_system::Trait;

	/// RuntimeApi
	type RuntimeApi: Send
	+ Sync
	+ 'static
	+ ConstructRuntimeApi<Self::Block, TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>>;

	/// select chain type.
	type SelectChain: SelectChain<Self::Block> + 'static;

	/// Block import type.
	type BlockImport: Send
	+ Sync
	+ Clone
	+ BlockImport<
		Self::Block,
		Error = sp_consensus::Error,
		Transaction = TransactionFor<TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>, Self::Block>,
	> + 'static;

	type SignedExtras: SignedExtension;

	/// chain spec factory
	fn load_spec() -> Result<Box<dyn ChainSpec>, String>;

	/// provide a path to an existing db
	fn base_path() -> Option<&'static str> {
		None
	}

	/// Signed extras, this function is caled in an externalities provided environment.
	fn signed_extras(from: <Self::Runtime as frame_system::Trait>::AccountId) -> Self::SignedExtras;

	/// Attempt to create client parts, including block import,
	/// select chain strategy and consensus data provider.
	fn create_client_parts(
		config: &Configuration,
	) -> Result<
		(
			Arc<TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>>,
			Arc<TFullBackend<Self::Block>>,
			SyncCryptoStorePtr,
			TaskManager,
			InherentDataProviders,
			Option<
				Box<
					dyn ConsensusDataProvider<
						Self::Block,
						Transaction = TransactionFor<
							TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>,
							Self::Block,
						>,
					>,
				>,
			>,
			Self::SelectChain,
			Self::BlockImport,
		),
		sc_service::Error,
	>;
}
