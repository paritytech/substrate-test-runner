use futures::compat::Future01CompatExt;
use manual_seal::consensus::{babe::BabeConsensusDataProvider, ConsensusDataProvider};
use pallet_balances::Call as BalancesCall;
use parity_scale_codec::alloc::sync::Arc;
use polkadot_runtime::{Runtime, SignedExtra};
use polkadot_service::{chain_spec::polkadot_development_config_genesis, PolkadotChainSpec};
use sc_consensus_babe::BabeBlockImport;
use sc_finality_grandpa::GrandpaBlockImport;
use sc_service::{new_full_parts, ChainType, Configuration, TFullBackend, TFullClient, TaskManager};
use sp_api::TransactionFor;
use sp_consensus_babe::AuthorityId;
use sp_core::crypto::{AccountId32, Pair};
use sp_inherents::InherentDataProviders;
use sp_keyring::sr25519::Keyring::Alice;
use sp_keyring::Sr25519Keyring;
use sp_keystore::SyncCryptoStorePtr;
use sp_runtime::generic::Era;
use sp_runtime::{traits::IdentifyAccount, MultiSigner};
use std::str::FromStr;
use substrate_test_runner::{
	node::{StateProvider, TestRuntimeRequirements},
	prelude::*,
	rpc, test,
};

struct Node;

type BlockImport<B, BE, C, SC> = BabeBlockImport<B, C, GrandpaBlockImport<BE, B, C, SC>>;

impl TestRuntimeRequirements for Node {
	type Block = polkadot_core_primitives::Block;
	type Executor = polkadot_service::PolkadotExecutor;
	type Runtime = polkadot_runtime::Runtime;
	type RuntimeApi = polkadot_runtime::RuntimeApi;
	type SelectChain = sc_consensus::LongestChain<TFullBackend<Self::Block>, Self::Block>;
	type BlockImport = BlockImport<
		Self::Block,
		TFullBackend<Self::Block>,
		TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>,
		Self::SelectChain,
	>;
	type SignedExtension = SignedExtra;

	fn load_spec() -> Result<Box<dyn sc_service::ChainSpec>, String> {
		let wasm_binary = polkadot_runtime::WASM_BINARY.ok_or("Polkadot development wasm not available")?;

		Ok(Box::new(PolkadotChainSpec::from_genesis(
			"Development",
			"polkadot",
			ChainType::Development,
			move || polkadot_development_config_genesis(wasm_binary),
			vec![],
			None,
			Some("dot"),
			None,
			Default::default(),
		)))
	}

	// fn base_path() -> Option<&'static str> {
	// 	Some("/home/seun/.local/share/polkadot")
	// }

	fn signed_extras<S>(state: &S, from: <Self::Runtime as frame_system::Trait>::AccountId) -> Self::SignedExtension
	where
		S: StateProvider,
	{
		let nonce = state.with_state(|| frame_system::Module::<Self::Runtime>::account_nonce(from));

		(
			frame_system::CheckSpecVersion::<Self::Runtime>::new(),
			frame_system::CheckTxVersion::<Self::Runtime>::new(),
			frame_system::CheckGenesis::<Self::Runtime>::new(),
			frame_system::CheckMortality::<Self::Runtime>::from(Era::Immortal),
			frame_system::CheckNonce::<Self::Runtime>::from(nonce),
			frame_system::CheckWeight::<Self::Runtime>::new(),
			pallet_transaction_payment::ChargeTransactionPayment::<Self::Runtime>::from(0),
			polkadot_runtime_common::claims::PrevalidateAttests::<Self::Runtime>::new(),
		)
	}

	fn create_client_parts(
		config: &Configuration,
	) -> Result<
		(
			Arc<TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>>,
			Arc<TFullBackend<Self::Block>>,
			SyncCryptoStorePtr,
			TaskManager,
			InherentDataProviders,
			Option<
				Box<
					dyn ConsensusDataProvider<
						Self::Block,
						Transaction = TransactionFor<
							TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>,
							Self::Block,
						>,
					>,
				>,
			>,
			Self::SelectChain,
			Self::BlockImport,
		),
		sc_service::Error,
	> {
		let (client, backend, keystore, task_manager) =
			new_full_parts::<Self::Block, Self::RuntimeApi, Self::Executor>(config)?;
		let client = Arc::new(client);

		let inherent_providers = InherentDataProviders::new();
		let select_chain = sc_consensus::LongestChain::new(backend.clone());

		let (grandpa_block_import, ..) =
			sc_finality_grandpa::block_import(client.clone(), &(client.clone() as Arc<_>), select_chain.clone())?;

		let (block_import, babe_link) = sc_consensus_babe::block_import(
			sc_consensus_babe::Config::get_or_compute(&*client)?,
			grandpa_block_import,
			client.clone(),
		)?;

		let consensus_data_provider = BabeConsensusDataProvider::new(
			client.clone(),
			keystore.sync_keystore(),
			&inherent_providers,
			babe_link.epoch_changes().clone(),
			vec![(AuthorityId::from(Alice.public()), 1000)],
		)
		.expect("failed to create ConsensusDataProvider");

		Ok((
			client,
			backend,
			keystore.sync_keystore(),
			task_manager,
			inherent_providers,
			Some(Box::new(consensus_data_provider)),
			select_chain,
			block_import,
		))
	}
}

#[test]
fn should_run_off_chain_worker() {
	let mut test = test::deterministic::<Node>();

	let chain_client = test.rpc::<rpc::ChainClient<Runtime>>();
	let rpc_client = test.raw_rpc();

	test.compat_runtime().borrow_mut().block_on_std(async {
		// TODO [ToDr] This should be even rawer - allowing to pass JSON call,
		// which in turn could be collected from the UI.
		let header = rpc_client
			.call_method("chain_getHeader", rpc::Params::Array(vec![]))
			.compat()
			.await;
		println!("{:?}", header);

		let header = chain_client.header(None).compat().await.unwrap();
		println!("{:?}", header);
	});

	test.produce_blocks(15);

	test.assert_log_line("best = true");
}

#[test]
fn should_read_and_write_state() {
	// given
	let mut test = test::deterministic::<Node>();

	type Balances = pallet_balances::Module<Runtime>;

	test.produce_blocks(1);

	let alice = Sr25519Keyring::Alice.pair();
	let alice_account_id = MultiSigner::from(alice.public()).into_account();
	let account_id = AccountId32::from_str("1rvXMZpAj9nKLQkPFCymyH7Fg3ZyKJhJbrc7UtHbTVhJm1A").unwrap();

	let old_balance = test.with_state(|| Balances::free_balance(account_id.clone()));

	println!("\n\nold_balance: {:?}\n\n\n", old_balance);

	test.send_extrinsic(
		BalancesCall::transfer(account_id.clone(), 7825388000000),
		alice_account_id,
	)
	.unwrap();

	test.produce_blocks(1);

	let new_balance = test.with_state(|| Balances::free_balance(account_id));

	println!("\n\nnew_balance: {:?}\n\n\n", new_balance);

	assert_eq!(old_balance + 7825388000000, new_balance);
	// todo should probably have an api for deleting blocks.
}

#[test]
fn elections_migration() {}

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
