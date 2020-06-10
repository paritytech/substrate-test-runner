use crate::node::{TestNode, TestKind, internal, rpc_connect_ws};
use jsonrpc_core_client::RpcChannel;

/// A deterministic internal instance of substrate node.
pub struct Deterministic<TRuntime> {
    node: internal::Node<TRuntime>,
}

impl<TRuntime> TestKind for Deterministic<TRuntime> {
    // TODO [ToDr] Override and use the direct channel.
    fn rpc<TClient: From<RpcChannel> + Send + 'static>(&mut self) -> TClient {
        rpc_connect_ws(internal::RPC_WS_URL)
    }
}

impl<TRuntime> TestNode<Deterministic<TRuntime>> {
    pub fn deterministic(node: internal::Node<TRuntime>) -> Self {
        TestNode {
            test: Deterministic { node },
        }
    }   
}

impl<TRuntime> std::ops::Deref for TestNode<Deterministic<TRuntime>> {
    type Target = internal::Node<TRuntime>;

    fn deref(&self) -> &Self::Target {
        &self.test.node
    }
}

impl<TRuntime> std::ops::DerefMut for TestNode<Deterministic<TRuntime>> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.test.node
    }
}

impl<TRuntime: frame_system::Trait> TestNode<Deterministic<TRuntime>> {
    fn wait_for_block(&mut self, number: impl Into<crate::types::BlockNumber<TRuntime>>) where
        // TODO The bound here is a bit shitty, cause in theory the RPC is not frame-specific.
        crate::types::BlockNumber<TRuntime>: std::convert::TryFrom<primitive_types::U256> + Into<primitive_types::U256>,
    {
        use jsonrpc_core::futures::Future;

        let number = number.into();
        let client = self.test.rpc::<crate::rpc::ChainClient<TRuntime>>();
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
