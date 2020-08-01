pub mod blackbox;
pub mod deterministic;
pub mod externalities;

pub fn blackbox_external<TRuntime: Send>(url: &str, _runtime: TRuntime) -> blackbox::BlackBox {
    blackbox::BlackBox::new(blackbox::BlackBoxNode::External(url.into()))
}

pub fn blackbox_internal<TRuntime: Send + Sync>(runtime: TRuntime) -> blackbox::BlackBox {
    let node = crate::node::InternalNode::builder(runtime).start();
    blackbox::BlackBox::new(blackbox::BlackBoxNode::Internal(node))
}

pub fn deterministic<TRuntime: Send + Sync>(node: crate::node::InternalNode) -> deterministic::Deterministic {
    deterministic::Deterministic::new(node)
}

pub fn node(runtime: TRuntime) -> crate::node::InternalNodeBuilder {
    crate::node::InternalNode::builder(runtime)
}

