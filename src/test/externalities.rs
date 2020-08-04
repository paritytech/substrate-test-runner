use crate::rpc;
use futures01::Future;
use sp_core::offchain::TransactionPool;
use sp_externalities::Extensions;
use sp_storage::{ChildInfo, StorageKey};
use std::any::{Any, TypeId};

pub struct TestExternalities<Runtime: frame_system::Trait> {
	client: rpc::StateClient<Runtime>,
	extensions: Extensions,
}

pub struct TxPoolExtApi<Runtime: frame_system::Trait> {
	client: rpc::AuthorClient<Runtime>,
}

impl<Runtime: frame_system::Trait> TransactionPool for TxPoolExtApi<Runtime> {
	fn submit_transaction(&mut self, extrinsic: Vec<u8>) -> Result<(), ()> {
		match self.client.submit_extrinsic(extrinsic.into()).wait() {
			Ok(hash) => log::info!("extrinsic successfully submitted with hash {:?}", hash),
			Err(err) => log::error!("error submitting extrinsic {:?}", err),
		};
		Ok(())
	}
}

impl<Runtime: frame_system::Trait> TxPoolExtApi<Runtime> {
	pub fn new(client: rpc::AuthorClient<Runtime>) -> Self {
		Self { client }
	}
}

impl<Runtime: frame_system::Trait> TestExternalities<Runtime> {
	pub fn new(client: rpc::StateClient<Runtime>) -> Self {
		Self {
			client,
			extensions: Extensions::new(),
		}
	}

	pub fn execute_with<R>(&mut self, execute: impl FnOnce() -> R) -> R {
		sp_externalities::set_and_run_with_externalities(self, execute)
	}
}

impl<Runtime: frame_system::Trait> sp_externalities::ExtensionStore for TestExternalities<Runtime> {
	fn extension_by_type_id(&mut self, type_id: TypeId) -> Option<&mut dyn Any> {
		self.extensions.get_mut(type_id)
	}

	fn register_extension_with_type_id(
		&mut self,
		type_id: TypeId,
		extension: Box<dyn sp_externalities::Extension>,
	) -> Result<(), sp_externalities::Error> {
		self.extensions.register_with_type_id(type_id, extension)
	}

	fn deregister_extension_by_type_id(&mut self, type_id: TypeId) -> Result<(), sp_externalities::Error> {
		self.extensions
			.deregister(type_id)
			.ok_or(sp_externalities::Error::ExtensionIsNotRegistered(type_id))
			.map(drop)
	}
}

impl<Runtime: frame_system::Trait> sp_externalities::Externalities for TestExternalities<Runtime> {
	fn set_offchain_storage(&mut self, _key: &[u8], _value: Option<&[u8]>) {
		unimplemented!("set_offchain_storage")
	}

	fn storage(&self, key: &[u8]) -> Option<Vec<u8>> {
		// this is pretty weird, but stay with me.
		// the tests in `simple_run` is wrapped with a tokio runtime
		// so this means the code path here has access to the tokio v0.1 runtime
		// requried for this future to complete, without the runtime, this call would panic.
		self.client
			.storage(StorageKey(key.to_vec()), None)
			.wait()
			.ok()
			.flatten()
			.map(|data| data.0)
	}

	fn storage_hash(&self, _key: &[u8]) -> Option<Vec<u8>> {
		unimplemented!("storage_hash")
	}

	fn child_storage_hash(&self, _child_info: &ChildInfo, _key: &[u8]) -> Option<Vec<u8>> {
		unimplemented!("child_storage_hash")
	}

	fn child_storage(&self, _child_info: &ChildInfo, _key: &[u8]) -> Option<Vec<u8>> {
		unimplemented!("child_storage")
	}

	fn next_storage_key(&self, _key: &[u8]) -> Option<Vec<u8>> {
		unimplemented!("next_storage_key")
	}

	fn next_child_storage_key(&self, _child_info: &ChildInfo, _key: &[u8]) -> Option<Vec<u8>> {
		unimplemented!("next_child_storage_key")
	}

	fn kill_child_storage(&mut self, _child_info: &ChildInfo) {
		unimplemented!("kill_child_storage")
	}

	fn clear_prefix(&mut self, _prefix: &[u8]) {
		unimplemented!("clear_prefix")
	}

	fn clear_child_prefix(&mut self, _child_info: &ChildInfo, _prefix: &[u8]) {
		unimplemented!("clear_child_prefix")
	}

	fn place_storage(&mut self, _key: Vec<u8>, _value: Option<Vec<u8>>) {
		// Create a sudo transaction that alters storage on-chain.
		unimplemented!("place_storage")
	}

	fn place_child_storage(&mut self, _child_info: &ChildInfo, _key: Vec<u8>, _value: Option<Vec<u8>>) {
		unimplemented!("place_child_storage")
	}

	fn chain_id(&self) -> u64 {
		unimplemented!("chain_id")
	}

	fn storage_root(&mut self) -> Vec<u8> {
		unimplemented!("storage_root")
	}

	fn child_storage_root(&mut self, _child_info: &ChildInfo) -> Vec<u8> {
		unimplemented!("child_storage_root")
	}

	fn storage_append(&mut self, _key: Vec<u8>, _value: Vec<u8>) {
		unimplemented!("storage_append")
	}

	fn storage_changes_root(&mut self, _parent: &[u8]) -> Result<Option<Vec<u8>>, ()> {
		unimplemented!("storage_changes_root")
	}

	fn storage_start_transaction(&mut self) {
		unimplemented!("storage_start_transaction")
	}

	fn storage_rollback_transaction(&mut self) -> Result<(), ()> {
		unimplemented!("storage_rollback_transaction")
	}

	fn storage_commit_transaction(&mut self) -> Result<(), ()> {
		unimplemented!("storage_commit_transaction")
	}

	fn wipe(&mut self) {
		unimplemented!("wipe")
	}

	fn commit(&mut self) {
		unimplemented!("commit")
	}

	fn read_write_count(&self) -> (u32, u32, u32, u32) {
		unimplemented!("read_write_count")
	}

	fn reset_read_write_count(&mut self) {
		unimplemented!("reset_read_write_count")
	}

	fn set_whitelist(&mut self, _: Vec<Vec<u8>>) {
		unimplemented!("set_whitelist")
	}
}
