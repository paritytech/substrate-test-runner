use std::{cell::RefCell, sync::Arc};

use futures::{FutureExt, SinkExt};
use jsonrpc_core::MetaIoHandler;
use jsonrpc_core_client::{transports::local, RpcChannel};
use manual_seal::{run_manual_seal, EngineCommand, ManualSealParams};
use parity_scale_codec::Encode;
use sc_cli::build_runtime;
use sc_client_api::{backend, backend::Backend, CallExecutor, ExecutorProvider};
use sc_service::{
	build_network, spawn_tasks, BuildNetworkParams, SpawnTasksParams, TFullBackend, TFullCallExecutor, TFullClient,
	TaskManager, TaskType,
};
use sc_transaction_pool::BasicPool;
use sp_api::{ApiErrorExt, ApiExt, ConstructRuntimeApi, Core, Metadata, OverlayedChanges, StorageTransactionCache};
use sp_block_builder::BlockBuilder;
use sp_blockchain::HeaderBackend;
use sp_core::{offchain::storage::OffchainOverlayedChanges, ExecutionContext};
use sp_offchain::OffchainWorkerApi;
use sp_runtime::traits::{Block as BlockT, Extrinsic};
use sp_runtime::{generic::BlockId, transaction_validity::TransactionSource, MultiSignature};
use sp_runtime::{generic::UncheckedExtrinsic, traits::NumberFor};
use sp_session::SessionKeys;
use sp_state_machine::Ext;
use sp_transaction_pool::runtime_api::TaggedTransactionQueue;
use sp_transaction_pool::TransactionPool;

pub use crate::utils::{config, logger};
use crate::TestRequirements;

/// This holds a reference to a running node on another thread,
/// the node process is dropped when this struct is dropped
/// also holds logs from the process.
pub struct Node<T: TestRequirements> {
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
	client: Arc<TFullClient<T::Block, T::RuntimeApi, T::Executor>>,
	/// transaction pool
	pool: Arc<
		dyn TransactionPool<
			Block = T::Block,
			Hash = <T::Block as BlockT>::Hash,
			Error = sc_transaction_pool::error::Error,
			InPoolTransaction = sc_transaction_graph::base_pool::Transaction<
				<T::Block as BlockT>::Hash,
				<T::Block as BlockT>::Extrinsic,
			>,
		>,
	>,
	/// channel to communicate with manual seal on.
	manual_seal_command_sink: futures::channel::mpsc::Sender<EngineCommand<<T::Block as BlockT>::Hash>>,
	/// backend type.
	backend: Arc<TFullBackend<T::Block>>,
	/// Block number at initialization of this Node.
	initial_block_number: NumberFor<T::Block>
}

impl<T: TestRequirements> Node<T> {
	/// Starts a node with the manual-seal authorship,
	pub fn new() -> Result<Self, sc_service::Error>
	where
		<T::RuntimeApi as ConstructRuntimeApi<T::Block, TFullClient<T::Block, T::RuntimeApi, T::Executor>>>::RuntimeApi:
			Core<T::Block>
				+ Metadata<T::Block>
				+ OffchainWorkerApi<T::Block>
				+ SessionKeys<T::Block>
				+ TaggedTransactionQueue<T::Block>
				+ BlockBuilder<T::Block>
				+ ApiErrorExt<Error = sp_blockchain::Error>
				+ ApiExt<T::Block, StateBackend = <TFullBackend<T::Block> as Backend<T::Block>>::State>,
	{
		let compat_runtime = tokio_compat::runtime::Runtime::new().unwrap();
		let tokio_runtime = build_runtime().unwrap();

		// unbounded logs, should be fine, test is shortlived.
		let (log_sink, log_stream) = futures::channel::mpsc::unbounded();

		logger(tokio_runtime.handle().clone(), log_sink);
		let runtime_handle = tokio_runtime.handle().clone();

		let task_executor = move |fut, task_type| match task_type {
			TaskType::Async => runtime_handle.spawn(fut).map(drop),
			TaskType::Blocking => runtime_handle
				.spawn_blocking(move || futures::executor::block_on(fut))
				.map(drop),
		};

		let config = config::<T>(task_executor.into());

		let (
			client,
			backend,
			keystore,
			mut task_manager,
			inherent_data_providers,
			consensus_data_provider,
			select_chain,
			block_import,
		) = T::create_client_parts(&config)?;

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
			task_manager.spawn_handle(),
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
				rpc_extensions_builder: Box::new(move |_, _| jsonrpc_core::IoHandler::default()),
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
		let initial_number = client.info().best_number;

		Ok(Self {
			rpc_handler,
			_task_manager: Some(task_manager),
			_runtime: tokio_runtime,
			_compat_runtime: RefCell::new(compat_runtime),
			client,
			pool: transaction_pool,
			backend,
			log_stream,
			manual_seal_command_sink: command_sink,
			initial_block_number: initial_number,
		})
	}

	/// Returns a reference to the rpc handlers.
	pub fn rpc_handler(&self) -> Arc<MetaIoHandler<sc_rpc::Metadata, sc_rpc_server::RpcMiddleware>> {
		self.rpc_handler.clone()
	}

	/// Return a reference to the Client
	pub fn client(&self) -> Arc<TFullClient<T::Block, T::RuntimeApi, T::Executor>> {
		self.client.clone()
	}

	/// Executes closure in an externalities provided environment.
	pub fn with_state<R>(&self, closure: impl FnOnce() -> R) -> R
	where
		<TFullCallExecutor<T::Block, T::Executor> as CallExecutor<T::Block>>::Error: std::fmt::Debug,
	{
		let id = BlockId::Hash(self.client.info().best_hash);
		let mut offchain_overlay = OffchainOverlayedChanges::disabled();
		let mut overlay = OverlayedChanges::default();
		let changes_trie = backend::changes_tries_state_at_block(&id, self.backend.changes_trie_storage()).unwrap();
		let mut cache =
			StorageTransactionCache::<T::Block, <TFullBackend<T::Block> as Backend<T::Block>>::State>::default();
		let mut extensions = self
			.client
			.execution_extensions()
			.extensions(&id, ExecutionContext::BlockConstruction);
		let state_backend = self
			.backend
			.state_at(id.clone())
			.expect(&format!("State at block {} not found", id));

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

	/// submit some extrinsic to the node, providing the sending account.
	pub fn submit_extrinsic(
		&self,
		call: impl Into<<T::Runtime as frame_system::Trait>::Call>,
		from: <T::Runtime as frame_system::Trait>::AccountId,
	) -> <T::Block as BlockT>::Hash
	where
		<T::Runtime as frame_system::Trait>::AccountId: Encode,
		<T::Runtime as frame_system::Trait>::Call: Encode,
		<T::Block as BlockT>::Extrinsic: From<
			UncheckedExtrinsic<
				<T::Runtime as frame_system::Trait>::AccountId,
				<T::Runtime as frame_system::Trait>::Call,
				MultiSignature,
				T::SignedExtras,
			>,
		>,
	{
		let extra = self.with_state(|| T::signed_extras(from.clone()));
		let signed_data = Some((from, Default::default(), extra));
		let ext = UncheckedExtrinsic::<
			<T::Runtime as frame_system::Trait>::AccountId,
			<T::Runtime as frame_system::Trait>::Call,
			MultiSignature,
			T::SignedExtras,
		>::new(call.into(), signed_data)
		.expect("UncheckedExtrinsic::new() always returns Some");
		let at = self.client.info().best_hash;

		self.compat_runtime()
			.borrow_mut()
			.block_on_std(
				self.pool
					.submit_one(&BlockId::Hash(at), TransactionSource::Local, ext.into()),
			)
			.unwrap()
	}

	/// Checks the node logs for a specific entry.
	pub fn assert_log_line(&mut self, content: &str) {
		futures::executor::block_on(async {
			use futures::StreamExt;

			while let Some(log_line) = self.log_stream.next().await {
				if log_line.contains(content) {
					return;
				}
			}

			panic!("Could not find {} in logs content", content);
		});
	}

	/// Instructs manual seal to seal new, possibly empty blocks.
	pub fn seal_blocks(&mut self, num: usize) {
		let mut tokio = self._compat_runtime.borrow_mut();
		for count in 0..num {
			let future = self.manual_seal_command_sink.send(EngineCommand::SealNewBlock {
				create_empty: true,
				finalize: false,
				parent_hash: None,
				sender: None,
			});

			tokio.block_on_std(future).expect("block production failed: ");
			log::info!("sealed {} of {} blocks", count + 1, num)
		}
	}

	/// Create a new jsonrpc client using the jsonrpc-core-client local transport
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

	/// Revert count number of blocks from the chain
	pub fn revert_blocks(&self, count: NumberFor<T::Block>) {
		self.backend.revert(count, true).expect("Failed to revert blocks: ");
	}

	/// provides access to the tokio compat runtime.
	pub fn compat_runtime(&self) -> &RefCell<tokio_compat::runtime::Runtime> {
		&self._compat_runtime
	}

	/// provides access to the tokio runtime.
	pub fn tokio_runtime(&mut self) -> &mut tokio::runtime::Runtime {
		&mut self._runtime
	}
}

impl<T: TestRequirements> Drop for Node<T> {
	fn drop(&mut self) {
        // if a db path was specified, revert all blocks we've added
		if let Some(_) = T::base_path() {
			let diff = self.client.info().best_number - self.initial_block_number;
			self.revert_blocks(diff);
		}
		if let Some(mut task_manager) = self._task_manager.take() {
			// if this isn't called the node will live forever
			task_manager.terminate()
		}
	}
}
