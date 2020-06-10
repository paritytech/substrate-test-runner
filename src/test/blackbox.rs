use crate::{
    node::InternalNode,
    rpc,
    types,
};
use jsonrpc_core_client::RpcChannel;

pub enum BlackBoxNode<TRuntime> {
    /// Connects to an external node.
    External(String),
    /// Spawns a pristine node.
    Internal(InternalNode<TRuntime>),
}

/// A black box test.
pub struct BlackBox<TRuntime> {
    node: BlackBoxNode<TRuntime>,
}

impl<TRuntime> rpc::RpcExtension for BlackBox<TRuntime> {
    fn rpc<TClient: From<RpcChannel> + Send + 'static>(&mut self) -> TClient {
        match self.node {
            BlackBoxNode::External(ref url) => rpc::connect_ws(&url),
            BlackBoxNode::Internal(_) => rpc::connect_ws(crate::node::RPC_WS_URL),
        }
    }
}

impl<TRuntime> crate::test::SubstrateTest for BlackBox<TRuntime> {}

impl<TRuntime> BlackBox<TRuntime> {
    pub fn new(node: BlackBoxNode<TRuntime>) -> Self {
        Self { node }
    }
}

impl<TRuntime: frame_system::Trait> BlackBox<TRuntime> {
    /// Wait `number` of blocks.
    pub fn wait_blocks(&self, _number: impl Into<types::BlockNumber<TRuntime>>) {
        todo!()
    }
}
