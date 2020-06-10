pub mod blackbox;
pub mod deterministic;

/// A base trait for shared part of every kind of substrate test.
pub trait SubstrateTest: crate::rpc::RpcExtension {}

pub fn blackbox_external<TRuntime>(url: &str, _runtime: TRuntime) -> blackbox::BlackBox<TRuntime> {
    blackbox::BlackBox::new(blackbox::BlackBoxNode::External(url.into()))
}

pub fn blackbox_internal<TRuntime>(runtime: TRuntime) -> blackbox::BlackBox<TRuntime> {
    let node = crate::node::InternalNode::builder(runtime).start();
    blackbox::BlackBox::new(blackbox::BlackBoxNode::Internal(node))
}

pub fn deterministic<TRuntime>(node: crate::node::InternalNode<TRuntime>) -> deterministic::Deterministic<TRuntime> {
    deterministic::Deterministic::new(node)
}

pub fn node<TRuntime>(runtime: TRuntime) -> crate::node::InternalNodeBuilder<TRuntime> {
    crate::node::InternalNode::builder(runtime)
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
