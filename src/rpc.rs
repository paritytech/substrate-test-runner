use crate::types;
use std::convert::TryFrom;
use jsonrpc_core_client::RpcChannel;
use jsonrpc_core::futures::Future;

pub use jsonrpc_core::types::params::Params;
pub use jsonrpc_core_client::RawClient;

pub type ChainClient<T> = sc_rpc_api::chain::ChainClient<
    types::BlockNumber<T>,
    types::BlockHash<T>,
    types::Header<T>,
    types::SignedBlock<T>,
>;

const RPC_WS_URL: &str = "ws://localhost:9944";

pub trait RpcExtension<T> {
    fn raw_rpc(&mut self) -> RawClient {
        self.rpc()
    }

    fn rpc<TClient: From<RpcChannel> + Send + 'static>(&mut self) -> TClient;

    fn wait_for_block(&mut self, number: impl Into<types::BlockNumber<T>>) where
        // TODO The bound here is a bit shitty, cause in theory the RPC is not frame-specific.
        T: frame_system::Trait,
        types::BlockNumber<T>: TryFrom<primitive_types::U256> + Into<primitive_types::U256>,
    {
        let number = number.into();
        let client = self.rpc::<ChainClient<T>>();
        let mut retry = 100;
        loop {
            let header = client.header(None).wait()
                .expect("Unable to get latest header from the node.")
                .expect("No best header?");

            if header.number > number  {
                return;
            }

            retry -= 1;
            if retry == 0 {
                panic!("Unable to reach block. Best found: {:?}", header);   
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}

impl<T> RpcExtension<T> for super::SubstrateNode<T> {
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
