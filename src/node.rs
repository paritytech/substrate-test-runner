use std::io::Write;
use std::sync::Arc;
use parking_lot::RwLock;
use sc_service::{ServiceBuilder, Configuration, ServiceComponents, TaskManager, RpcHandlers, TaskType};
use sp_inherents::InherentDataProviders;
use sc_cli::{SubstrateCli, build_runtime, CliConfiguration};
use jsonrpc_core::MetaIoHandler;
use sc_executor::native_executor_instance;

// Our native executor instance.
native_executor_instance!(
	pub Executor,
	runtime::api::dispatch,
	runtime::native_version,
);


/// TODO [ToDr] Remove in favour of direct use of `AbstractService`.
pub(crate) const RPC_WS_URL: &str = "ws://127.0.0.1:9944";

/// TODO [ToDr] This should probably be a path to the chain spec file.
type ChainSpec = &'static str;

type Module = String;
type Logger = Arc<RwLock<std::collections::HashMap<Module, Vec<String>>>>;

/// this holds a reference to a running node on another thread,
/// we set a port over cli, process is dropped when this struct is dropped
/// holds logs from the process.
pub struct InternalNode<T> {
    runtime: T,
    logs: Logger,
    tokio_runtime: tokio::runtime::Runtime,
    rpc_handlers: Arc<MetaIoHandler<sc_rpc::Metadata>>,
    task_manager: TaskManager,
}

impl<T> From<T> for InternalNode<T> {
    fn from(runtime: T) -> Self {
        InternalNodeBuilder::new(runtime).start()
    }
}

impl<T> InternalNode<T> {
    pub fn builder(runtime: T) -> InternalNodeBuilder<T> {
        InternalNodeBuilder::new(runtime)
    }

    pub fn new(logs: Logger, cli: &[String], runtime: T) -> Self {
        use sc_cli::SubstrateCli;
        let cli = node_cli::Cli::from_iter(cli.iter());
        let tokio_runtime = build_runtime().unwrap();
        let runtime_handle = tokio_runtime.handle().clone();

        let task_executor = move |fut, task_type| {
            match task_type {
                TaskType::Async => { runtime_handle.spawn(fut); }
                TaskType::Blocking => {
                    runtime_handle.spawn(async move {
                        // `spawn_blocking` is looking for the current runtime, and as such has to
                        // be called from within `spawn`.
                        tokio::task::spawn_blocking(move || futures::executor::block_on(fut))
                    });
                }
            }
        };

        let config = cli
            .create_configuration(&cli.run, task_executor.into())
            // Todo: return result
            .unwrap();
        // TODO: result
        let (task_manager, rpc_handlers) = build_node(config)
            .unwrap();

        Self {
            logs,
            runtime,
            task_manager,
            tokio_runtime,
            rpc_handlers: Arc::new(rpc_handlers.into_handler().into()),
        }
    }

    pub fn rpc_handler(&self) -> Arc<MetaIoHandler<sc_rpc::Metadata>> {
        self.rpc_handlers.clone()
    }

    pub(crate) fn logs(&self) -> &Logger {
        &self.logs
    }
}

#[derive(Debug)]
pub struct InternalNodeBuilder<T> {
    /// Parameters passed as-is.
    cli: Vec<String>,
    logs: Logger,
    runtime: T,
}

impl<T> From<InternalNodeBuilder<T>> for InternalNode<T> {
    fn from(builder: InternalNodeBuilder<T>) -> Self {
        builder.start()
    }
}

impl<T> InternalNodeBuilder<T> {
    pub fn new(runtime: T) -> Self {
        let ignore = [
            "yamux", "multistream_select", "libp2p",
            "sc_network", "tokio_reactor", "jsonrpc_client_transports",
            "ws", "sc_network::protocol::generic_proto::behaviour",
            "sc_service", "sc_peerset", "rpc", "sub-libp2p", "sync", "peerset"
        ];
        let logs = Logger::default();
        {
            let logs = logs.clone();
            let mut builder = env_logger::builder();
            builder.format(move |buf: &mut env_logger::fmt::Formatter, record: &log::Record| {
                let entry = format!("{} {} {}", record.level(), record.target(), record.args());
                let res = writeln!(buf, "{}", entry);
                logs.write()
                    .entry(record.target().to_string())
                    .or_default()
                    .push(entry);
                res
            });
            builder.filter_level(log::LevelFilter::Debug);
            builder.filter_module("runtime", log::LevelFilter::Trace);
            for module in &ignore {
                builder.filter_module(module, log::LevelFilter::Info);
            }

            let _ = builder
                .is_test(true)
                .try_init();
        }

        // create random directory for database
        let random_path = {
            let dir: String = rand::Rng::sample_iter(
                    rand::thread_rng(),
                    &rand::distributions::Alphanumeric
                )
                .take(15)
                .collect();
            let path = format!("/tmp/substrate-test-runner/{}", dir);
            std::fs::create_dir_all(&path).unwrap();
            path
        };

        Self {
            cli: vec![
                "--no-mdns".into(),
                "--no-prometheus".into(),
                "--no-telemetry".into(),
                format!("--base-path={}", random_path),
                "--dev".into(),
            ],
            logs,
            runtime,
        }
    }

    pub fn cli_param(mut self, param: &str) -> Self {
        self.cli.push(param.into());
        self
    }

    pub fn start(self) -> InternalNode<T> {
        InternalNode::new(self.logs, &self.cli, self.runtime)
    }
}

/// TODO: should be generic over the runtime, block and executor.
/// starts a manual seal authorship task.
pub fn build_node(config: Configuration) -> Result<(TaskManager, RpcHandlers), sc_service::Error> {
    // Channel for the rpc handler to communicate with the authorship task.
    let (command_sink, commands_stream) = futures::channel::mpsc::channel(1000);

    let ServiceComponents {
        task_manager, rpc_handlers, client,
        transaction_pool, select_chain, prometheus_registry,
        ..
    } = ServiceBuilder::new_full::<runtime::opaque::Block, runtime::RuntimeApi, Executor>(config)?
        .with_select_chain(|_config, backend| {
            Ok(sc_consensus::LongestChain::new(backend.clone()))
        })?
        .with_transaction_pool(|builder| {
            let pool_api = sc_transaction_pool::FullChainApi::new(builder.client().clone());
            Ok(sc_transaction_pool::BasicPool::new(
                builder.config().transaction_pool.clone(),
                std::sync::Arc::new(pool_api),
                builder.prometheus_registry(),
            ))
        })?
        .with_import_queue(|_config, client, _select_chain, _transaction_pool, spawn_task_handle, registry| {
            Ok(manual_seal::import_queue(
                Box::new(client),
                spawn_task_handle,
                registry,
            ))
        })?
        .with_rpc_extensions(|_| -> Result<jsonrpc_core::IoHandler<sc_rpc::Metadata>, _> {
            use manual_seal::rpc;
            let mut io = jsonrpc_core::IoHandler::default();
            io.extend_with(
                // We provide the rpc handler with the sending end of the channel to allow the rpc
                // send EngineCommands to the background block authorship task.
                rpc::ManualSealApi::to_delegate(rpc::ManualSeal::<runtime::Hash>::new(command_sink)),
            );
            Ok(io)
        })?
        .build_full()?;

    let inherent_data_providers = InherentDataProviders::new();
    inherent_data_providers
        .register_provider(sp_timestamp::InherentDataProvider)
        .unwrap();

    // Proposer object for block authorship.
    let proposer = sc_basic_authorship::ProposerFactory::new(
        client.clone(),
        transaction_pool.clone(),
        prometheus_registry.as_ref(),
    );

    // Background authorship future.
    let authorship_future = manual_seal::run_manual_seal(
        Box::new(client.clone()),
        proposer,
        client,
        transaction_pool.pool().clone(),
        commands_stream,
        select_chain.expect("SelectChain is set at Service initialization; qed"),
        inherent_data_providers,
    );

    // we spawn the future on a background thread managed by service.
    task_manager.spawn_essential_handle()
        .spawn("manual-seal", authorship_future);

    // we really only care about the rpc interface.
    Ok((task_manager, rpc_handlers))
}