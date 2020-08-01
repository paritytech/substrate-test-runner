use crate::{
    node::InternalNode,
    rpc::{self, RpcExtension},
    types,
};
use jsonrpc_core_client::{RpcChannel, transports::local};
use crate::test::externalities::TestExternalities;
use tokio_compat::runtime::Runtime;

pub enum BlackBoxNode {
    /// Connects to an external node.
    External(String),
    /// Spawns a pristine node.
    Internal(InternalNode),
}

/// A black box test.
pub struct BlackBox {
    node: BlackBoxNode,
    compat_runtime: Runtime,
}

impl BlackBox {
    pub async fn with_state<R>(&mut self, closure: impl FnOnce() -> R) -> R where TRuntime: frame_system::Trait {
        TestExternalities::<TRuntime>::new(self.rpc())
            .execute_with(closure)
    }
}

impl rpc::RpcExtension for BlackBox {
    fn rpc<TClient: From<RpcChannel> + 'static>(&mut self) -> TClient {
        let client = match self.node {
            BlackBoxNode::External(ref url) => {
                self.compat_runtime.block_on_std( rpc::connect_ws(&url)).unwrap()
            },
            BlackBoxNode::Internal(ref node) => {
                use futures01::Future;
                let (client, fut) = local::connect::<TClient, _, _>(node.rpc_handler());
				self.compat_runtime.spawn(fut.map_err(|_| ()));
				
                client
            },
		};
        client
    }
}


impl BlackBox {
    pub fn new(node: BlackBoxNode) -> Self {
        let runtime = Runtime::new().unwrap();
        Self { node, compat_runtime: runtime }
    }
}

impl<TRuntime: frame_system::Trait> BlackBox {
    /// Wait `number` of blocks.
    pub fn wait_blocks(&self, _number: impl Into<types::BlockNumber<TRuntime>>) {
        todo!()
    }
}
