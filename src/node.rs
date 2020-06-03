use crate::RawRpcClient;
use jsonrpc_core_client::RpcChannel;

const RPC_WS_URL: &str = "ws://localhost:9944";

#[derive(Debug)]
pub enum Consensus {
    InstantSeal,
    Manual,
}

/// TODO [ToDr] This should probably be a path to the chain spec file.
type ChainSpec = &'static str;

#[derive(Debug)]
pub struct SubstrateNode {
}
impl SubstrateNode {
    pub fn builder() -> SubstrateNodeBuilder {
        let _ = env_logger::try_init();
        SubstrateNodeBuilder {
            cli: Default::default(),
            consensus: Consensus::InstantSeal,
        }
    }

    pub fn new(
        cli: &[String],    
        chain_spec: ChainSpec,
    ) -> Self {
        use sc_cli::SubstrateCli;
        let cli = node_cli::cli::Cli::from_iter(cli.iter());
        let runner = cli.create_runner(&cli.run).unwrap();
        runner.run_node(
            node_cli::service::new_light,
            node_cli::service::new_full,
            node_runtime::VERSION
        );
        Self {}
    }

    pub fn raw_rpc(&mut self) -> RawRpcClient {
        self.rpc()
    }

    pub fn rpc<TClient: From<RpcChannel> + Send + 'static>(&mut self) -> TClient {
        use futures::prelude::*;
        let (tx, rx) = std::sync::mpsc::channel();
        let url = url::Url::parse(RPC_WS_URL).expect("URL is valid");
        tokio::run(jsonrpc_core_client::transports::ws::connect(&url)
            .map(move |client| tx.send(client).expect("Rx not dropped; qed"))
            .map_err(|e| panic!("Unable to start WS client: {:?}", e))
        );
        rx.recv().expect("WS client was not able to connect.")
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
