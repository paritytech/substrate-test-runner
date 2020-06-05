use std::io::Write;
use std::sync::Arc;
use parking_lot::RwLock;

#[derive(Debug)]
pub enum Consensus {
    InstantSeal,
    Manual,
}

/// TODO [ToDr] This should probably be a path to the chain spec file.
type ChainSpec = &'static str;

type Module = String;
type Logger = Arc<RwLock<std::collections::HashMap<Module, Vec<String>>>>;

#[derive(Debug)]
pub struct SubstrateNode<T> {
    node_handle: Option<std::thread::JoinHandle<Result<(), sc_cli::Error>>>,
    stop_signal: Option<futures::channel::oneshot::Sender<()>>,
    logs: Logger,
    runtime: T,
}

impl<T> SubstrateNode<T> {
    pub fn builder(runtime: T) -> SubstrateNodeBuilder<T> {
        let ignore = [
            "yamux", "multistream_select", "libp2p",
            "sc_network", "tokio_reactor", "jsonrpc_client_transports",
            "ws", "sc_network::protocol::generic_proto::behaviour",
            "sc_service", "sc_peerset", "rpc", "sub-libp2p"
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

        SubstrateNodeBuilder {
            cli: vec![
                "--dev".into(),
                "--no-mdns".into(),
                "--no-prometheus".into(),
                "--no-telemetry".into(),
                format!("--base-path={}", random_path)
            ],
            logs,
            consensus: Consensus::InstantSeal,
            runtime,
        }
    }

    pub fn new(
        logs: Logger,
        cli: &[String],    
        runtime: T,
        _chain_spec: ChainSpec,
    ) -> Self {
        use futures::future::FutureExt;
        use sc_cli::SubstrateCli;
        let (tx, rx) = futures::channel::oneshot::channel();
        let (send_start, start) = std::sync::mpsc::channel();
        let cli = node_cli::Cli::from_iter(cli.iter());
        // TODO [ToDr] Get a handle of `AbstractService` instead
        // (crate_configuration + new_light/new_full)
        // it can be used to send RPC requests directly.
        let handle = std::thread::spawn(move || {
            let runner = cli.create_runner(&cli.run)
                .expect("Unable to create Node runner.");
            let _ = send_start.send(());
            runner.run_node_until(
                node_cli::service::new_light,
                node_cli::service::new_full,
                rx.map(|_| ())
            )
        });

        // That's so crappy
        start.recv().unwrap();
        std::thread::sleep(std::time::Duration::from_secs(5));

        Self {
            node_handle: Some(handle),
            stop_signal: Some(tx),
            logs,
            runtime,
        }
    }

    pub fn assert_log_line(&self, module: &str, content: &str) {
        if let Some(logs) = self.logs.read().get(module) {
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
}

impl<T> Drop for SubstrateNode<T> {
    fn drop(&mut self) {
        // TODO [ToDr] unwraps!
        if let Some(signal) = self.stop_signal.take() {
            signal.send(()).unwrap();
        }
        if let Some(handle) = self.node_handle.take() {
            handle.join().unwrap().unwrap();
        }
    }
}

#[derive(Debug)]
pub struct SubstrateNodeBuilder<T> {
    /// Parameters passed as-is.
    cli: Vec<String>,
    /// TODO [ToDr] This should be used to construct a special chainspec file.
    consensus: Consensus,
    logs: Logger,
    runtime: T,
}

impl<T> SubstrateNodeBuilder<T> {
    pub fn cli_param(mut self, param: &str) -> Self {
        self.cli.push(param.into());
        self
    }

    pub fn consensus(mut self, consensus: Consensus) -> Self {
        log::warn!("Changing consensus is a no-op currently.");
        self.consensus = consensus;
        self
    }

    pub fn start(self) -> SubstrateNode<T> {
        // TODO [ToDr] Actaully create the chainspec.
        let chain_spec = "dev";

        SubstrateNode::new(
            self.logs,
            &self.cli,
            self.runtime,
            chain_spec,
        )
    }
}
//
// trait TestKind {}
// struct SubstrateNode<T: TestKind>;
//
// enum NodeKind {
//     ExternalOverWebSocket,
//     ExternalOverHttp, 
//     /// Addds the clean-state assumption
//     Internal,
// }
//
// struct BlackBox {
//     node: NodeKind,
//     client: Option<Box<_>>,
// }
// impl TestKind for BlackBox {}
//
// impl SubstrateNode<BlackBox> {
//  pub fn wait_for_block() {}
//  pub fn rpc<T>() -> T { todo!() }
// }
//
// struct Deterministic {}
// impl TestKind for Deterministic {}
//
// impl SubstrateNode<Deterministic> {
//  pub fn create() {}
//  pub fn assert_log_line() {}
//  pub fn rpc<T>() -> T { todo!() }
// } 

// Substrate Repo
// #[test]
// fn balances_transfer_should_work() {
//     let node = SubstrateNode::deterministic(node_cli::Service);
// }
//
// fn generic_balances_transfer_test<T: NodeKind>(node: SubstrateNode<T, R>) {
//
// }
//
// #[test]
// fn balances_transfer_should_work2() {
//     generic_balances_transfer_test(SubstrateNode::deterministic(runtime));
//
//
// // Polkadot Repo
// #[test]
// fn transfer_works() {
//     let node = SubstrateNode::start_external(polkadot::Client);
//     generic_balances_transfer_test(node);
// }
//
//
// // E2E Tests
// #[test]
// fn transfer_works() {
//     generic_balances_transfer_test(
//         SubstrateNode::external("ws://somenode:9944"),
//         runtime_primitives,
//     )
// }
//
// trait RuntimePrimitivesTrait {
//     type BlockNumber;
//     type BlockHash;
//     type Block =generic::Block<Header, OpaqueExtrinsic>;
// }
//
// impl<T: frame_system::Trait> for RuntimePrimitivesTrait for T {
//     type Block = T::Block;
// }
//
// // decl_runtime!{} 
// impl RuntimePrimitivesTrait for FrameRuntime {
//
// }
//
// //struct CustomRuntime;
// impl RuntimePrimitivesTrait for CustomRuntime {
//
// }
//
//
//
//
//
//
//
//
//
//
//
//
//
//
//
//
