#![allow(dead_code)]
use futures::compat::Future01CompatExt;
use manual_seal::consensus::{babe::BabeConsensusDataProvider, ConsensusDataProvider};
use pallet_balances::Call as BalancesCall;
use parity_scale_codec::alloc::sync::Arc;
use polkadot_runtime::{Runtime, SignedExtra, FastTrackVotingPeriod};
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

use primitive_types::H256;
use parity_scale_codec::{Encode, Decode};
use hex::ToHex;
use polkadot_core_primitives::AccountId;
use sp_blockchain::HeaderBackend;
use frame_support::dispatch::GetDispatchInfo;
use frame_system::EventRecord;

struct Node;

type BlockImport<B, BE, C, SC> = BabeBlockImport<B, C, GrandpaBlockImport<BE, B, C, SC>>;

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
		Some("/home/seunlanlege/polkadot")
	}

	fn signed_extras<S>(state: &S, from: <Self::Runtime as frame_system::Trait>::AccountId) -> Self::SignedExtension
	where
		S: StateProvider,
	{
		let nonce = state.with_state(|| {
			use frame_support::StorageMap;
			sp_externalities::with_externalities(|ext| {
				let key = frame_system::Account::<Runtime>::hashed_key_for(from.clone());
				let raw = ext.storage(&key).expect("account should be present");
				let acc = AccountInfo::<u32, u8, pallet_balances::AccountData<u128>>::decode(&mut &raw[..]).unwrap();
				acc.nonce
			}).unwrap()
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

// #[test]
// fn do_runtime_migration() {
// 	use frame_support::{StorageValue, StorageMap};
//
// 	// given
// 	let mut test = test::deterministic::<Node>();
//
// 	test.revert_blocks(5).expect("initial reverting failed");
//
// 	type Balances = pallet_balances::Module<Runtime>;
//
// 	let whale_str = "12dfEn1GycUmHtfEDW3BuQYzsMyUR1PqUPH2pyEikYeUX59o";
// 	let whale = AccountId32::from_str(whale_str).unwrap();
//
// 	let orig_whale_account = test.with_state(|| {
// 		frame_system::Account::<Runtime>::get(whale.clone())
// 	});
//
// 	test.produce_blocks(1);
//
// 	// let bytes = include_bytes!("/home/apopiak/remote-builds/polkadot/target/release/wbuild/polkadot-runtime/polkadot_runtime.compact.wasm").to_vec();
// 	let bytes = new_polkadot_runtime::WASM_BINARY.expect("WASM runtime needs to be available.").to_vec();
//
// 	test.with_state(|| {
// 		use sp_core::storage::well_known_keys;
// 		frame_support::storage::unhashed::put_raw(well_known_keys::CODE, &bytes);
//
// 		frame_system::LastRuntimeUpgrade::set(None);
// 	});
//
// 	test.produce_blocks(1);
//
// 	// try a balance transfer after the upgrade
// 	let account_id = AccountId32::from_str("1rvXMZpAj9nKLQkPFCymyH7Fg3ZyKJhJbrc7UtHbTVhJm1A").unwrap();
//
// 	let whale_account = test.with_state(|| {
// 		use new_frame_support::StorageMap;
//
// 		let new_whale = new_sp_core::crypto::AccountId32::from_str(whale_str).unwrap();
//
// 		sp_externalities::with_externalities(|ext| {
// 					/// Information of an account.
// 			#[derive(Clone, Eq, PartialEq, Default, Debug, Encode, Decode)]
// 			pub struct AccountInfo<Index, RefCount, AccountData> {
// 				/// The number of transactions this account has sent.
// 				pub nonce: Index,
// 				/// The number of other modules that currently depend on this account's existence. The account
// 				/// cannot be reaped until this is zero.
// 				pub refcount: RefCount,
// 				/// The additional data that belongs to this account. Used to store the balance(s) in a lot of
// 				/// chains.
// 				pub data: AccountData,
// 			}
//
// 			use parity_scale_codec::Decode;
// 			let key = frame_system::Account::<Runtime>::hashed_key_for(whale.clone());
// 			let raw = ext.storage(&key).expect("account should be present");
// 			println!("raw: {:?}", raw);
// 			let acc = AccountInfo::<u32, u8, pallet_balances::AccountData<u128>>::decode(&mut &raw[..]);
// 			println!("acc: {:?}", acc);
// 			let new_key = new_frame_system::Account::<new_polkadot_runtime::Runtime>::hashed_key_for(new_whale.clone());
// 			let new_raw = ext.storage(&new_key).expect("account should be present");
// 			println!("raw new: {:?}", new_raw);
// 			let new_acc = new_frame_system::AccountInfo::<u32, new_pallet_balances::AccountData<u128>>::decode(&mut &new_raw[..]);
// 			println!("acc new: {:?}", new_acc);
// 		}).expect("externalities should be present");
//
// 		frame_system::Account::<Runtime>::get(whale.clone())
// 	});
//
// 	println!("new whale account: {:?}", whale_account);
//
// 	test.revert_blocks(2).expect("final reverting failed");
//
// 	assert!(false);
// }

#[test]
fn runtime_upgrade() {
	use frame_support::{StorageValue, StorageMap};
	use polkadot_runtime::{CouncilCollective, TechnicalCollective};

	// given
	let mut test = test::deterministic::<Node>();

	// test.revert_blocks(2).expect("final reverting failed");

	type Balances = pallet_balances::Module<Runtime>;
	type Democracy = pallet_democracy::Module<Runtime>;
	type Collective = pallet_collective::Module<Runtime>;
	type SystemCall = frame_system::Call<Runtime>;
	type SystemEvents = frame_system::Events<Runtime>;
	type DemocracyCall = pallet_democracy::Call<Runtime>;
	type TechnicalCollectiveCall = pallet_collective::Call<Runtime, TechnicalCollective>;
	type CouncilCollectiveCall = pallet_collective::Call<Runtime, CouncilCollective>;

	let whale = AccountId32::from_str("1rvXMZpAj9nKLQkPFCymyH7Fg3ZyKJhJbrc7UtHbTVhJm1A").unwrap();
	let whale_balance = test.with_state(|| {
		frame_system::Account::<Runtime>::get(&whale)
	});

	// pre-upgrade assertions
	test.with_state(|| {
		use frame_support::StorageMap;
		sp_externalities::with_externalities(|ext| {
			let key = frame_system::Account::<Runtime>::hashed_key_for(whale.clone());
			let raw = ext.storage(&key).expect("account should be present");
			let acc = AccountInfo::<u32, u8, pallet_balances::AccountData<u128>>::decode(&mut &raw[..]);
			assert!(acc.is_ok());
		})
	});

	let wasm = polkadot_runtime::WASM_BINARY.expect("WASM runtime needs to be available.").to_vec();

	let (technical_collective, council_collective) = test.with_state(|| {
		(
			pallet_collective::Members::<Runtime, TechnicalCollective>::get(),
			pallet_collective::Members::<Runtime, CouncilCollective>::get()
		)
	});

	let call = SystemCall::set_code(wasm.clone()).encode();
	// note pre-image
	test.send_extrinsic(DemocracyCall::note_preimage(call.clone()), whale.clone()).unwrap();
	log::info!("sent note_preimage extrinsic");

	test.produce_blocks(1);

	// submit external propose through council
	let proposal_hash = sp_core::hashing::blake2_256(&call[..]);
	log::info!("hashed proposal");
	let external_propose = DemocracyCall::external_propose(proposal_hash.clone().into());
	let proposal_length = external_propose.using_encoded(|x| x.len()) as u32 + 1;
	let council_proposal_weight = external_propose.get_dispatch_info().weight;
	let proposal = CouncilCollectiveCall::propose(council_collective.len() as u32, Box::new(external_propose.into()), proposal_length);
	test.send_extrinsic(proposal.clone(), council_collective[0].clone()).unwrap();
	test.produce_blocks(1);

	let events = test.with_state(|| {
		sp_externalities::with_externalities(|ext| {
			let raw = ext.storage(&SystemEvents::hashed_key()).expect("account should be present");
			log::info!("\n\n{}\n\n", hex::encode(&raw));
			Vec::<EventRecord<<Runtime as frame_system::Trait>::Event, H256>>::decode(&mut &raw[..])
		}).unwrap()
	});

	log::info!("events: {:?}", events);

	// TODO: fetch proposal index from logs
	// vote
	// let council_proposal_hash = sp_core::blake2_256(&proposal.encode());
	// for member in &council_collective {
	// 	test.send_extrinsic(
	// 		CouncilCollectiveCall::vote(council_proposal_hash.clone().into(), proposal_index, true),
	// 		member.clone()
	// 	).unwrap();
	// }
	// test.produce_blocks(1);
	//
	// // close vote
	// test.send_extrinsic(
	// 	CouncilCollectiveCall::close(council_proposal_hash.into(), proposal_index, council_proposal_weight, proposal_length),
	// 	council_collective[0].clone()
	// );
	// test.produce_blocks(1);
	//
	// // next technical collective must fast track.
	// let fast_track = DemocracyCall::fast_track(proposal_hash.into(), FastTrackVotingPeriod::get(), 0);
	// let fast_track_length = fast_track.encode().len() as u32;
	// let technical_proposal_weight = external_propose.get_dispatch_info().weight;
	// let proposal = TechnicalCollectiveCall::propose(technical_collective.len() as u32, Box::new(fast_track.into()), fast_track_length);
	// let technical_proposal_hash = sp_core::blake2_256(&proposal.encode());
	// test.send_extrinsic(
	// 	proposal,
	// 	technical_collective[0].clone(),
	// );
	// test.produce_blocks(1);
	//
	// // TODO: fetch proposal index from logs
	// // vote
	// for member in &technical_collective {
	// 	test.send_extrinsic(
	// 		TechnicalCollectiveCall::vote(technical_proposal_hash.clone().into(), proposal_index, true),
	// 		member.clone()
	// 	).unwrap();
	// }
	// test.produce_blocks(1);
	//
	// // close vote
	// test.send_extrinsic(
	// 	TechnicalCollectiveCall::close(technical_proposal_hash.into(), proposal_index, technical_proposal_weight, fast_track_length),
	// 	technical_collective[0].clone()
	// );
	// test.produce_blocks(1);
	//
	// // wait for fast track period.
	// test.produce_blocks(FastTrackVotingPeriod::get() as usize);
	//
	// // TODO: assert runtime upgraded event in logs
	//
	// test.produce_blocks(1);
	//
	// test.with_state(|| {
	// 	use new_frame_support::StorageMap;
	//
	// 	let new_whale = new_sp_core::crypto::AccountId32::from_str("12dfEn1GycUmHtfEDW3BuQYzsMyUR1PqUPH2pyEikYeUX59o").unwrap();
	//
	// 	sp_externalities::with_externalities(|ext| {
	// 		let new_key = new_frame_system::Account::<new_polkadot_runtime::Runtime>::hashed_key_for(new_whale.clone());
	// 		let new_raw = ext.storage(&new_key).expect("account should be present");
	// 		let new_acc = new_frame_system::AccountInfo::<u32, new_pallet_balances::AccountData<u128>>::decode(&mut &new_raw[..]);
	// 		println!("acc new: {:?}", new_acc);
	// 		assert!(new_acc.is_ok())
	//
	// 	}).expect("externalities should be present");
	// });
	//
	// test.revert_blocks(8 + FastTrackVotingPeriod::get() as _).expect("final reverting failed");
	test.revert_blocks(2).expect("final reverting failed");
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
