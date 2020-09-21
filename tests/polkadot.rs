use frame_system::offchain::{SendSignedTransaction, Signer};
use futures::compat::Future01CompatExt;
use pallet_balances::Call as BalancesCall;
use polkadot_runtime::{Runtime, UncheckedExtrinsic, SignedExtra};
use sp_core::crypto::{Pair, AccountId32};
use sp_keyring::Sr25519Keyring;
use sp_runtime::{traits::{IdentifyAccount, Extrinsic}, MultiSigner, MultiSignature};
use substrate_test_runner::{
	prelude::*, rpc, test, node::{InternalNode, TestRuntimeRequirements},
};
use sc_service::{TFullClient, new_full_parts, TFullBackend, TaskManager, Configuration, ChainType};
use parity_scale_codec::alloc::sync::Arc;
use sc_keystore::KeyStorePtr;
use sp_inherents::InherentDataProviders;
use sc_consensus_babe::BabeBlockImport;
use sc_finality_grandpa::GrandpaBlockImport;
use manual_seal::consensus::{ConsensusDataProvider,	babe::BabeConsensusDataProvider};
use sp_api::TransactionFor;
use sp_application_crypto::sr25519;
use sp_consensus_babe::AuthorityId;
use sp_keyring::sr25519::Keyring::Alice;
use polkadot_service::{PolkadotChainSpec, chain_spec::polkadot_development_config_genesis};
use sp_runtime::generic::Era;
use parity_scale_codec::Encode;
use std::str::FromStr;

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

	fn base_path() -> Option<&'static str> {
		Some("/home/seun/.local/share/polkadot")
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
			vec![(AuthorityId::from(Alice.public()), 1000)]
		)
		.expect("failed to create ConsensusDataProvider");

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

	test.produce_blocks(1);

	let alice = Sr25519Keyring::Alice.pair();
	let alice_account_id = MultiSigner::from(alice.public()).into_account();
	let call = BalancesCall::transfer(alice_account_id.clone().into(), 7825388000000);
	// random address on chain.
	let address = AccountId32::from_str("1rvXMZpAj9nKLQkPFCymyH7Fg3ZyKJhJbrc7UtHbTVhJm1A").unwrap();

	let (account_nonce, account_balance) = test.with_state(|| {
		(
			frame_system::Module::<Runtime>::account_nonce(address.clone()),
			Balances::free_balance(address.clone())
		)
	});

	println!("\n\naccount_nonce: {:?}\n\n\n\n\naccount_balance: {:?}\n\n\n", account_nonce, account_balance);

	let extra: SignedExtra = test.with_state(|| {
		(
			frame_system::CheckSpecVersion::<Runtime>::new(),
			frame_system::CheckTxVersion::<Runtime>::new(),
			frame_system::CheckGenesis::<Runtime>::new(),
			frame_system::CheckMortality::<Runtime>::from(Era::Immortal),
			frame_system::CheckNonce::<Runtime>::from(account_nonce),
			frame_system::CheckWeight::<Runtime>::new(),
			pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(0),
			polkadot_runtime_common::claims::PrevalidateAttests::<Runtime>::new(),
		)
	});
	let signed_data = Some(((address, Default::default(), extra)));
	let extrinsic = UncheckedExtrinsic::new(call.into(), signed_data).unwrap();
	let bytes = extrinsic.encode();
	let client = test.rpc_client::<rpc::AuthorClient<Runtime>>();
	test.compat_runtime().block_on_std(async move {
		let result = client.submit_extrinsic(bytes.into())
			.compat()
			.await;

		println!("\n\ntransaction: {:?}\n\n\n", result);
	});

	test.produce_blocks(1);

	let alice_balance = test.with_state(|| {
		Balances::free_balance(alice_account_id)
	});

	println!("\n\nalice_balance: {:?}\n\n\n", alice_balance);

	// todo should probably have an api for deleting blocks.
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
