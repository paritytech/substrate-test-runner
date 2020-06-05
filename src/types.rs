// TODO [ToDr] This is a bit shit - perhaps would be good to let it be more configurable.
// based on the runtime maybe?

pub type BlockNumber<T> = <T as frame_system::Trait>::BlockNumber;
pub type BlockHash<T> = <T as frame_system::Trait>::Hash;
pub type BlockHashing<T> = <T as frame_system::Trait>::Hashing;
pub type Header<T> = sp_runtime::generic::Header<BlockNumber<T>, BlockHashing<T>>;
pub type Block<T> = sp_runtime::generic::Block<Header<T>, sp_runtime::OpaqueExtrinsic>;
pub type SignedBlock<T> = sp_runtime::generic::SignedBlock<Block<T>>;
