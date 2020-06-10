use substrate_test_runner::{test, rpc, prelude::*};
use jsonrpc_core::futures::Future;
use node_runtime::Runtime;


#[test]
fn should_run_off_chain_worker() {
    let mut test = test::deterministic(
        test::node(Runtime)
            // TODO [ToDr] This does not work properly, since we have a shared logger.
            .cli_param("-lsc_offchain=trace") 
            .with_sudo(Keyring::Alice)
            .with_genesis_state(|| {
                ...
            })
            .start()
    );

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
    // given
    let mut test = test::deterministic(Runtime.into());
    // test.send_transaction()
    //     .to_module("System")
    //     .call("set_heap_pages")
    //     .origin(Root)
    //     .send();

    // test.with_state(Read::Memory(&map), Write::Sudo, || {
    //   my_pallet::test_conditions(1)
    // });
    //
    // let balance_call = balances_helper().tranfser(Alice, Bob);
    // test.read_state(|| {
    //     <Runtime as CreateTransaction>::create_transaction(
    //         balance_call,
    //         signer,
    //         account,
    //         nonce,
    //     )
    // });

    // when
    test.produce_blocks(5_u32);
    test.send_transfer(Alice, Bob);
    test.with_state(|| {
    // test.with_state(Read::External, Write::Memory(&mut storage), || {
        let events = frame_system::Module::<Runtime>::events();
        assert_eq!(events.len(), 1);

        let events = frame_system::Module::<Runtime>::events();
        assert_eq!(events.len(), 1);
    });

    // when
    test.produce_blocks(5_u32);

    // then
    test.with_state(|| {
    // test.with_state(Read::External, Write::Memory(&mut storage), || {
        let events = frame_system::Module::<Runtime>::events();
        assert_eq!(events.len(), 0);

        let events = frame_system::Module::<Runtime>::events();
        assert_eq!(events.len(), 0);
    });
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

