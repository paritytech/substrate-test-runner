use futures::FutureExt;
use jsonrpc_core::MetaIoHandler;
use jsonrpc_core_client::{transports::local, RpcChannel};
use parking_lot::RwLock;
use sc_cli::{build_runtime, SubstrateCli, ChainSpecFactory};
use sc_executor::NativeExecutionDispatch;
use sc_service::{
	build_network, new_full_parts, spawn_tasks, BuildNetworkParams,	SpawnTasksParams,
	TaskExecutor, TaskManager, TaskType, TFullClient, TFullBackend,
};
use sc_transaction_pool::BasicPool;
use sp_inherents::InherentDataProviders;
use sp_runtime::traits::Block as BlockT;
use sp_transaction_pool::runtime_api::TaggedTransactionQueue;
use sp_offchain::OffchainWorkerApi;
use sp_session::SessionKeys;
use sp_block_builder::BlockBuilder;
use sc_client_api::backend::Backend;
use sp_api::{ConstructRuntimeApi, ApiErrorExt, Core, Metadata, ApiExt};
use std::io::Write;
use std::sync::Arc;
use crate::cli::Cli;

type Module = String;
type Logger = Arc<RwLock<std::collections::HashMap<Module, Vec<String>>>>;

/// this holds a reference to a running node on another thread,
/// we set a port over cli, process is dropped when this struct is dropped
/// holds logs from the process.
pub struct InternalNode {
	logger: Logger,

	/// rpc handler for communicating with the node over rpc.
	rpc_handlers: Arc<MetaIoHandler<sc_rpc::Metadata>>,

	/// tokio-compat runtime
	compat_runtime: tokio_compat::runtime::Runtime,

	/// node tokio runtime
	_runtime: tokio::runtime::Runtime,

	/// handle to the running node.
	_task_manager: Option<TaskManager>,
}

impl InternalNode {
	pub fn rpc_handler(&self) -> Arc<MetaIoHandler<sc_rpc::Metadata>> {
		self.rpc_handlers.clone()
	}

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

	pub fn tokio_runtime(&mut self) -> &mut tokio_compat::runtime::Runtime {
		&mut self.compat_runtime
	}

	pub(crate) fn logs(&self) -> &Logger {
		&self.logger
	}
}

impl Drop for InternalNode {
	fn drop(&mut self) {
		if let Some(mut task_manager) = self._task_manager.take() {
			task_manager.terminate()
		}
	}
}

pub fn build_logger() -> Logger {
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

/// starts a manual seal authorship task.
pub fn start_node<Block, RuntimeApi, Executor, F>(cli_args: &[&str], spec_factory: F)
	-> Result<InternalNode, sc_service::Error>
	where
		Block: BlockT,
		Executor: NativeExecutionDispatch + 'static,
		F: ChainSpecFactory,
		RuntimeApi: ConstructRuntimeApi<Block, TFullClient<Block, RuntimeApi, Executor>> + Send + Sync + 'static,
		<RuntimeApi as ConstructRuntimeApi<Block, TFullClient<Block, RuntimeApi, Executor>>>::RuntimeApi:
			Core<Block> + Metadata<Block> + OffchainWorkerApi<Block> + TaggedTransactionQueue<Block>
			+ SessionKeys<Block> + BlockBuilder<Block> + ApiErrorExt<Error = sp_blockchain::Error>
			+ ApiExt<Block, StateBackend = <TFullBackend<Block> as Backend<Block>>::State>,
{
	let logger = build_logger();
	// create random directory for database
	let base_path = {
		let dir: String = rand::Rng::sample_iter(rand::thread_rng(), &rand::distributions::Alphanumeric)
			.take(15)
			.collect();
		let path = format!("/tmp/substrate-test-runner/{}", dir);
		std::fs::create_dir_all(&path).unwrap();
		format!("--base-path={}", path)
	};
	let mut args = vec![
		"--dev",
		"--no-mdns",
		"--no-prometheus",
		"--no-telemetry",
	];
	args.push(&base_path);
	args.extend(cli_args.iter().cloned());

	let cli = Cli::from_iter(args);
	let compat_runtime = tokio_compat::runtime::Runtime::new().unwrap();
	let tokio_runtime = build_runtime().unwrap();
	let runtime_handle = tokio_runtime.handle().clone();

	let task_executor = move |fut, task_type| match task_type {
		TaskType::Async => runtime_handle.spawn(fut).map(drop),
		TaskType::Blocking => runtime_handle
			.spawn_blocking(move || futures::executor::block_on(fut))
			.map(drop),
	};

	let config = cli.create_configuration(
		&cli.run,
		spec_factory,
		TaskExecutor::from(task_executor)
	).expect("failed to create node config");

	let (client, backend, keystore, mut task_manager) = new_full_parts::<Block, RuntimeApi, Executor>(&config)?;
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
				io.extend_with(
					// We provide the rpc handler with the sending end of the channel to allow the rpc
					// send EngineCommands to the background block authorship task.
					rpc::ManualSealApi::to_delegate(rpc::ManualSeal::<Block::Hash>::new(command_sink.clone())),
				);
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

	Ok(InternalNode {
		rpc_handlers: rpc_handlers.io_handler(),
		_task_manager: Some(task_manager),
		_runtime: tokio_runtime,
		compat_runtime,
		logger,
	})
}
