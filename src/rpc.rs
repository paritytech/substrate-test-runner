use crate::types;
use jsonrpc_core_client::RpcChannel;

pub use jsonrpc_core::types::params::Params;
pub use jsonrpc_core_client::RawClient;

pub type ChainClient = sc_rpc_api::chain::ChainClient<
    types::BlockNumber,
    types::BlockHash,
    types::Header,
    types::SignedBlock,
>;

const RPC_WS_URL: &str = "ws://localhost:9944";

pub trait RpcExtension {
    fn raw_rpc(&mut self) -> RawClient {
        self.rpc()
    }

    fn rpc<TClient: From<RpcChannel> + Send + 'static>(&mut self) -> TClient;
}

impl RpcExtension for super::SubstrateNode {
    fn rpc<TClient: From<RpcChannel> + Send + 'static>(&mut self) -> TClient {
        use jsonrpc_core::futures::prelude::*;
        let (tx, rx) = std::sync::mpsc::channel();
        let url = url::Url::parse(RPC_WS_URL).expect("URL is valid");
        println!("Connecting to RPC.");
        std::thread::spawn(move || {
            tokio::run(jsonrpc_core_client::transports::ws::connect(&url)
                .map(move |client| {
                    println!("Client built, sending.");
                    tx.send(client).expect("Rx not dropped; qed");
                    println!("Sent.");
                })
                .map_err(|e| panic!("Unable to start WS client: {:?}", e))
            );
        });
        println!("Waiting for the client");
        rx.recv().expect("WS client was not able to connect.")
    }
}
