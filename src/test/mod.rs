pub mod blackbox;
pub mod deterministic;
pub mod externalities;

use crate::node::{InternalNode, TestRuntimeRequirements};

pub fn blackbox_external<R: TestRuntimeRequirements>(url: &str) -> blackbox::BlackBox<R> {
	blackbox::BlackBox::new(blackbox::BlackBoxNode::External(url.into()))
}

pub fn blackbox_internal<R: TestRuntimeRequirements>(node: InternalNode<R>) -> blackbox::BlackBox<R> {
	blackbox::BlackBox::new(blackbox::BlackBoxNode::Internal(node))
}

pub fn deterministic<R: TestRuntimeRequirements + 'static>(node: InternalNode<R>) -> deterministic::Deterministic<R> {
	deterministic::Deterministic::new(node)
}
