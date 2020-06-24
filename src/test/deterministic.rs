use crate::{
    node::InternalNode,
    rpc::{self, RpcExtension},
};
use jsonrpc_core_client::RpcChannel;
use tokio::time::{delay_for, Duration};
use async_trait::async_trait;
use futures::compat::Future01CompatExt;
use crate::test::externalities;

/// A deterministic internal instance of substrate node.
pub struct Deterministic<TRuntime> {
    node: InternalNode<TRuntime>,
}

#[async_trait]
impl<TRuntime: Send> rpc::RpcExtension for Deterministic<TRuntime> {
    // TODO [ToDr] Override and use the direct channel.
    async fn rpc<TClient: From<RpcChannel>>(&mut self) -> TClient {
		rpc::connect_ws(crate::node::RPC_WS_URL)
            .await
			.expect("error occured connecting to the node")
    }
}

impl<TRuntime: Send> Deterministic<TRuntime> {
    pub fn new(node: InternalNode<TRuntime>) -> Self {
        Self { node }
    }

    pub async fn with_state<R>(&mut self, closure: impl FnOnce() -> R) -> R
        where
            TRuntime: frame_system::Trait
    {
        externalities::TestExternalities::<TRuntime>::new(self.rpc().await)
            .execute_with(closure)
    }
}

impl<TRuntime: frame_system::Trait + Send> Deterministic<TRuntime> {
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

    /// TODO [ToDr] This method should probably be `produce_blocks(5)` when we switch to
    /// `ManualConsensus` engine.
    pub async fn produce_blocks(&mut self, diff: impl Into<crate::types::BlockNumber<TRuntime>>) where
       // TODO The bound here is a bit shitty, cause in theory the RPC is not frame-specific.
        crate::types::BlockNumber<TRuntime>: std::convert::TryFrom<primitive_types::U256> + Into<primitive_types::U256>,
    {
        // TODO [ToDr] Read from the chain.
        let current_block_number: crate::types::BlockNumber<TRuntime> = 0.into();
        let number = current_block_number + diff.into();
        let client = self.rpc::<crate::rpc::ChainClient<TRuntime>>().await;
        let mut retry = 100;
        loop {
            let header = client.header(None).compat()
                .await
                .expect("Unable to get latest header from the node.")
                .expect("No best header?");

            if header.number > number  {
                return;
            }

            retry -= 1;
            if retry == 0 {
                panic!("Unable to reach block. Best found: {:?}", header);   
            }
            delay_for(Duration::from_secs(1)).await;
        }
    }
}
