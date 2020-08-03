use crate::{
    node::InternalNode,
    rpc::{self, RpcExtension},
};
use jsonrpc_core_client::RpcChannel;
use crate::test::externalities::TestExternalities;
use jsonrpc_core_client::transports::local;
use manual_seal::rpc::ManualSealClient;
use std::ops::{Deref, DerefMut};

/// A deterministic internal instance of substrate node.
pub struct Deterministic<Runtime> {
    node: InternalNode<Runtime>,
}

impl<Runtime> rpc::RpcExtension for Deterministic<Runtime> {
    fn rpc<TClient: From<RpcChannel> + 'static>(&mut self) -> TClient {
        use futures01::Future;
        let rpc_handler = self.node.rpc_handler();
        let (client, fut) = local::connect::<TClient, _, _>(rpc_handler);
        self.node.tokio_runtime().spawn(fut.map_err(|_| ()));

        client
    }
}

impl<Runtime> Deterministic<Runtime> {
    pub fn new(node: InternalNode<Runtime>) -> Self {
        Self { node }
    }
}

impl<Runtime: frame_system::Trait + Send + Sync> Deterministic<Runtime> {
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
		log::info!("produce blocks");

		let client = self.rpc::<ManualSealClient<Runtime::Hash>>();
		log::info!("produce blocks");
		
        for _ in 0..num {
			self.node.tokio_runtime()
				.block_on(client.create_block(true, true, None))
                .expect("block production failed: ");

		}
		log::info!("sealed {} blocks", num)
	}
	

    pub fn with_state<R>(&mut self, closure: impl FnOnce() -> R) -> R {
        TestExternalities::<Runtime>::new(self.rpc())
            .execute_with(closure)
	}
	
}

impl<T> Deref for Deterministic<T> {
    type Target = InternalNode<T>;

    fn deref(&self) -> &Self::Target {
        &self.node
    }
}

impl<T> DerefMut for Deterministic<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.node
    }
}


