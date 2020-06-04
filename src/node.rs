#[derive(Debug)]
pub enum Consensus {
    InstantSeal,
    Manual,
}

/// TODO [ToDr] This should probably be a path to the chain spec file.
type ChainSpec = &'static str;

#[derive(Debug)]
pub struct SubstrateNode {
    node_handle: Option<std::thread::JoinHandle<Result<(), sc_cli::Error>>>,
    stop_signal: Option<futures::channel::oneshot::Sender<()>>,
}

impl SubstrateNode {
    pub fn builder() -> SubstrateNodeBuilder {
        let ignore = [
            "yamux", "multistream_select", "libp2p",
            "sc_network", "tokio_reactor", "jsonrpc_client_transports",
            "ws", "sc_network::protocol::generic_proto::behaviour",
            "sc_service", "sc_peerset"
        ];
        {
            let mut builder = env_logger::builder();
            builder.filter_level(log::LevelFilter::Debug);
            for module in &ignore {
                builder.filter(Some(module), log::LevelFilter::Info);
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
                format!("--base-path={}", random_path)
            ],
            consensus: Consensus::InstantSeal,
        }
    }

    pub fn new(
        cli: &[String],    
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
            println!("Node is starting up.");
            runner.run_node_until(
                node_cli::service::new_light,
                node_cli::service::new_full,
                rx.map(|_| ())
            )
        });

        // That's so crappy
        start.recv().unwrap();
        std::thread::sleep(std::time::Duration::from_secs(5));

        println!("Node built.");
        Self {
            node_handle: Some(handle),
            stop_signal: Some(tx),
        }
    }
}

impl Drop for SubstrateNode {
    fn drop(&mut self) {
        println!("Killing node");
        // TODO [ToDr] unwraps!
        if let Some(signal) = self.stop_signal.take() {
            signal.send(()).unwrap();
        }
        println!("Waiting for finish");
        if let Some(handle) = self.node_handle.take() {
            handle.join().unwrap().unwrap();
        }
    }
}

#[derive(Debug)]
pub struct SubstrateNodeBuilder {
    /// Parameters passed as-is.
    cli: Vec<String>,
    /// TODO [ToDr] This should be used to construct a special chainspec file.
    consensus: Consensus,
}

impl SubstrateNodeBuilder {
    pub fn cli_param(mut self, param: &str) -> Self {
        self.cli.push(param.into());
        self
    }

    pub fn consensus(mut self, consensus: Consensus) -> Self {
        log::warn!("Changing consensus is a no-op currently.");
        self.consensus = consensus;
        self
    }

    pub fn start(&mut self) -> SubstrateNode {
        // TODO [ToDr] Actaully create the chainspec.
        let chain_spec = "dev";

        SubstrateNode::new(
            &self.cli,
            chain_spec
        )
    }
}
