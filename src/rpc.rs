use crate::types;
use jsonrpc_core_client::RpcChannel;

pub use jsonrpc_core::types::params::Params;
pub use jsonrpc_core_client::RawClient;

pub type ChainClient<T> = sc_rpc_api::chain::ChainClient<
    types::BlockNumber<T>,
    types::BlockHash<T>,
    types::Header<T>,
    types::SignedBlock<T>,
>;

pub trait RpcExtension {
    fn raw_rpc(&mut self) -> RawClient {
        self.rpc()
    }

    fn rpc<TClient: From<RpcChannel> + Send + 'static>(&mut self) -> TClient;
}

pub(crate) fn connect_ws<TClient: From<RpcChannel> + Send + 'static>(url: &str) -> TClient {
    use jsonrpc_core::futures::prelude::*;
    let (tx, rx) = std::sync::mpsc::channel();
    let url = url::Url::parse(url).expect("URL is valid");
    println!("Connecting to RPC at {}", url);
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

