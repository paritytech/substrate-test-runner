use frame_system::offchain::{SendSignedTransaction, Signer};
use futures::compat::Future01CompatExt;
use pallet_balances::Call as BalancesCall;
use pallet_indices::address::Address;
use runtime::{Runtime, RuntimeKeyType};
use sp_core::crypto::Pair;
use sp_keyring::Sr25519Keyring;
use sp_runtime::{traits::IdentifyAccount, MultiSigner};
use substrate_subxt::{balances::TransferCallExt, ClientBuilder, DefaultNodeRuntime, PairSigner};
use substrate_test_runner::{prelude::*, rpc, subxt, test};
#[test]
fn should_run_off_chain_worker() {
	let mut test = test::deterministic(
		test::node::<Runtime>()
			// TODO [ToDr] This does not work properly, since we have a shared logger.
			.cli_param("-lsc_offchain=trace")
			// .with_sudo(Keyring::Alice)
			// .with_genesis_state(|| {
			//     ...
			// })
			.start(),
	);
	let mut runtime = tokio_compat::runtime::Runtime::new().unwrap();
	runtime.block_on_std(async {
		let chain_client = test.rpc::<rpc::ChainClient<Runtime>>();
		let rpc_client = test.raw_rpc();

		// TODO [ToDr] This should be even rawer - allowing to pass JSON call,
		// which in turn could be collected from the UI.
		let header = rpc_client
			.call_method("chain_getHeader", rpc::Params::Array(vec![]))
			.compat()
			.await;
		println!("{:?}", header);

		let header = chain_client.header(None).compat().await.unwrap();
		println!("{:?}", header);

		test.produce_blocks(15);

		// test.assert_log_line("db", "best = true");
	});
}

#[test]
fn should_read_state() {
	// given
	let mut test = test::deterministic(test::node::<Runtime>().start());
	type Balances = pallet_balances::Module<Runtime>;

	// test.produce_blocks(1);

	let alice = Sr25519Keyring::Alice.pair();
	let bob = Sr25519Keyring::Bob.pair();

	let signer = Signer::<Runtime, RuntimeKeyType>::all_accounts()
		// only use alice' account.
		.with_filter(vec![alice.public().into()]);

	let alice_balance = test.with_state(|| Balances::free_balance(MultiSigner::from(alice.public()).into_account()));

	let result = test.with_state(|| {
		signer.send_signed_transaction(|_| {
			BalancesCall::transfer(
				Address::from(MultiSigner::from(bob.public()).into_account()),
				8900000000000000,
			)
		})
	});

	log::info!("ext result: {:#?}", result);

	test.produce_blocks(1);

	let new_alice_balance =
		test.with_state(|| Balances::free_balance(MultiSigner::from(alice.public()).into_account()));

	// account for fees
	assert!((alice_balance - new_alice_balance) > 8900000000000000);
}

#[test]
fn external_black_box() {
	let test = test::blackbox_external::<Runtime>("ws://127.0.0.1:3001");
	test.wait_blocks(5_u32);
}

// Check state using decl_storage
// Assert a log line
// Initially start with a "runtime example"
// Customize the runtime somehow
//  $ cp node/runtime /tmp/temp_runtime
//  $ sed -../
