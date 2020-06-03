use crate::types;

pub type ChainClient = sc_rpc_api::chain::ChainClient<
    types::BlockNumber,
    types::BlockHash,
    types::Header,
    types::SignedBlock,
>;
