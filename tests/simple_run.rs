use substrate_test_runner::{SubstrateNode, Consensus, RawRpcClient, rpc};

#[test]
fn should_run_off_chain_worker() {
    let mut node = SubstrateNode::builder()
        .cli_param("-lrpc=trace")
        .consensus(Consensus::Manual)
        .start();

    let chain_client = node.rpc::<rpc::ChainClient>();
    let rpc_client = node.raw_rpc();

}

