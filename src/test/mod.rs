pub mod blackbox;
pub mod deterministic;
pub mod externalities;

use crate::node::{InternalNode, TestRuntimeRequirements};
use sp_api::{ConstructRuntimeApi, ApiErrorExt, ApiExt};
use sc_service::{TFullClient, TFullBackend};
use sp_api::{Core, Metadata};
use sp_offchain::OffchainWorkerApi;
use sp_session::SessionKeys;
use sp_transaction_pool::runtime_api::TaggedTransactionQueue;
use sp_block_builder::BlockBuilder;
use sc_client_api::Backend;

pub fn blackbox_external<R: TestRuntimeRequirements>(url: &str) -> blackbox::BlackBox<R> {
	blackbox::BlackBox::new(blackbox::BlackBoxNode::External(url.into()))
}

pub fn blackbox_internal<R: TestRuntimeRequirements>(node: InternalNode<R>) -> blackbox::BlackBox<R> {
	blackbox::BlackBox::new(blackbox::BlackBoxNode::Internal(node))
}

pub fn deterministic<Node>() -> deterministic::Deterministic<Node>
	where
		Node: TestRuntimeRequirements + 'static,
		<Node::RuntimeApi as
			ConstructRuntimeApi<
				Node::Block,
				TFullClient<Node::Block, Node::RuntimeApi, Node::Executor>
			>
		>::RuntimeApi: Core<Node::Block> + Metadata<Node::Block>
		+ OffchainWorkerApi<Node::Block> + SessionKeys<Node::Block>
		+ TaggedTransactionQueue<Node::Block> + BlockBuilder<Node::Block>
		+ ApiErrorExt<Error=sp_blockchain::Error>
		+ ApiExt<
			Node::Block,
			StateBackend =
			<TFullBackend<Node::Block> as Backend<Node::Block>>::State,
		>,
{
	deterministic::Deterministic::new(InternalNode::<Node>::new().unwrap())
}
