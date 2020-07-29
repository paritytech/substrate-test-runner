use crate::{
    node::InternalNode,
    rpc::{self, RpcExtension},
    types,
};
use jsonrpc_core_client::{RpcChannel, transports::local};
use crate::test::externalities::TestExternalities;
use tokio_compat::runtime::Runtime;

pub enum BlackBoxNode<TRuntime> {
    /// Connects to an external node.
    External(String),
    /// Spawns a pristine node.
    Internal(InternalNode<TRuntime>),
}

/// A black box test.
pub struct BlackBox<TRuntime> {
    node: BlackBoxNode<TRuntime>,
    compat_runtime: Runtime,
}

impl<TRuntime: Send + Sync> BlackBox<TRuntime> {
    pub async fn with_state<R>(&mut self, closure: impl FnOnce() -> R) -> R where TRuntime: frame_system::Trait {
        TestExternalities::<TRuntime>::new(self.rpc())
            .execute_with(closure)
    }
}

impl<TRuntime: Send> rpc::RpcExtension for BlackBox<TRuntime> {
    fn rpc<TClient: From<RpcChannel> + 'static>(&mut self) -> TClient {
        let client = match self.node {
            BlackBoxNode::External(ref url) => {
                self.compat_runtime.block_on_std( rpc::connect_ws(&url)).unwrap()
            },
            BlackBoxNode::Internal(ref internal) => {
                use futures01::Future;

                let (client, fut) = local::connect::<TClient, _, _>(internal.rpc_handler());
                self.compat_runtime.spawn(fut.map_err(|_| ()));
                client
            },
		};
        client
    }
}


impl<TRuntime> BlackBox<TRuntime> {
    pub fn new(node: BlackBoxNode<TRuntime>) -> Self {
        let runtime = Runtime::new().unwrap();
        Self { node, compat_runtime: runtime }
    }
}

impl<TRuntime: frame_system::Trait> BlackBox<TRuntime> {
    /// Wait `number` of blocks.
    pub fn wait_blocks(&self, _number: impl Into<types::BlockNumber<TRuntime>>) {
        todo!()
    }
}
