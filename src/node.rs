use std::io::Write;
use std::sync::Arc;

use futures::FutureExt;
use jsonrpc_core::MetaIoHandler;
use jsonrpc_core_client::{RpcChannel, transports::local};
use parking_lot::RwLock;
use sc_cli::{build_runtime, ChainSpecFactory};
use sc_client_api::{backend::Backend, execution_extensions::ExecutionStrategies};
use sc_executor::NativeExecutionDispatch;
use sc_informant::OutputFormat;
use sc_network::{config::TransportConfig, multiaddr};
use sc_service::{
	BasePath, build_network, BuildNetworkParams, Configuration, DatabaseConfig, new_full_parts,
	Role, spawn_tasks, SpawnTasksParams, TaskExecutor, TaskManager, TaskType, TFullBackend, TFullClient,
};
use sc_service::config::{KeystoreConfig, NetworkConfiguration, WasmExecutionMethod};
use sc_transaction_pool::BasicPool;
use sp_api::{ApiErrorExt, ApiExt, ConstructRuntimeApi, Core, Metadata};
use sp_block_builder::BlockBuilder;
use sp_inherents::InherentDataProviders;
use sp_keyring::Sr25519Keyring;
use sp_offchain::OffchainWorkerApi;
use sp_runtime::traits::Block as BlockT;
use sp_session::SessionKeys;
use sp_transaction_pool::runtime_api::TaggedTransactionQueue;
use std::marker::PhantomData;

type Module = String;
type Logger = Arc<RwLock<std::collections::HashMap<Module, Vec<String>>>>;

/// This holds a reference to a running node on another thread,
/// the node process is dropped when this struct is dropped
/// also holds logs from the process.
pub struct InternalNode<Node> {
	logger: Logger,

	/// rpc handler for communicating with the node over rpc.
	rpc_handlers: Arc<MetaIoHandler<sc_rpc::Metadata>>,

	/// tokio-compat runtime
	compat_runtime: tokio_compat::runtime::Runtime,

	/// node tokio runtime
	_runtime: tokio::runtime::Runtime,

	/// handle to the running node.
	_task_manager: Option<TaskManager>,

	_phantom: PhantomData<Node>,
}

impl<Node> InternalNode<Node> {
	/// Starts a node with the manual-seal authorship,
	pub fn new<SpecFactory>(spec_factory: SpecFactory) -> Result<Self, sc_service::Error>
		where
			Node: TestRuntimeRequirements,
			<Node::RuntimeApi as
				ConstructRuntimeApi<Node::OpaqueBlock, TFullClient<Node::OpaqueBlock, Node::RuntimeApi, Node::Executor>>
			>::RuntimeApi:
				Core<Node::OpaqueBlock> + Metadata<Node::OpaqueBlock> + OffchainWorkerApi<Node::OpaqueBlock>
				+ SessionKeys<Node::OpaqueBlock> + TaggedTransactionQueue<Node::OpaqueBlock>
				+ BlockBuilder<Node::OpaqueBlock> + ApiErrorExt<Error=sp_blockchain::Error>
				+ ApiExt<
					Node::OpaqueBlock,
					StateBackend= <TFullBackend<Node::OpaqueBlock> as Backend<Node::OpaqueBlock>>::State
				>,
			SpecFactory: ChainSpecFactory,
	{
		let logger = build_logger();

		let compat_runtime = tokio_compat::runtime::Runtime::new().unwrap();
		let tokio_runtime = build_runtime().unwrap();
		let runtime_handle = tokio_runtime.handle().clone();

		let task_executor = move |fut, task_type| match task_type {
			TaskType::Async => runtime_handle.spawn(fut).map(drop),
			TaskType::Blocking => runtime_handle
				.spawn_blocking(move || futures::executor::block_on(fut))
				.map(drop),
		};

		let config = build_config(spec_factory, task_executor.into());

		let (client, backend, keystore, mut task_manager) =
			new_full_parts::<Node::OpaqueBlock, Node::RuntimeApi, Node::Executor>(&config)?;
		let client = Arc::new(client);
		let import_queue = manual_seal::import_queue(Box::new(client.clone()), &task_manager.spawn_handle(), None);

		let transaction_pool = BasicPool::new_full(
			config.transaction_pool.clone(),
			config.prometheus_registry(),
			task_manager.spawn_handle(),
			client.clone(),
		);

		let (network, network_status_sinks, system_rpc_tx) = {
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
		let proposer = sc_basic_authorship::ProposerFactory::new(
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
				rpc_extensions_builder: Box::new(move |_| {
					use manual_seal::rpc;
					let mut io = jsonrpc_core::IoHandler::default();
					io.extend_with({
						// We provide the rpc handler with the sending end of the channel to allow the rpc
						// send EngineCommands to the background block authorship task.
						let handler = rpc::ManualSeal::<<Node::OpaqueBlock as BlockT>::Hash>::new(command_sink.clone());
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

		let inherent_data_providers = InherentDataProviders::new();
		inherent_data_providers
			.register_provider(sp_timestamp::InherentDataProvider)
			.expect("failed to register timestamp inherent");

		let select_chain = sc_consensus::LongestChain::new(backend.clone());

		// Background authorship future.
		let authorship_future = manual_seal::run_manual_seal(
			Box::new(client.clone()),
			proposer,
			client,
			transaction_pool.pool().clone(),
			commands_stream,
			select_chain,
			inherent_data_providers,
		);

		// spawn the authorship task as an essential task.
		task_manager
			.spawn_essential_handle()
			.spawn("manual-seal", authorship_future);

		Ok(Self {
			rpc_handlers: rpc_handlers.io_handler(),
			_task_manager: Some(task_manager),
			_phantom: PhantomData,
			_runtime: tokio_runtime,
			compat_runtime,
			logger,
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
	pub fn tokio_runtime(&mut self) -> &mut tokio_compat::runtime::Runtime {
		&mut self.compat_runtime
	}

	pub(crate) fn logs(&self) -> &Logger {
		&self.logger
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
	type OpaqueBlock: BlockT;
	/// Executor type
	type Executor: NativeExecutionDispatch + 'static;
	/// Runtime
	type Runtime: frame_system::Trait;
	/// RuntimeApi
	type RuntimeApi: Send + Sync + 'static
		+ ConstructRuntimeApi<Self::OpaqueBlock, TFullClient<Self::OpaqueBlock, Self::RuntimeApi, Self::Executor>>;
}

/// Used to create `Configuration` object for the node.
fn build_config<SpecFactory>(spec_factory: SpecFactory, task_executor: TaskExecutor) -> Configuration
	where
		SpecFactory: ChainSpecFactory
{
	let base_path = BasePath::new_temp_dir().expect("could not create temporary directory");
	let root = base_path.path();
	let role = Role::Authority {
		sentry_nodes: Vec::new(),
	};
	let key_seed = Sr25519Keyring::Alice.to_seed();
	let mut chain_spec = spec_factory
		.load_spec("dev".into())
		.expect("failed to load chain specification");
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
			path: root.join("key"),
			password: None,
		},
		database: DatabaseConfig::RocksDb {
			path: root.join("db"),
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
fn build_logger() -> Logger {
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
	let logs = Logger::default();
	{
		let logs = logs.clone();
		let mut builder = env_logger::builder();
		builder.format(move |buf: &mut env_logger::fmt::Formatter, record: &log::Record| {
			let entry = format!("{} {} {}", record.level(), record.target(), record.args());
			let res = writeln!(buf, "{}", entry);
			logs.write().entry(record.target().to_string()).or_default().push(entry);
			res
		});
		builder.write_style(env_logger::WriteStyle::Always);
		builder.filter_level(log::LevelFilter::Debug);
		builder.filter_module("runtime", log::LevelFilter::Trace);
		for module in &ignore {
			builder.filter_module(module, log::LevelFilter::Info);
		}
		let _ = builder.is_test(true).try_init();
	}
	logs
}
