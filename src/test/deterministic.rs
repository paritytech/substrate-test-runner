use crate::{
    node::InternalNode,
    rpc::{self, RpcExtension},
};
use jsonrpc_core_client::RpcChannel;
use tokio::time::{delay_for, Duration};
use async_trait::async_trait;
use futures::compat::Future01CompatExt;
use crate::test::externalities;
use tokio_compat::runtime::Runtime;
use futures::FutureExt;
use jsonrpc_core_client::transports::local;
use std::thread;
use manual_seal::rpc::ManualSealClient;

/// A deterministic internal instance of substrate node.
pub struct Deterministic<TRuntime> {
    node: InternalNode<TRuntime>,
    compat_runtime: Runtime,
}

impl<TRuntime: Send + Sync> rpc::RpcExtension for Deterministic<TRuntime> {
    fn rpc<TClient: From<RpcChannel> + 'static>(&mut self) -> TClient {
        use futures01::Future;
        let rpc_handler = self.node.rpc_handler();
        let (client, fut) = local::connect::<TClient, _, _>(rpc_handler);
        self.compat_runtime.spawn(fut.map_err(|err| {
            log::error!("\n\nerror: {:?}\n\n", err)
        }));

        client
    }
}

impl<TRuntime: Send + Sync> Deterministic<TRuntime> {
    pub fn new(node: InternalNode<TRuntime>) -> Self {
        let runtime = Runtime::new().unwrap();
        Self { node, compat_runtime: runtime }
    }

    pub fn with_state<R>(&mut self, closure: impl FnOnce() -> R) -> R
        where
            TRuntime: frame_system::Trait
    {
        let client = self.rpc();
        externalities::TestExternalities::<TRuntime>::new(client)
            .execute_with(closure)
    }
}

impl<TRuntime: frame_system::Trait + Send + Sync> Deterministic<TRuntime> {
    pub fn assert_log_line(&self, module: &str, content: &str) {
        if let Some(logs) = self.node.logs().read().get(module) {
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

    pub fn produce_blocks(&mut self, num: usize) {
        let client = self.rpc::<ManualSealClient<runtime::Block>>();
        self.compat_runtime.block_on_std(async {
           for _ in 0..num {
               println!("\n\nproduce_blocks\n\n");
               let result = client.create_block(true, true, None)
                   .compat()
                   .await
                   .expect("failed to create blocks");
               println!("\n\n\n{:#?}\n\n\n\n", result)
           }
        });
    }
}
