use crate::{
    node::InternalNode,
    rpc::{self, RpcExtension},
    types,
};
use jsonrpc_core_client::RpcChannel;
use async_trait::async_trait;
use crate::test::externalities;

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

impl<TRuntime: Send> BlackBox<TRuntime> {
    pub async fn with_state<R>(&mut self, closure: impl FnOnce() -> R) -> R where TRuntime: frame_system::Trait {
        externalities::TestExternalities::<TRuntime>::new(
            self.rpc().await
        ).execute_with(closure)
    }
}

#[async_trait]
impl<TRuntime: Send> rpc::RpcExtension for BlackBox<TRuntime> {
    async fn rpc<TClient: From<RpcChannel>>(&mut self) -> TClient {
        let future = match self.node {
            BlackBoxNode::External(ref url) => rpc::connect_ws(&url),
            BlackBoxNode::Internal(_) => rpc::connect_ws(crate::node::RPC_WS_URL),
		};
        future.await
            .unwrap()
    }
}


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
