use jsonrpc_core_client::{RpcChannel, RawClient};

pub mod internal;
pub mod blackbox;
pub mod deterministic;

fn rpc_connect_ws<TClient: From<RpcChannel> + Send + 'static>(url: &str) -> TClient {
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

pub trait TestKind {
    fn raw_rpc(&mut self) -> RawClient {
        self.rpc()
    }

    fn rpc<TClient: From<RpcChannel> + Send + 'static>(&mut self) -> TClient;
}

pub struct TestNode<T> {
    test: T,
}

impl<T> TestNode<T> {
    pub fn new(test: T) -> Self {
        Self { test }
    }
}

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
