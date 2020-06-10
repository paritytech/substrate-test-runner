use crate::rpc;

pub struct TestExternalities<TRuntime: frame_system::Trait> {
    client: rpc::StateClient<TRuntime>,
}

impl<TRuntime: frame_system::Trait> TestExternalities<TRuntime> {
    pub fn new(client: rpc::StateClient<TRuntime>) -> Self {
        Self { client }
    }

	pub fn ext(&mut self) -> Ext {
		Ext
	}

	pub fn execute_with<R>(&mut self, execute: impl FnOnce() -> R) -> R {
		let mut ext = self.ext();
		sp_externalities::set_and_run_with_externalities(&mut ext, execute)
	}
}

pub struct Ext;

use std::any::{TypeId, Any};
use sp_externalities::{Extension, Error};
use sp_storage::ChildInfo;

// TODO [ToDr] Most likely the implementation is not really relevant, but we still need the trait.
impl sp_externalities::ExtensionStore for Ext {
	fn extension_by_type_id(&mut self, type_id: TypeId) -> Option<&mut dyn Any> {
        todo!()
    }

	fn register_extension_with_type_id(&mut self, type_id: TypeId, extension: Box<dyn Extension>) -> Result<(), Error> {
        todo!()
    }

	fn deregister_extension_by_type_id(&mut self, type_id: TypeId) -> Result<(), Error> {
        todo!()
    }
}


impl sp_externalities::Externalities for Ext {
	fn set_offchain_storage(&mut self, key: &[u8], value: Option<&[u8]>) { todo!() }

	fn storage(&self, key: &[u8]) -> Option<Vec<u8>> {
        println!("Reading: {:?}", std::str::from_utf8(key));
        None
    }

	fn storage_hash(&self, key: &[u8]) -> Option<Vec<u8>> { todo!() }

	fn child_storage_hash(
		&self,
		child_info: &ChildInfo,
		key: &[u8],
	) -> Option<Vec<u8>> { todo!() }

	fn child_storage(
		&self,
		child_info: &ChildInfo,
		key: &[u8],
	) -> Option<Vec<u8>> { todo!() }

	fn next_storage_key(&self, key: &[u8]) -> Option<Vec<u8>> { todo!() }

	fn next_child_storage_key(
		&self,
		child_info: &ChildInfo,
		key: &[u8],
	) -> Option<Vec<u8>> { todo!() }

	fn kill_child_storage(&mut self, child_info: &ChildInfo) { todo!() }

	fn clear_prefix(&mut self, prefix: &[u8]) { todo!() }

	fn clear_child_prefix(
		&mut self,
		child_info: &ChildInfo,
		prefix: &[u8],
	) { todo!() }

	fn place_storage(&mut self, key: Vec<u8>, value: Option<Vec<u8>>) { todo!() }

	fn place_child_storage(
		&mut self,
		child_info: &ChildInfo,
		key: Vec<u8>,
		value: Option<Vec<u8>>,
	) { todo!() }

	fn chain_id(&self) -> u64 { todo!() }

	fn storage_root(&mut self) -> Vec<u8> { todo!() }

	fn child_storage_root(
		&mut self,
		child_info: &ChildInfo,
	) -> Vec<u8> { todo!() }

	fn storage_append(
		&mut self,
		key: Vec<u8>,
		value: Vec<u8>,
	) { todo!() }

	fn storage_changes_root(&mut self, parent: &[u8]) -> Result<Option<Vec<u8>>, ()> { todo!() }

	fn wipe(&mut self) { todo!() }

	fn commit(&mut self) { todo!() }
}
