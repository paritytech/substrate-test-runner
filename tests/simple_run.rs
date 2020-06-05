use substrate_test_runner::{SubstrateNode, Consensus};
use substrate_test_runner::rpc::{self, RpcExtension};
use jsonrpc_core::futures::Future;
use node_runtime::Runtime;


#[test]
fn should_run_off_chain_worker() {
    let mut node = SubstrateNode::builder(Runtime)
        // TODO [ToDr] This does not work properly, since we have a shared logger.
        .cli_param("-lsc_offchain=trace") 
        .consensus(Consensus::Manual)
        .start();

    let chain_client = node.rpc::<rpc::ChainClient<Runtime>>();
    let rpc_client = node.raw_rpc();

    // TODO [ToDr] This should be even rawer - allowing to pass JSON call,
    // which in turn could be collected from the UI.
    let header = rpc_client.call_method(
        "chain_getHeader",
        rpc::Params::Array(vec![]),
    ).wait();
    println!("{:?}", header);

    let header = chain_client.header(None).wait().unwrap();
    println!("{:?}", header);

    node.wait_for_block(15_u32);

    node.assert_log_line("db", "best = true");
}

#[test]
fn should_read_state() {
    let mut node = SubstrateNode::builder(Runtime)
        .start();
    node.wait_for_block(5_u32);
}

// Check state using decl_storage
// Assert a log line
// Initially start with a "runtime example"
// Customize the runtime somehow 
//  $ cp node/runtime /tmp/temp_runtime
//  $ sed -../

