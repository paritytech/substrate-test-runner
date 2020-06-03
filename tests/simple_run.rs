use substrate_test_runner::{SubstrateNode, Consensus};
use substrate_test_runner::rpc::{self, RpcExtension};
use jsonrpc_core::futures::Future;

#[test]
fn should_run_off_chain_worker() {
    let mut node = SubstrateNode::builder()
        .cli_param("-lrpc=trace")
        .consensus(Consensus::Manual)
        .start();

    println!("Requsting RPC clients");
    let chain_client = node.rpc::<rpc::ChainClient>();
    println!("Got 1");
    let rpc_client = node.raw_rpc();

    println!("Got RPC clients");
    // TODO [ToDr] This should be even rawer - allowing to pass JSON call,
    // which in turn could be collected from the UI.
    let header = rpc_client.call_method(
        "chain_getHeader",
        rpc::Params::Array(vec![]),
    ).wait();
    println!("{:?}", header);

    let header = chain_client.header(None).wait().unwrap();
    println!("{:?}", header);
}

