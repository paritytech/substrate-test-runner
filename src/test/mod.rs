pub mod blackbox;
pub mod deterministic;
pub mod externalities;

use crate::node::InternalNode;

pub fn blackbox_external<R: frame_system::Trait>(url: &str) -> blackbox::BlackBox<R> {
	blackbox::BlackBox::new(blackbox::BlackBoxNode::External(url.into()))
}

pub fn blackbox_internal<R: frame_system::Trait>(node: InternalNode) -> blackbox::BlackBox<R> {
	blackbox::BlackBox::new(blackbox::BlackBoxNode::Internal(node))
}

pub fn deterministic<R: frame_system::Trait>(node: InternalNode) -> deterministic::Deterministic<R> {
	deterministic::Deterministic::new(node)
}
