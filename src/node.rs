use std::{
	cell::RefCell,
	sync::Arc,
};

use futures::{channel::mpsc, compat::Future01CompatExt, FutureExt};
use jsonrpc_core::MetaIoHandler;
use jsonrpc_core_client::{transports::local, RpcChannel};
use manual_seal::{run_manual_seal, ManualSealParams, consensus::ConsensusDataProvider};
use sc_cli::build_runtime;
use sc_client_api::{backend::Backend, ExecutorProvider, backend, CallExecutor};
use sc_executor::NativeExecutionDispatch;
use sc_service::{
	build_network, spawn_tasks, BuildNetworkParams, ChainSpec, Configuration, SpawnTasksParams, TFullBackend,
	TFullClient, TaskManager, TaskType, TFullCallExecutor,
};
use sc_transaction_pool::BasicPool;
use sp_api::{
	ApiErrorExt, ApiExt, ConstructRuntimeApi, Core, Metadata,
	TransactionFor, OverlayedChanges, StorageTransactionCache,
};
use sp_block_builder::BlockBuilder;
use sp_inherents::InherentDataProviders;
use sp_offchain::OffchainWorkerApi;
use sp_runtime::{traits::{Block as BlockT, SignedExtension}, generic::UncheckedExtrinsic, MultiSignature};
use sp_session::SessionKeys;
use sp_transaction_pool::runtime_api::TaggedTransactionQueue;
use sc_keystore::KeyStorePtr;
use sp_consensus::{BlockImport, SelectChain};
use sp_runtime::traits::{Extrinsic, NumberFor};
use parity_scale_codec::Encode;
use sp_state_machine::Ext;

use crate::rpc;

mod extensions;
pub mod utils;

pub use self::{
	extensions::ExtensionFactory,
	utils::{build_config, build_logger, StateProvider}
};
use sp_core::offchain::storage::OffchainOverlayedChanges;
use sp_core::ExecutionContext;
use sp_blockchain::HeaderBackend;
use sp_runtime::generic::BlockId;

/// This holds a reference to a running node on another thread,
/// the node process is dropped when this struct is dropped
/// also holds logs from the process.
pub struct InternalNode<Node: TestRuntimeRequirements> {
	/// rpc handler for communicating with the node over rpc.
	rpc_handler: Arc<MetaIoHandler<sc_rpc::Metadata, sc_rpc_server::RpcMiddleware>>,
	/// tokio-compat runtime
	_compat_runtime: RefCell<tokio_compat::runtime::Runtime>,
	/// Stream of log lines
	log_stream: futures::channel::mpsc::UnboundedReceiver<String>,
	/// node tokio runtime
	_runtime: tokio::runtime::Runtime,
	/// handle to the running node.
	_task_manager: Option<TaskManager>,
	/// client instance
	client: Arc<TFullClient<Node::Block, Node::RuntimeApi, Node::Executor>>,
	/// backend type.
	backend: Arc<TFullBackend<Node::Block>>,
}

impl<Node: TestRuntimeRequirements> InternalNode<Node> {
	/// Starts a node with the manual-seal authorship,
	pub fn new() -> Result<Self, sc_service::Error>
		where
			<Node::RuntimeApi as
				ConstructRuntimeApi<
					Node::Block,
					TFullClient<Node::Block, Node::RuntimeApi, Node::Executor>
				>
			>::RuntimeApi: Core<Node::Block> + Metadata<Node::Block>
				+ OffchainWorkerApi<Node::Block> + SessionKeys<Node::Block>
				+ TaggedTransactionQueue<Node::Block> + BlockBuilder<Node::Block>
				+ ApiErrorExt<Error=sp_blockchain::Error>
				+ ApiExt<
					Node::Block,
					StateBackend =
					<TFullBackend<Node::Block> as Backend<Node::Block>>::State,
				>,
	{
		let compat_runtime = tokio_compat::runtime::Runtime::new().unwrap();
		let tokio_runtime = build_runtime().unwrap();

		// unbounded logs, should be fine, test is shortlived.
		let (log_sink, log_stream) = futures::channel::mpsc::unbounded();

		build_logger(tokio_runtime.handle().clone(), log_sink);
		let runtime_handle = tokio_runtime.handle().clone();

		let task_executor = move |fut, task_type| match task_type {
			TaskType::Async => runtime_handle.spawn(fut).map(drop),
			TaskType::Blocking => runtime_handle
				.spawn_blocking(move || futures::executor::block_on(fut))
				.map(drop),
		};

		let config = build_config::<Node>(task_executor.into());

		let (
			client,
			backend,
			keystore,
			mut task_manager,
			inherent_data_providers,
			consensus_data_provider,
			select_chain,
			block_import,
		) = Node::create_client_parts(&config)?;

		client.execution_extensions().set_extensions_factory(Box::new(ExtensionFactory));

		let import_queue =
			manual_seal::import_queue(Box::new(block_import.clone()), &task_manager.spawn_handle(), None);

		let transaction_pool = BasicPool::new_full(
			config.transaction_pool.clone(),
			config.prometheus_registry(),
			task_manager.spawn_handle(),
			client.clone(),
		);

		let (network, network_status_sinks, system_rpc_tx, network_starter) = {
			let params = BuildNetworkParams {
				config: &config,
				client: client.clone(),
				transaction_pool: transaction_pool.clone(),
				spawn_handle: task_manager.spawn_handle(),
				import_queue,
				on_demand: None,
				block_announce_validator_builder: None,
				finality_proof_request_builder: None,
				finality_proof_provider: None,
			};
			build_network(params)?
		};

		// Proposer object for block authorship.
		let env = sc_basic_authorship::ProposerFactory::new(
			client.clone(),
			transaction_pool.clone(),
			config.prometheus_registry(),
		);

		// Channel for the rpc handler to communicate with the authorship task.
		let (command_sink, commands_stream) = futures::channel::mpsc::channel(10);

		let rpc_handlers = {
			let params = SpawnTasksParams {
				config,
				client: client.clone(),
				backend: backend.clone(),
				task_manager: &mut task_manager,
				keystore,
				on_demand: None,
				transaction_pool: transaction_pool.clone(),
				rpc_extensions_builder: Box::new(move |_, _| {
					use manual_seal::rpc;
					let mut io = jsonrpc_core::IoHandler::default();
					io.extend_with({
						// We provide the rpc handler with the sending end of the channel to allow the rpc
						// send EngineCommands to the background block authorship task.
						let handler = rpc::ManualSeal::<<Node::Block as BlockT>::Hash>::new(command_sink.clone());
						rpc::ManualSealApi::to_delegate(handler)
					});
					io
				}),
				remote_blockchain: None,
				network,
				network_status_sinks,
				system_rpc_tx,
				telemetry_connection_sinks: Default::default(),
			};
			spawn_tasks(params)?
		};

		// Background authorship future.
		let authorship_future = run_manual_seal(ManualSealParams {
			block_import,
			env,
			client: client.clone(),
			pool: transaction_pool.pool().clone(),
			commands_stream,
			select_chain,
			consensus_data_provider,
			inherent_data_providers,
		});

		// spawn the authorship task as an essential task.
		task_manager
			.spawn_essential_handle()
			.spawn("manual-seal", authorship_future);

		network_starter.start_network();
		let rpc_handler = rpc_handlers.io_handler();

		Ok(Self {
			rpc_handler,
			_task_manager: Some(task_manager),
			_runtime: tokio_runtime,
			_compat_runtime: RefCell::new(compat_runtime),
			client,
			backend,
			log_stream,
		})
	}

	/// returns a reference to the rpc handlers.
	pub fn rpc_handler(&self) -> Arc<MetaIoHandler<sc_rpc::Metadata, sc_rpc_server::RpcMiddleware>> {
		self.rpc_handler.clone()
	}

	/// send some extrinsic to the node, providing the sending account.
	pub fn send_extrinsic(
		&self,
		call: impl Into<<Node::Runtime as frame_system::Trait>::Call>,
		from: <Node::Runtime as frame_system::Trait>::AccountId,
	) -> Result<<Node::Runtime as frame_system::Trait>::Hash, jsonrpc_core_client::RpcError>
		where
			<Node::Runtime as frame_system::Trait>::AccountId: Encode,
			<Node::Runtime as frame_system::Trait>::Call: Encode,
	{
		let extra = Node::signed_extras::<Self>(&self, from.clone());
		let signed_data = Some((from, Default::default(), extra));
		let ext = UncheckedExtrinsic::<
			<Node::Runtime as frame_system::Trait>::AccountId,
			<Node::Runtime as frame_system::Trait>::Call,
			MultiSignature,
			Node::SignedExtension,
		>::new(call.into(), signed_data)
			.expect("UncheckedExtrinsic::new() always returns Some");
		let rpc_client = self.rpc_client::<rpc::AuthorClient<Node::Runtime>>();

		self.compat_runtime().borrow_mut()
			.block_on_std(async move {
				rpc_client.submit_extrinsic(ext.encode().into()).compat().await
			})
	}

	/// create a new jsonrpc client using the jsonrpc-core-client local transport
	pub fn rpc_client<C>(&self) -> C
	where
		C: From<RpcChannel> + 'static,
	{
		use futures01::Future;
		let rpc_handler = self.rpc_handler.clone();
		let (client, fut) = local::connect_with_middleware::<C, _, _, _>(rpc_handler);
		self._compat_runtime.borrow().spawn(fut.map_err(|_| ()));
		client
	}

	/// revert count number of blocks from the chain
	pub fn revert_blocks(&self, count: NumberFor<Node::Block>) -> Result<(), sp_blockchain::Error> {
		self.backend.revert(count, true)?;
		Ok(())
	}

	/// provides access to the tokio compat runtime.
	pub fn compat_runtime(&self) -> &RefCell<tokio_compat::runtime::Runtime> {
		&self._compat_runtime
	}

	/// provides access to the tokio runtime.
	pub fn tokio_runtime(&mut self) -> &mut tokio::runtime::Runtime {
		&mut self._runtime
	}

	pub(crate) fn log_stream(&mut self) -> &mut mpsc::UnboundedReceiver<String> {
		&mut self.log_stream
	}
}

impl<Node: TestRuntimeRequirements> Drop for InternalNode<Node> {
	fn drop(&mut self) {
		if let Some(mut task_manager) = self._task_manager.take() {
			// if this isn't called the node will live forever
			task_manager.terminate()
		}
	}
}

impl<Node: TestRuntimeRequirements> StateProvider for InternalNode<Node>
	where
		<TFullCallExecutor<Node::Block, Node::Executor> as CallExecutor<Node::Block>>::Error: std::fmt::Debug,
{
	fn with_state<R>(&self, closure: impl FnOnce() -> R) -> R {
		let id = BlockId::Hash(self.client.info().best_hash);
		let mut offchain_overlay = OffchainOverlayedChanges::disabled();
		let mut overlay = OverlayedChanges::default();
		let changes_trie = backend::changes_tries_state_at_block(
			&id, self.backend.changes_trie_storage()
		).unwrap();
		let mut cache = StorageTransactionCache::<
			Node::Block,
			<TFullBackend<Node::Block> as Backend<Node::Block>>::State
		>::default();
		let mut extensions = self.client.execution_extensions()
			.extensions(&id, ExecutionContext::BlockConstruction);
		let state_backend = self.backend.state_at(id).unwrap();
		let mut ext = Ext::new(
			&mut overlay,
			&mut offchain_overlay,
			&mut cache,
			&state_backend,
			changes_trie.clone(),
			Some(&mut extensions),
		);
		sp_externalities::set_and_run_with_externalities(&mut ext, closure)
	}
}

/// Wrapper trait for concrete type required by this testing framework.
pub trait TestRuntimeRequirements: Sized {
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
	type BlockImport: Send + Sync + Clone
		+ BlockImport<
			Self::Block,
			Error = sp_consensus::Error,
			Transaction = TransactionFor<
				TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>,
				Self::Block
			>
		> + 'static;

	type SignedExtension: SignedExtension;

	/// chain spec factory
	fn load_spec() -> Result<Box<dyn ChainSpec>, String>;

	/// provide a path to an existing db
	fn base_path() -> Option<&'static str> {
		None
	}

	/// Signed extras.
	fn signed_extras<S>(
		state: &S,
		from: <Self::Runtime as frame_system::Trait>::AccountId,
	) -> Self::SignedExtension
		where
			S: StateProvider;

	/// Attempt to create client parts, including blockimport,
	/// selectchain strategy and consensus data provider.
	fn create_client_parts(config: &Configuration) -> Result<
		(
			Arc<TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>>,
			Arc<TFullBackend<Self::Block>>,
			KeyStorePtr,
			TaskManager,
			InherentDataProviders,
			Option<Box<
				dyn ConsensusDataProvider<
					Self::Block,
					Transaction = TransactionFor<
						TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>,
						Self::Block
					>,
				>
			>>,
			Self::SelectChain,
			Self::BlockImport
		),
		sc_service::Error
	>;
}
