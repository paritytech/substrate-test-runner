// TODO [ToDr] This is a bit shit - perhaps would be good to let it be more configurable.
// based on the runtime maybe?

pub type BlockNumber = u64;
pub type BlockHash = sp_runtime::traits::BlakeTwo256;
pub type Header = sp_runtime::generic::Header<BlockNumber, BlockHash>;
pub type Block = sp_runtime::generic::Block<Header, sp_runtime::OpaqueExtrinsic>;
pub type SignedBlock = sp_runtime::generic::SignedBlock<Block>;
