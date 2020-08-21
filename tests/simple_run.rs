use frame_system::offchain::{SendSignedTransaction, Signer};
use futures::compat::Future01CompatExt;
use pallet_balances::Call as BalancesCall;
use runtime::{Runtime, RuntimeKeyType};
use sp_core::crypto::Pair;
use sp_keyring::Sr25519Keyring;
use sp_runtime::{traits::IdentifyAccount, MultiSigner};
use substrate_test_runner::{
	prelude::*, rpc, test, node::{InternalNode, TestRuntimeRequirements},
	chain_spec::spec_factory,
};
use sc_executor::native_executor_instance;

// Declare an instance of the native executor named `Executor`. Include the wasm binary as the
// equivalent wasm code.
native_executor_instance!(
	pub Executor,
	runtime::api::dispatch,
	runtime::native_version,
);

struct Node;

impl TestRuntimeRequirements for Node {
	type OpaqueBlock = node_primitives::Block;
	type Executor = node_executor::Executor;
	type Runtime = node_runtime::Runtime;
	type RuntimeApi = node_runtime::RuntimeApi;
}

#[test]
fn should_run_off_chain_worker() {
	let node = InternalNode::<Node>::new(spec_factory).unwrap();

	let mut test = test::deterministic(node);
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

		test.assert_log_line("db", "best = true");
	});
}

#[test]
fn should_read_state() {
	// given
	let node = InternalNode::<Node>::new(spec_factory).unwrap();
	let mut test = test::deterministic(node);
	type Balances = pallet_balances::Module<Runtime>;

	test.produce_blocks(1);

	let alice = Sr25519Keyring::Alice.pair();
	let bob = Sr25519Keyring::Bob.pair();
	let (alice_public, bob_public) = (
		alice.public(),
		bob.public(),
	);

	let signer = Signer::<node_runtime::Runtime, RuntimeKeyType>::all_accounts()
		// only use alice' account for signing
		.with_filter(vec![alice.public().into()]);

	let (bob_balance, alice_balance) = test.with_state(|| {
		let events = frame_system::Module::<Runtime>::events();
		(
			Balances::free_balance(MultiSigner::from(bob_public).into_account()),
			Balances::free_balance(MultiSigner::from(alice_public).into_account()),
		)
	});

	let mut result = test.with_state(|| {
		signer.send_signed_transaction(|_account| {
			BalancesCall::transfer(MultiSigner::from(bob_public).into_account().into(), 8900000000000000)
		})
	});

	assert!(result.pop().unwrap().1.is_ok());

	test.produce_blocks(1);

	let (new_bob_balance, new_alice_balance) = test.with_state(|| {
		let events = frame_system::Module::<Runtime>::events();
		log::info!("{:#?}", events);
		(
			Balances::free_balance(MultiSigner::from(bob_public).into_account()),
			Balances::free_balance(MultiSigner::from(alice_public).into_account()),
		)
	});

	// FIXME: alice' balance doesnt change lmao
	log::info!(
		"\n\n{:#?}: before {}, after {}\n\n{:#?}: before {}, after {}\n\n",
		MultiSigner::from(bob_public),
		bob_balance,
		new_bob_balance,
		MultiSigner::from(alice_public),
		alice_balance,
		new_alice_balance
	);

	assert_eq!((new_bob_balance - bob_balance), 8900000000000000);
}

#[test]
fn external_black_box() {
	let test = test::blackbox_external::<Node>("ws://127.0.0.1:3001");
	test.wait_blocks(5_u32);
}

// Check state using decl_storage
// Assert a log line
// Initially start with a "runtime example"
// Customize the runtime somehow
//  $ cp node/runtime /tmp/temp_runtime
//  $ sed -../
