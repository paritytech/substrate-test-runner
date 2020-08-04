pub mod blackbox;
pub mod deterministic;
pub mod externalities;

pub fn blackbox_external<Runtime: Send>(url: &str) -> blackbox::BlackBox<Runtime> {
	blackbox::BlackBox::new(blackbox::BlackBoxNode::External(url.into()))
}

pub fn blackbox_internal<Runtime: Send + Sync>() -> blackbox::BlackBox<Runtime> {
	let node = crate::node::InternalNode::<Runtime>::builder().start();
	blackbox::BlackBox::new(blackbox::BlackBoxNode::Internal(node))
}

pub fn deterministic<Runtime: Send + Sync>(
	node: crate::node::InternalNode<Runtime>,
) -> deterministic::Deterministic<Runtime> {
	deterministic::Deterministic::new(node)
}

pub fn node<Runtime>() -> crate::node::InternalNodeBuilder<Runtime> {
	crate::node::InternalNode::<Runtime>::builder()
}
