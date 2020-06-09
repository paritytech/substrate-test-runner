use jsonrpc_core_client::{RpcChannel, RawClient};

pub mod internal;

pub enum BlackBoxNode<T> {
    // Connects to an external node.
    External(String),
    // Spawns a pristine node.
    Internal(internal::Node<T>),
}

pub trait TestKind {
    fn raw_rpc(&mut self) -> RawClient {
        self.rpc()
    }

    fn rpc<TClient: From<RpcChannel> + Send + 'static>(&mut self) -> TClient {

    }
}

/// A black box test.
pub struct BlackBox<T> {
    node: BlackBoxNode<T>,
}

/// A deterministic internal instance of substrate node.
pub struct Deterministic<T> {
    node: internal::Node<T>,
}

impl<T> TestKind for Deterministic<T> {}

pub struct TestNode<T> {
    test: T,
}

impl<T> TestNode {
    pub fn new(test: T) -> Self {
        Self { test }
    }
}

impl<T> TestNode<Deterministic<T>> {}
    pub fn deterministic(node: internal::Node<T>) -> Self {
        TestNode {
            test: Deterministic { node },
        }
    }   
}

impl<T> std::ops::Deref for TestNode<Deterministic<T>> {
    type Target = internal::Node<T>;

    fn deref(&self) -> &Self::Target {
        &self.test.node
    }
}

impl<T> std::ops::DerefMut for TestNode<Deterministic<T>> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.test.node
    }
}

impl<T> TestNode<BlackBox<T>> {
    pub fn black_box(node: BlackBoxNode<T>) -> Self {
        TestNode {
            test: BlackBox { node },
        }
    }

    /// Wait `number` of blocks.
    pub fn wait_blocks(&self, number: impl Into<types::BlockNumber<T>>) {
        todo!()
    }
}


// Substrate Repo
// #[test]
// fn balances_transfer_should_work() {
//     let node = SubstrateNode::deterministic(node_cli::Service);
// }
//
// fn generic_balances_transfer_test<T: NodeKind>(node: SubstrateNode<T, R>) {
//
// }
//
// #[test]
// fn balances_transfer_should_work2() {
//     generic_balances_transfer_test(SubstrateNode::deterministic(runtime));
//
//
// // Polkadot Repo
// #[test]
// fn transfer_works() {
//     let node = SubstrateNode::start_external(polkadot::Client);
//     generic_balances_transfer_test(node);
// }
//
//
// // E2E Tests
// #[test]
// fn transfer_works() {
//     generic_balances_transfer_test(
//         SubstrateNode::external("ws://somenode:9944"),
//         runtime_primitives,
//     )
// }
//
// trait RuntimePrimitivesTrait {
//     type BlockNumber;
//     type BlockHash;
//     type Block =generic::Block<Header, OpaqueExtrinsic>;
// }
//
// impl<T: frame_system::Trait> for RuntimePrimitivesTrait for T {
//     type Block = T::Block;
// }
//
// // decl_runtime!{} 
// impl RuntimePrimitivesTrait for FrameRuntime {
//
// }
//
// //struct CustomRuntime;
// impl RuntimePrimitivesTrait for CustomRuntime {
//
// }
//
//
//
//
//
//
//
//
//
//
//
//
//
//
//
//
