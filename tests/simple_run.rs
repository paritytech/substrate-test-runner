use substrate_test_runner::{test, rpc::{self, RpcExtension}, SubstrateTest};
use jsonrpc_core::futures::Future;
use node_runtime::Runtime;


#[test]
fn should_run_off_chain_worker() {
    let node = test::node(Runtime)
        // TODO [ToDr] This does not work properly, since we have a shared logger.
        .cli_param("-lsc_offchain=trace") 
        .start();
    let mut test = test::deterministic(node);

    let chain_client = test.rpc::<rpc::ChainClient<Runtime>>();
    let rpc_client = test.raw_rpc();

    // TODO [ToDr] This should be even rawer - allowing to pass JSON call,
    // which in turn could be collected from the UI.
    let header = rpc_client.call_method(
        "chain_getHeader",
        rpc::Params::Array(vec![]),
    ).wait();
    println!("{:?}", header);

    let header = chain_client.header(None).wait().unwrap();
    println!("{:?}", header);

    test.wait_for_block(15_u32);

    test.assert_log_line("db", "best = true");
}

#[test]
fn should_read_state() {
    let node = test::node(Runtime)
        .start();
    let mut test = test::deterministic(node);
    test.wait_for_block(5_u32);
}

#[test]
fn external_black_box() {
    let test = test::blackbox_external("ws://127.0.0.1:3001", Runtime);
    test.wait_blocks(5_u32);
}


// Check state using decl_storage
// Assert a log line
// Initially start with a "runtime example"
// Customize the runtime somehow 
//  $ cp node/runtime /tmp/temp_runtime
//  $ sed -../

