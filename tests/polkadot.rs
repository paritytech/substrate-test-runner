use manual_seal::consensus::{babe::BabeConsensusDataProvider, ConsensusDataProvider};
use parity_scale_codec::alloc::sync::Arc;
use polkadot_runtime::{Runtime, SignedExtra, FastTrackVotingPeriod, Event};
use polkadot_service::{chain_spec::polkadot_development_config_genesis, PolkadotChainSpec};
use sc_consensus_babe::BabeBlockImport;
use sc_finality_grandpa::GrandpaBlockImport;
use sc_service::{new_full_parts, ChainType, Configuration, TFullBackend, TFullClient, TaskManager};
use sp_api::TransactionFor;
use sp_consensus_babe::AuthorityId;
use sp_core::crypto::AccountId32;
use sp_inherents::InherentDataProviders;
use sp_keyring::sr25519::Keyring::Alice;
use sp_keystore::SyncCryptoStorePtr;
use sp_runtime::generic::Era;
use std::str::FromStr;
use substrate_test_runner::{node::{StateProvider, TestRuntimeRequirements}, test};
use parity_scale_codec::Encode;
use frame_support::dispatch::GetDispatchInfo;

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
		Some("/home/seunlanlege/polkadot")
	}

	fn signed_extras<S>(state: &S, from: <Self::Runtime as frame_system::Trait>::AccountId) -> Self::SignedExtension
	where
		S: StateProvider,
	{
		let nonce = state.with_state(|| frame_system::Module::<Runtime>::account_nonce(from));

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
fn runtime_upgrade() {
	use frame_support::StorageValue;
	use polkadot_runtime::{CouncilCollective, TechnicalCollective};

	let mut test = test::deterministic::<Node>();
	test.revert_blocks(8 + FastTrackVotingPeriod::get()).expect("final reverting failed");


	type SystemCall = frame_system::Call<Runtime>;
	type DemocracyCall = pallet_democracy::Call<Runtime>;
	type TechnicalCollectiveCall = pallet_collective::Call<Runtime, TechnicalCollective>;
	type CouncilCollectiveCall = pallet_collective::Call<Runtime, CouncilCollective>;

	// here lies a black mirror esque copy of an on chain whale.
	let whale = AccountId32::from_str("1rvXMZpAj9nKLQkPFCymyH7Fg3ZyKJhJbrc7UtHbTVhJm1A").unwrap();

	// we'll be needing this
	let wasm = polkadot_runtime::WASM_BINARY.expect("WASM runtime needs to be available.").to_vec();

	// and these
	let (technical_collective, council_collective) = test.with_state(|| {
		(
			pallet_collective::Members::<Runtime, TechnicalCollective>::get(),
			pallet_collective::Members::<Runtime, CouncilCollective>::get()
		)
	});

	let call = SystemCall::set_code(wasm.clone()).encode();
	// note the call (pre-image?) of the proposal
	test.send_extrinsic(DemocracyCall::note_preimage(call.clone()), whale.clone()).unwrap();
	log::info!("sent note_preimage extrinsic");
	test.produce_blocks(1);

	// fetch proposal hash from event emitted by the runtime
	let events = test.with_state(|| frame_system::Events::<Runtime>::get());
	let proposal_hash = events.into_iter()
		.filter_map(|event| match event.event {
			Event::pallet_democracy(pallet_democracy::RawEvent::PreimageNoted(proposal_hash, _, _)) => Some(proposal_hash),
			_ => None
		})
		.next()
		.unwrap();

	// submit proposal call through council
	let external_propose = DemocracyCall::external_propose_majority(proposal_hash.clone().into());
	let proposal_length = external_propose.using_encoded(|x| x.len()) as u32 + 1;
	let proposal_weight = external_propose.get_dispatch_info().weight;
	let proposal = CouncilCollectiveCall::propose(council_collective.len() as u32, Box::new(external_propose.clone().into()), proposal_length);
	test.send_extrinsic(proposal.clone(), council_collective[0].clone()).unwrap();
	test.produce_blocks(1);

	// fetch proposal index from event emitted by the runtime
	let events = test.with_state(|| frame_system::Events::<Runtime>::get());
	let (council_proposal_index, council_proposal_hash) = events.into_iter()
		.filter_map(|event| {
			match event.event {
				Event::pallet_collective_Instance1(pallet_collective::RawEvent::Proposed(_, index, proposal_hash, _)) => Some((index, proposal_hash)),
				_ => None
			}
		})
		.next()
		.unwrap();

	// vote
	for member in &council_collective[1..] {
		test.send_extrinsic(
			CouncilCollectiveCall::vote(council_proposal_hash.clone(), council_proposal_index, true),
			member.clone()
		).unwrap();
	}
	test.produce_blocks(1);

	// close vote
	test.send_extrinsic(
		CouncilCollectiveCall::close(council_proposal_hash, council_proposal_index, proposal_weight, proposal_length),
		council_collective[0].clone()
	).unwrap();
	test.produce_blocks(1);

	// assert that proposal has been passed on chain
	let events = test.with_state(|| frame_system::Events::<Runtime>::get())
		.into_iter()
		.filter(|event| {
			match event.event {
				Event::pallet_collective_Instance1(pallet_collective::RawEvent::Closed(_, _, _)) |
				Event::pallet_collective_Instance1(pallet_collective::RawEvent::Approved(_,)) |
				Event::pallet_collective_Instance1(pallet_collective::RawEvent::Executed(_, Ok(()))) => true,
				_ => false,
			}
		})
		.collect::<Vec<_>>();

	// make sure all 3 events are in state
	assert_eq!(events.len(), 3);

	// next technical collective must fast track the proposal.
	let fast_track = DemocracyCall::fast_track(proposal_hash.into(), FastTrackVotingPeriod::get(), 0);
	let proposal_weight = fast_track.get_dispatch_info().weight;
	let fast_track_length = fast_track.using_encoded(|x| x.len()) as u32 + 1;
	let proposal = TechnicalCollectiveCall::propose(technical_collective.len() as u32, Box::new(fast_track.into()), fast_track_length);
	test.send_extrinsic(
		proposal,
		technical_collective[0].clone(),
	).unwrap();
	test.produce_blocks(1);

	let events = test.with_state(|| frame_system::Events::<Runtime>::get());
	let (technical_proposal_index, technical_proposal_hash) = events.into_iter()
		.filter_map(|event| {
			match event.event {
				Event::pallet_collective_Instance2(pallet_collective::RawEvent::Proposed(_, index, hash, _)) => Some((index, hash)),
				_ => None
			}
		})
		.next()
		.unwrap();

	// TODO: fetch proposal index from logs
	// vote
	for member in &technical_collective[1..] {
		test.send_extrinsic(
			TechnicalCollectiveCall::vote(technical_proposal_hash.clone(), technical_proposal_index, true),
			member.clone()
		).unwrap();
	}
	test.produce_blocks(1);

	// close vote
	test.send_extrinsic(
		TechnicalCollectiveCall::close(technical_proposal_hash, technical_proposal_index, proposal_weight, fast_track_length),
		technical_collective[0].clone()
	).unwrap();
	test.produce_blocks(1);

	// assert that proposal has been passed on chain
	let events = test
		.with_state(|| frame_system::Events::<Runtime>::get())
		.into_iter()
		.filter(|event| {
			match event.event {
				Event::pallet_collective_Instance2(pallet_collective::RawEvent::Closed(_, _, _)) |
				Event::pallet_collective_Instance2(pallet_collective::RawEvent::Approved(_)) |
				Event::pallet_collective_Instance2(pallet_collective::RawEvent::Executed(_, Ok(()))) => true,
				_ => false,
			}
		})
		.collect::<Vec<_>>();

	// make sure all 3 events are in state
	assert_eq!(events.len(), 3);

	// wait for fast track period.
	test.produce_blocks(FastTrackVotingPeriod::get() as usize);

	test.with_state(|| {
		log::info!("\n\n{:#?}\n\n", frame_system::Events::<Runtime>::get())
	});

	let event = test.with_state(|| frame_system::Events::<Runtime>::get())
		.into_iter()
		.find(|event| {
			match event.event {
				Event::frame_system(frame_system::RawEvent::CodeUpdated) => true,
				_ => false,
			}
		});

	// make sure events are in state
	assert!(event.is_some());

	// TODO: assert runtime upgraded event in logs

	test.produce_blocks(1);
	test.revert_blocks(8 + FastTrackVotingPeriod::get()).expect("final reverting failed");
}

