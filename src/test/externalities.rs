use crate::rpc;
use std::any::{TypeId, Any};
use sp_externalities::{Extension, Error};
use sp_storage::{ChildInfo, StorageKey};
use futures01::Future;

pub struct TestExternalities<Runtime: frame_system::Trait> {
    client: rpc::StateClient<Runtime>,
}

impl<Runtime: frame_system::Trait> TestExternalities<Runtime> {
    pub fn new(client: rpc::StateClient<Runtime>) -> Self {
        Self { client }
    }

	pub fn execute_with<R>(&mut self, execute: impl FnOnce() -> R) -> R {
		sp_externalities::set_and_run_with_externalities(self, execute)
	}
}

// TODO [ToDr] Most likely the implementation is not really relevant, but we still need the trait.
impl<Runtime: frame_system::Trait> sp_externalities::ExtensionStore for TestExternalities<Runtime> {
	fn extension_by_type_id(&mut self, _type_id: TypeId) -> Option<&mut dyn Any> {
        todo!()
    }

	fn register_extension_with_type_id(&mut self, _type_id: TypeId, _extension: Box<dyn Extension>) -> Result<(), Error> {
        todo!()
    }

	fn deregister_extension_by_type_id(&mut self, _type_id: TypeId) -> Result<(), Error> {
        todo!()
    }
}


impl<Runtime: frame_system::Trait> sp_externalities::Externalities for TestExternalities<Runtime> {
	fn set_offchain_storage(&mut self, _key: &[u8], _value: Option<&[u8]>) { todo!() }

	fn storage(&self, key: &[u8]) -> Option<Vec<u8>> {
		// this is pretty weird, but stay with me.
		// the tests in `simple_run` is wrapped with a tokio runtime
		// so this means the code path here has access to the tokio v0.1 runtime
		// requried for this future to complete, without the runtime, this call would panic.
		self.client.storage(StorageKey(key.to_vec()), None)
			.wait()
			.ok()
			.flatten()
			.map(|data| data.0)
    }

	fn storage_hash(&self, _key: &[u8]) -> Option<Vec<u8>> { todo!() }

	fn child_storage_hash(
		&self,
		_child_info: &ChildInfo,
		_key: &[u8],
	) -> Option<Vec<u8>> { todo!() }

	fn child_storage(
		&self,
		_child_info: &ChildInfo,
		_key: &[u8],
	) -> Option<Vec<u8>> { todo!() }

	fn next_storage_key(&self, _key: &[u8]) -> Option<Vec<u8>> { todo!() }

	fn next_child_storage_key(
		&self,
		_child_info: &ChildInfo,
		_key: &[u8],
	) -> Option<Vec<u8>> { todo!() }

	fn kill_child_storage(&mut self, _child_info: &ChildInfo) { todo!() }

	fn clear_prefix(&mut self, _prefix: &[u8]) { todo!() }

	fn clear_child_prefix(
		&mut self,
		_child_info: &ChildInfo,
		_prefix: &[u8],
	) { todo!() }

	fn place_storage(&mut self, _key: Vec<u8>, _value: Option<Vec<u8>>) {
        // Create a sudo transaction that alters storage on-chain.
        todo!()
    }

	fn place_child_storage(
		&mut self,
		_child_info: &ChildInfo,
		_key: Vec<u8>,
		_value: Option<Vec<u8>>,
	) { todo!() }

	fn chain_id(&self) -> u64 { todo!() }

	fn storage_root(&mut self) -> Vec<u8> { todo!() }

	fn child_storage_root(
		&mut self,
		_child_info: &ChildInfo,
	) -> Vec<u8> { todo!() }

	fn storage_append(
		&mut self,
		_key: Vec<u8>,
		_value: Vec<u8>,
	) { todo!() }

	fn storage_changes_root(&mut self, _parent: &[u8]) -> Result<Option<Vec<u8>>, ()> { todo!() }

	fn storage_start_transaction(&mut self) {
		todo!()
	}

	fn storage_rollback_transaction(&mut self) -> Result<(), ()> {
		todo!()
	}

	fn storage_commit_transaction(&mut self) -> Result<(), ()> {
		todo!()
	}

	fn wipe(&mut self) { todo!() }

	fn commit(&mut self) { todo!() }

	fn read_write_count(&self) -> (u32, u32, u32, u32) {
		todo!()
	}

	fn reset_read_write_count(&mut self) {
		todo!()
	}

	fn set_whitelist(&mut self, _: Vec<Vec<u8>>) {
		todo!()
	}
}
