use frame_system::offchain::{SendSignedTransaction, Signer};
use futures::compat::Future01CompatExt;
use pallet_balances::Call as BalancesCall;
use node_runtime::{Runtime, RuntimeApi};
use sp_core::crypto::Pair;
use sp_keyring::Sr25519Keyring;
use sp_runtime::{traits::IdentifyAccount, MultiSigner, MultiSignature};
use substrate_test_runner::{
	prelude::*, rpc, test, node::{InternalNode, TestRuntimeRequirements},
};
use sc_service::{TFullClient, new_full_parts, TFullBackend, TaskManager, Configuration};
use parity_scale_codec::alloc::sync::Arc;
use sc_keystore::KeyStorePtr;
use sp_inherents::InherentDataProviders;
use sc_consensus_babe::BabeBlockImport;
use sc_finality_grandpa::GrandpaBlockImport;
use manual_seal::consensus::{ConsensusDataProvider,	babe::BabeConsensusDataProvider};
use sp_api::TransactionFor;
use sp_application_crypto::sr25519;

struct Node;

type BlockImport<B, BE, C, SC> = BabeBlockImport<B, C, GrandpaBlockImport<BE, B, C, SC>>;

impl TestRuntimeRequirements for Node {
	type Block = node_primitives::Block;
	type Executor = node_executor::Executor;
	type Runtime = Runtime;
	type RuntimeApi = RuntimeApi;
	type SelectChain = sc_consensus::LongestChain<TFullBackend<Self::Block>, Self::Block>;
	type BlockImport = BlockImport<
		Self::Block,
		TFullBackend<Self::Block>,
		TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>,
		Self::SelectChain,
	>;

	fn load_spec() -> Result<Box<dyn sc_service::ChainSpec>, String> {
		Ok(Box::new(node_cli::chain_spec::development_config()))
	}

	fn create_client_parts(config: &Configuration) -> Result<
		(
			Arc<TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>>,
			Arc<TFullBackend<Self::Block>>,
			KeyStorePtr,
			TaskManager,
			InherentDataProviders,
			Option<Box<
				dyn ConsensusDataProvider<
					Self::Block,
					Transaction = TransactionFor<
						TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>,
						Self::Block
					>,
				>
			>>,
			Self::SelectChain,
			Self::BlockImport
		),
		sc_service::Error
	> {
		let (
			client,
			backend,
			keystore,
			task_manager,
		) = new_full_parts::<Self::Block, Self::RuntimeApi, Self::Executor>(config)?;
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
			keystore.clone(),
			&inherent_providers,
			babe_link.epoch_changes().clone(),
		)
		.expect("failed to create DigestProvider");

		Ok((
			client,
			backend,
			keystore,
			task_manager,
			inherent_providers,
			Some(Box::new(consensus_data_provider)),
			select_chain,
			block_import
		))
	}
}

struct Sr25519;

impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for Sr25519 {
	type RuntimeAppPublic = sr25519::AppPublic;
	type GenericPublic = sp_core::sr25519::Public;
	type GenericSignature = sp_core::sr25519::Signature;
}


#[test]
fn should_run_off_chain_worker() {
	let node = InternalNode::<Node>::new().unwrap();
	let mut test = test::deterministic(node);

	let chain_client = test.rpc::<rpc::ChainClient<Runtime>>();
	let rpc_client = test.raw_rpc();

	test.compat_runtime().block_on_std(async {
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
fn should_read_state() {
	// given
	let node = InternalNode::<Node>::new().unwrap();
	let mut test = test::deterministic(node);

	type Balances = pallet_balances::Module<Runtime>;

	// test.produce_blocks(1);
	//
	// let alice = Sr25519Keyring::Alice.pair();
	// let bob = Sr25519Keyring::Bob.pair();
	// let bob_public = bob.public();
	//
	// let signer = Signer::<Runtime, Sr25519>::all_accounts()
	// 	// only use alice' account for signing
	// 	.with_filter(vec![alice.public().into()]);
	//
	// let bob_balance = test.with_state(|| {
	// 	Balances::free_balance(MultiSigner::from(bob_public).into_account())
	// });

	// let mut result = test.with_state(|| {
	// 	signer.send_signed_transaction(|_account| {
	// 		BalancesCall::transfer(MultiSigner::from(bob_public).into_account().into(), 8900000000000000)
	// 	})
	// });
	//
	// assert!(result.pop().unwrap().1.is_ok());

	test.produce_blocks(220);
	//
	// let new_bob_balance = test.with_state(|| {
	// 	Balances::free_balance(MultiSigner::from(bob_public).into_account())
	// });
	//
	// assert_eq!((new_bob_balance - bob_balance), 8900000000000000);
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
