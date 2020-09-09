use std::io::Write;
use std::marker::PhantomData;
use std::sync::Arc;
use std::fmt;

use futures::{channel::mpsc, FutureExt, Sink, SinkExt};
use jsonrpc_core::MetaIoHandler;
use jsonrpc_core_client::{transports::local, RpcChannel};
use manual_seal::{
	run_manual_seal, ManualSealParams, consensus::ConsensusDataProvider,
};
use sc_cli::build_runtime;
use sc_client_api::{backend::Backend, execution_extensions::ExecutionStrategies};
use sc_executor::NativeExecutionDispatch;
use sc_informant::OutputFormat;
use sc_network::{config::TransportConfig, multiaddr};
use sc_service::{
	build_network,
	config::{KeystoreConfig, NetworkConfiguration, WasmExecutionMethod},
	spawn_tasks, BasePath, BuildNetworkParams, ChainSpec, Configuration, DatabaseConfig, Role,
	SpawnTasksParams, TFullBackend, TFullClient, TaskExecutor, TaskManager, TaskType,
};
use sc_transaction_pool::BasicPool;
use sp_api::{ApiErrorExt, ApiExt, ConstructRuntimeApi, Core, Metadata, TransactionFor};
use sp_block_builder::BlockBuilder;
use sp_inherents::InherentDataProviders;
use sp_keyring::Sr25519Keyring;
use sp_offchain::OffchainWorkerApi;
use sp_runtime::traits::Block as BlockT;
use sp_session::SessionKeys;
use sp_transaction_pool::runtime_api::TaggedTransactionQueue;
use sc_keystore::KeyStorePtr;
use sp_consensus::{BlockImport, SelectChain};

/// This holds a reference to a running node on another thread,
/// the node process is dropped when this struct is dropped
/// also holds logs from the process.
pub struct InternalNode<Node> {
	/// rpc handler for communicating with the node over rpc.
	rpc_handlers: Arc<MetaIoHandler<sc_rpc::Metadata>>,

	/// tokio-compat runtime
	compat_runtime: tokio_compat::runtime::Runtime,

	///Stream of log lines
	log_stream: futures::channel::mpsc::UnboundedReceiver<String>,

	/// node tokio runtime
	_runtime: tokio::runtime::Runtime,

	/// handle to the running node.
	_task_manager: Option<TaskManager>,

	_phantom: PhantomData<Node>,
}

impl<Node> InternalNode<Node> {
	/// Starts a node with the manual-seal authorship,
	pub fn new() -> Result<Self, sc_service::Error>
		where
			Node: TestRuntimeRequirements,
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
			client,
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

		Ok(Self {
			rpc_handlers: rpc_handlers.io_handler(),
			_task_manager: Some(task_manager),
			_phantom: PhantomData,
			_runtime: tokio_runtime,
			compat_runtime,
			log_stream,
		})
	}

	/// returns a reference to the rpc handlers.
	pub fn rpc_handler(&self) -> Arc<MetaIoHandler<sc_rpc::Metadata>> {
		self.rpc_handlers.clone()
	}

	/// create a new jsonrpc client using the jsonrpc-core-client local transport
	pub fn rpc_client<C>(&self) -> C
	where
		C: From<RpcChannel> + 'static,
	{
		use futures01::Future;
		let rpc_handler = self.rpc_handlers.clone();
		let (client, fut) = local::connect::<C, _, _>(rpc_handler);
		self.compat_runtime.spawn(fut.map_err(|_| ()));
		client
	}

	/// provides access to the tokio compat runtime.
	pub fn compat_runtime(&mut self) -> &mut tokio_compat::runtime::Runtime {
		&mut self.compat_runtime
	}

	/// provides access to the tokio runtime.
	pub fn tokio_runtime(&mut self) -> &mut tokio::runtime::Runtime {
		&mut self._runtime
	}

	pub(crate) fn log_stream(&mut self) -> &mut mpsc::UnboundedReceiver<String> {
		&mut self.log_stream
	}
}

impl<Node> Drop for InternalNode<Node> {
	fn drop(&mut self) {
		if let Some(mut task_manager) = self._task_manager.take() {
			// if this isn't called the node will live forever
			task_manager.terminate()
		}
	}
}

/// Wrapper trait for concrete type required by this testing framework.
pub trait TestRuntimeRequirements {
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

	/// chain spec factory
	fn load_spec() -> Result<Box<dyn ChainSpec>, String>;

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

/// Used to create `Configuration` object for the node.
fn build_config<Node>(task_executor: TaskExecutor) -> Configuration
where
	Node: TestRuntimeRequirements,
{
	let base_path =BasePath::from_project("", "", "polkadot");
	let root_path = base_path.path()
		.to_path_buf()
		.join("chains")
		.join("polkadot");
	let role = Role::Authority {
		sentry_nodes: Vec::new(),
	};
	let key_seed = Sr25519Keyring::Alice.to_seed();
	let mut chain_spec = Node::load_spec().expect("failed to load chain specification");
	let storage = chain_spec
		.as_storage_builder()
		.build_storage()
		.expect("could not build storage");

	chain_spec.set_storage(storage);

	let mut network_config = NetworkConfiguration::new(
		format!("Polkadot Test Node for: {}", key_seed),
		"network/test/0.1",
		Default::default(),
		None,
	);
	let informant_output_format = OutputFormat {
		enable_color: false,
		prefix: format!("[{}] ", key_seed),
	};

	network_config.allow_non_globals_in_dht = true;

	network_config
		.listen_addresses
		.push(multiaddr::Protocol::Memory(rand::random()).into());

	network_config.transport = TransportConfig::MemoryOnly;

	Configuration {
		impl_name: "polkadot-test-node".to_string(),
		impl_version: "0.1".to_string(),
		role,
		task_executor,
		transaction_pool: Default::default(),
		network: network_config,
		keystore: KeystoreConfig::Path {
			path: root_path.join("key"),
			password: None,
		},
		database: DatabaseConfig::RocksDb {
			path: root_path.join("db"),
			cache_size: 128,
		},
		state_cache_size: 16777216,
		state_cache_child_ratio: None,
		pruning: Default::default(),
		chain_spec,
		wasm_method: WasmExecutionMethod::Interpreted,
		// NOTE: we enforce the use of the native runtime to make the errors more debuggable
		execution_strategies: ExecutionStrategies {
			syncing: sc_client_api::ExecutionStrategy::NativeWhenPossible,
			importing: sc_client_api::ExecutionStrategy::NativeWhenPossible,
			block_construction: sc_client_api::ExecutionStrategy::NativeWhenPossible,
			offchain_worker: sc_client_api::ExecutionStrategy::NativeWhenPossible,
			other: sc_client_api::ExecutionStrategy::NativeWhenPossible,
		},
		rpc_http: None,
		rpc_ws: None,
		rpc_ipc: None,
		rpc_ws_max_connections: None,
		rpc_cors: None,
		rpc_methods: Default::default(),
		prometheus_config: None,
		telemetry_endpoints: None,
		telemetry_external_transport: None,
		default_heap_pages: None,
		offchain_worker: Default::default(),
		force_authoring: false,
		disable_grandpa: false,
		dev_key_seed: Some(key_seed),
		tracing_targets: None,
		tracing_receiver: Default::default(),
		max_runtime_instances: 8,
		announce_block: true,
		base_path: Some(base_path),
		informant_output_format,
	}
}

/// Builds the global logger.
fn build_logger<LogSink>(executor: tokio::runtime::Handle, log_sink: LogSink)
	where
		LogSink: Sink<String> + Clone + Unpin + Send + Sync + 'static,
		LogSink::Error: Send + Sync + fmt::Debug,
{
	let ignore = [
		"yamux",
		"multistream_select",
		"libp2p",
		"jsonrpc_client_transports",
		"sc_network",
		"tokio_reactor",
		"sub-libp2p",
		"sync",
		"peerset",
		"ws",
		"sc_network",
		"sc_service",
		"sc_peerset",
		"rpc",
	];
	let mut builder = env_logger::builder();
	builder.format(move |buf: &mut env_logger::fmt::Formatter, record: &log::Record| {
		let entry = format!("{} {} {}", record.level(), record.target(), record.args());
		let res = writeln!(buf, "{}", entry);
		
		let mut log_sink_clone = log_sink.clone();
		let _ = executor.spawn(async move {
			log_sink_clone.send(entry).await
				.expect("log_stream is dropped");
		});
		res
	});
	builder.write_style(env_logger::WriteStyle::Always);
	builder.filter_level(log::LevelFilter::Debug);
	builder.filter_module("runtime", log::LevelFilter::Trace);
	builder.filter_module("sc_service", log::LevelFilter::Trace);
	for module in &ignore {
		builder.filter_module(module, log::LevelFilter::Info);
	}
	let _ = builder.is_test(true).try_init();
}
