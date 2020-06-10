use crate::node::{TestNode, TestKind, rpc_connect_ws, internal};
use crate::types;
use jsonrpc_core_client::RpcChannel;

pub enum BlackBoxNode<TRuntime> {
    /// Connects to an external node.
    External(String),
    /// Spawns a pristine node.
    Internal(internal::Node<TRuntime>),
}

/// A black box test.
pub struct BlackBox<TRuntime> {
    node: BlackBoxNode<TRuntime>,
}

impl<TRuntime> TestKind for BlackBox<TRuntime> {
    fn rpc<TClient: From<RpcChannel> + Send + 'static>(&mut self) -> TClient {
        match self.node {
            BlackBoxNode::External(ref url) => rpc_connect_ws(url),
            BlackBoxNode::Internal(_) => rpc_connect_ws(internal::RPC_WS_URL),
        }
    }
}

impl<TRuntime> TestNode<BlackBox<TRuntime>> {
    pub fn black_box(node: BlackBoxNode<TRuntime>) -> Self {
        TestNode {
            test: BlackBox { node },
        }
    }
}
impl<TRuntime: frame_system::Trait> TestNode<BlackBox<TRuntime>> {
    /// Wait `number` of blocks.
    pub fn wait_blocks(&self, number: impl Into<types::BlockNumber<TRuntime>>) {
        todo!()
    }
}
