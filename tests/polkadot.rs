use futures::compat::Future01CompatExt;
use pallet_balances::Call as BalancesCall;
use frame_system::Call as SystemCall;
use pallet_democracy::Call as DemocracyCall;
use polkadot_runtime::{Runtime, SignedExtra};
use sp_core::crypto::{Pair, AccountId32};
use sp_keyring::Sr25519Keyring;
use sp_runtime::{traits::IdentifyAccount, MultiSigner};
use substrate_test_runner::{prelude::*, rpc, test, node::{TestRuntimeRequirements, StateProvider}};
use sc_service::{TFullClient, new_full_parts, TFullBackend, TaskManager, Configuration, ChainType};
use parity_scale_codec::alloc::sync::Arc;
use sc_keystore::KeyStorePtr;
use sp_inherents::InherentDataProviders;
use sc_consensus_babe::BabeBlockImport;
use sc_finality_grandpa::GrandpaBlockImport;
use manual_seal::consensus::{ConsensusDataProvider,	babe::BabeConsensusDataProvider};
use sp_api::TransactionFor;
use sp_consensus_babe::AuthorityId;
use sp_keyring::sr25519::Keyring::Alice;
use polkadot_service::{PolkadotChainSpec, chain_spec::polkadot_development_config_genesis};
use sp_runtime::generic::Era;
use std::str::FromStr;

use primitive_types::H256;
use parity_scale_codec::Encode;

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

	fn base_path() -> Option<&'static str> {
		Some("/home/apopiak/backup_db/")
	}

	fn signed_extras<S>(
		state: &S,
		from: <Self::Runtime as frame_system::Trait>::AccountId,
	) -> Self::SignedExtension
	where
		S: StateProvider
	{
		let nonce = state.with_state(|| {
			frame_system::Module::<Self::Runtime>::account_nonce(from)
		});

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
fn do_runtime_migration() {
	use frame_support::{StorageValue, StorageMap};

	// given
	let mut test = test::deterministic::<Node>();

	test.revert_blocks(5).expect("initial reverting failed");

	type Balances = pallet_balances::Module<Runtime>;

	let whale_str = "12dfEn1GycUmHtfEDW3BuQYzsMyUR1PqUPH2pyEikYeUX59o";
	let whale = AccountId32::from_str(whale_str).unwrap();

	let orig_whale_account = test.with_state(|| {
		frame_system::Account::<Runtime>::get(whale.clone())
	});

	test.produce_blocks(1);

	// let bytes = include_bytes!("/home/apopiak/remote-builds/polkadot/target/release/wbuild/polkadot-runtime/polkadot_runtime.compact.wasm").to_vec();
	let bytes = new_polkadot_runtime::WASM_BINARY.expect("WASM runtime needs to be available.").to_vec();

	test.with_state(|| {
		use sp_core::storage::well_known_keys;
		frame_support::storage::unhashed::put_raw(well_known_keys::CODE, &bytes);

		frame_system::LastRuntimeUpgrade::set(None);
	});

	test.produce_blocks(1);

	// try a balance transfer after the upgrade
	let account_id = AccountId32::from_str("1rvXMZpAj9nKLQkPFCymyH7Fg3ZyKJhJbrc7UtHbTVhJm1A").unwrap();

	let whale_account = test.with_state(|| {
		use new_frame_support::StorageMap;

		let new_whale = new_sp_core::crypto::AccountId32::from_str(whale_str).unwrap();

		sp_externalities::with_externalities(|ext| {
					/// Information of an account.
			#[derive(Clone, Eq, PartialEq, Default, Debug, Encode, Decode)]
			pub struct AccountInfo<Index, RefCount, AccountData> {
				/// The number of transactions this account has sent.
				pub nonce: Index,
				/// The number of other modules that currently depend on this account's existence. The account
				/// cannot be reaped until this is zero.
				pub refcount: RefCount,
				/// The additional data that belongs to this account. Used to store the balance(s) in a lot of
				/// chains.
				pub data: AccountData,
			}

			use parity_scale_codec::Decode;
			let key = frame_system::Account::<Runtime>::hashed_key_for(whale.clone());
			let raw = ext.storage(&key).expect("account should be present");
			println!("raw: {:?}", raw);
			let acc = AccountInfo::<u32, u8, pallet_balances::AccountData<u128>>::decode(&mut &raw[..]);
			println!("acc: {:?}", acc);
			let new_key = new_frame_system::Account::<new_polkadot_runtime::Runtime>::hashed_key_for(new_whale.clone());
			let new_raw = ext.storage(&new_key).expect("account should be present");
			println!("raw new: {:?}", new_raw);
			let new_acc = new_frame_system::AccountInfo::<u32, new_pallet_balances::AccountData<u128>>::decode(&mut &new_raw[..]);
			println!("acc new: {:?}", new_acc);
		}).expect("externalities should be present");
		
		frame_system::Account::<Runtime>::get(whale.clone())
	});

	println!("new whale account: {:?}", whale_account);

	test.revert_blocks(2).expect("final reverting failed");

	assert!(false);
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
