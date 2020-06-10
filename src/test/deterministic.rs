use crate::{
    node::InternalNode,
    rpc::{self, RpcExtension},
};
use jsonrpc_core_client::RpcChannel;

/// A deterministic internal instance of substrate node.
pub struct Deterministic<TRuntime> {
    node: InternalNode<TRuntime>,
}

impl<TRuntime> rpc::RpcExtension for Deterministic<TRuntime> {
    // TODO [ToDr] Override and use the direct channel.
    fn rpc<TClient: From<RpcChannel> + Send + 'static>(&mut self) -> TClient {
        rpc::connect_ws(crate::node::RPC_WS_URL)
    }
}

impl<TRuntime> crate::SubstrateTest for Deterministic<TRuntime> {}

impl<TRuntime> Deterministic<TRuntime> {
    pub fn new(node: InternalNode<TRuntime>) -> Self {
        Self { node }
    }   
}

impl<TRuntime: frame_system::Trait> Deterministic<TRuntime> {
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
    pub fn wait_for_block(&mut self, number: impl Into<crate::types::BlockNumber<TRuntime>>) where
       // TODO The bound here is a bit shitty, cause in theory the RPC is not frame-specific.
        crate::types::BlockNumber<TRuntime>: std::convert::TryFrom<primitive_types::U256> + Into<primitive_types::U256>,
    {
        use jsonrpc_core::futures::Future;

        let number = number.into();
        let client = self.rpc::<crate::rpc::ChainClient<TRuntime>>();
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
