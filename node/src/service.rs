//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use capsule_pow::genesis::genesis_seal;
use capsule_pow::{genesis, CapsuleAlgorithm, Compute, Seal};
use capsule_runtime::{self, opaque::Block, BlockNumber, RuntimeApi};
use cp_constants::{
	Difficulty, KEYCHAIN_MAP_FILE_PATH, MAX_DIFFICULTY, MINNING_WORKER_BUILD_TIME,
	MINNING_WORKER_TIMEOUT, MIN_DIFFICULTY,
};
use elgamal_capsule::{KeyGenerator, RawPublicKey};
use futures::executor::block_on;
use rug::rand::RandState;
use sc_client_api::{Backend, ExecutorProvider};
pub use sc_executor::NativeElseWasmExecutor;
use sc_service::{
	error::Error as ServiceError, Configuration, PartialComponents, TaskManager, DEFAULT_GROUP_NAME,
};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sp_blockchain::HeaderBackend;
use sp_core::{Decode, Encode, U256};
use sp_runtime::generic::BlockId;
use std::collections::HashMap;
use std::{sync::Arc, thread, time::Duration};
use futures::stream::iter;

// Our native executor instance.
pub struct ExecutorDispatch;

impl sc_executor::NativeExecutionDispatch for ExecutorDispatch {
	/// Only enable the benchmarking host functions when we actually want to benchmark.
	#[cfg(feature = "runtime-benchmarks")]
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;
	/// Otherwise we only use the default Substrate host functions.
	#[cfg(not(feature = "runtime-benchmarks"))]
	type ExtendHostFunctions = ();

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		capsule_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		capsule_runtime::native_version()
	}
}

type FullClient =
	sc_service::TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<ExecutorDispatch>>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;

/// Returns most parts of a service. Not enough to run a full chain,
/// But enough to perform chain operations like purge-chain
#[allow(clippy::type_complexity)]
pub fn new_partial(
	config: &Configuration,
) -> Result<
	PartialComponents<
		FullClient,
		FullBackend,
		FullSelectChain,
		sc_consensus::DefaultImportQueue<Block, FullClient>,
		sc_transaction_pool::FullPool<Block, FullClient>,
		(
			sc_consensus_pow::PowBlockImport<
				Block,
				Arc<FullClient>,
				FullClient,
				FullSelectChain,
				CapsuleAlgorithm<FullClient>,
				impl sp_consensus::CanAuthorWith<Block>,
				impl sp_inherents::CreateInherentDataProviders<Block, ()>,
			>,
			Option<Telemetry>,
		),
	>,
	ServiceError,
> {
	let executor = NativeElseWasmExecutor::<ExecutorDispatch>::new(
		config.wasm_method,
		config.default_heap_pages,
		config.max_runtime_instances,
		config.runtime_cache_size,
	);

	let telemetry = config
		.telemetry_endpoints
		.clone()
		.filter(|x| !x.is_empty())
		.map(|endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts::<Block, RuntimeApi, _>(
			&config,
			telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
			executor,
		)?;
	// map telemetry to task manager.
	let telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager.spawn_handle().spawn("telemetry", None, worker.run());
		telemetry
	});

	let select_chain = sc_consensus::LongestChain::new(Arc::clone(&backend));
	let client = Arc::new(client);

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		config.prometheus_registry(),
		task_manager.spawn_essential_handle(),
		Arc::clone(&client),
	);

	let can_author_with = sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());

	let algorithm = capsule_pow::CapsuleAlgorithm::new(client.clone());

	let pow_block_import = sc_consensus_pow::PowBlockImport::new(
		Arc::clone(&client),
		Arc::clone(&client),
		algorithm.clone(),
		0, // check inherent starting at block 0
		select_chain.clone(),
		|_parent, ()| async {
			let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
			// let capsule_data = cp_inherent::InherentDataProvider::from_default_value();
			Ok(timestamp)
		},
		can_author_with,
	);

	let import_queue = sc_consensus_pow::import_queue(
		Box::new(pow_block_import.clone()),
		None,
		algorithm.clone(),
		&task_manager.spawn_essential_handle(),
		config.prometheus_registry(),
	)?;

	Ok(PartialComponents {
		client,
		backend,
		import_queue,
		keystore_container,
		task_manager,
		transaction_pool,
		select_chain,
		other: (pow_block_import, telemetry),
	})
}
pub fn update_keychains_with_difficulty(
	difficulty: &u128,
	best_number: BlockNumber,
) -> RawPublicKey {
	// cast current difficulty to owned.
	let current_difficulty = difficulty.to_owned();
	// create a file instance for write and read.
	let f = std::fs::OpenOptions::new()
		.write(true)
		.create(true)
		.read(true)
		.open(KEYCHAIN_MAP_FILE_PATH);
	// get keychain map from json file.
	let default_keychain_map: HashMap<Difficulty, HashMap<u32, Vec<u8>>> =
		HashMap::<Difficulty, HashMap<u32, Vec<u8>>>::new();
	let mut keychain_map: HashMap<Difficulty, HashMap<u32, Vec<u8>>> = match f {
		Ok(keychain_file) => match serde_json::from_reader(keychain_file) {
			Ok(keychain_map) => keychain_map,
			Err(_) => default_keychain_map,
		},
		Err(_) => default_keychain_map,
	};

	// Generate pubkey of current difficulty
	let keychain_map_tuple = handle_pubkey(&mut keychain_map, &difficulty, &best_number);
	let last_pubkey = keychain_map_tuple.0;
	let mut keychain_map_final = keychain_map_tuple.1;

	for difficulty_tmp in MIN_DIFFICULTY..(MAX_DIFFICULTY - 1) {
		if difficulty_tmp == difficulty.to_owned() {
			continue;
		}
		let keychain_map_tuple = handle_pubkey(&mut keychain_map, &difficulty_tmp, &best_number);
		keychain_map_final = keychain_map_tuple.1;
	}

	// remove old file,make sure the file is clear for writing.
	std::fs::remove_file(KEYCHAIN_MAP_FILE_PATH);
	// new a file instance for overwrite json file.
	let f = std::fs::OpenOptions::new()
		.write(true)
		.create(true)
		.open(KEYCHAIN_MAP_FILE_PATH);
	match f {
		Ok(keychain_file) => {
			// write file using serde
			serde_json::to_writer_pretty(keychain_file, &keychain_map_final);
		},
		Err(_) => {},
	}

	last_pubkey
}

pub fn handle_pubkey(
	keychain_map: &mut HashMap<Difficulty, HashMap<u32, Vec<u8>>>,
	difficulty: &Difficulty,
	best_number: &u32,
) -> (RawPublicKey, HashMap<Difficulty, HashMap<u32, Vec<u8>>>) {
	// init rand instance
	let mut rand = RandState::new_mersenne_twister();
	// difficulty to owned
	let difficulty = difficulty.to_owned();

	// get difficult and last_number for specified difficulty
	let last_pubkey = match keychain_map.get(&difficulty) {
		Some(last_number_and_difficulty) => {
			// get last number
			let mut last_number = 0u32;
			for item in last_number_and_difficulty.keys() {
				last_number = item.to_owned();
				break;
			}
			// get last number's pubkey for bytes format
			let last_pubkey_bytes = last_number_and_difficulty.get(&last_number).unwrap();
			// decode to RawPublicKey struct
			let last_pubkey = RawPublicKey::decode(&mut &last_pubkey_bytes[..]).unwrap();
			// define pubkey for iteration
			let mut iter_pubkey = last_pubkey;
			// Iteratively generate the pubkey corresponding to bestnumber for current difficulty
			for _ in last_number..best_number.to_owned() {
				let next_pubkey = iter_pubkey.yield_pubkey(&mut rand, difficulty as u32);
				iter_pubkey = next_pubkey;
			}
			// return pubkey
			iter_pubkey
		},
		None => {
			// Genesis generates pubkey
			let seal = genesis_seal(difficulty.to_owned());
			// define pubkey for iteration
			let mut iter_pubkey = seal.pubkey;
			// Iteratively generate the pubkey corresponding to bestnumber for current difficulty
			for _ in 0..best_number.to_owned() {
				let next_pubkey = iter_pubkey.yield_pubkey(&mut rand, difficulty as u32);
				iter_pubkey = next_pubkey;
			}
			// return pubkey
			iter_pubkey
		},
	};

	// update(mutate or insert) keychain_map
	if keychain_map.contains_key(&difficulty) {
		if let Some(keymap) = keychain_map.get_mut(&difficulty) {
			let mut last_number = 0u32;
			for item in keymap.keys() {
				last_number = item.to_owned();
				break;
			}
			let mut keymap_tmp = HashMap::<u32, Vec<u8>>::new();
			keymap_tmp.insert(best_number.to_owned(), last_pubkey.encode());
			*keymap = keymap_tmp;
		}
	} else {
		let mut keymap = HashMap::<u32, Vec<u8>>::new();
		keymap.insert(best_number.to_owned(), last_pubkey.encode());
		keychain_map.insert(difficulty.to_owned(), keymap);
	}
	(last_pubkey, keychain_map.to_owned())
}

/// Builds a new service for a full client.
pub fn new_full(config: Configuration, mining: bool) -> Result<TaskManager, ServiceError> {
	let sc_service::PartialComponents {
		client,
		backend,
		mut task_manager,
		import_queue,
		keystore_container,
		select_chain,
		transaction_pool,
		other: (pow_block_import, mut telemetry),
	} = new_partial(&config)?;

	let (network, system_rpc_tx, network_starter) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			client: Arc::clone(&client),
			transaction_pool: Arc::clone(&transaction_pool),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			block_announce_validator_builder: None,
			warp_sync: None,
		})?;

	if config.offchain_worker.enabled {
		sc_service::build_offchain_workers(
			&config,
			task_manager.spawn_handle(),
			Arc::clone(&client),
			Arc::clone(&network),
		);
	}

	let is_authority = config.role.is_authority();
	let prometheus_registry = config.prometheus_registry().cloned();

	if is_authority {
		let proposer = sc_basic_authorship::ProposerFactory::new(
			task_manager.spawn_handle(),
			Arc::clone(&client),
			Arc::clone(&transaction_pool),
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|x| x.handle()),
		);

		let can_author_with =
			sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());

		if mining {
			let algorithm = capsule_pow::CapsuleAlgorithm::new(client.clone());
			// Parameter details:
			//   https://substrate.dev/rustdocs/latest/sc_consensus_pow/fn.start_mining_worker.html
			// Also refer to kulupu config:
			//   https://github.com/kulupu/kulupu/blob/master/src/service.rs
			let (_worker, worker_task) = sc_consensus_pow::start_mining_worker(
				Box::new(pow_block_import),
				Arc::clone(&client),
				select_chain,
				algorithm.clone(),
				proposer,
				Arc::clone(&network),
				Arc::clone(&network),
				// Here, the pre-runtime item is the public key for time release encryption.
				None,
				// For block production we want to provide our inherent data provider
				|_parent, ()| async {
					let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
					// let capsule_data = cp_inherent::InherentDataProvider::from_default_value();
					Ok(timestamp)
				},
				// time to wait for a new block before starting to mine a new one
				Duration::from_secs(MINNING_WORKER_TIMEOUT),
				// how long to take to actually build the block (i.e. executing extrinsics)
				Duration::from_secs(MINNING_WORKER_BUILD_TIME),
				can_author_with,
			);

			task_manager.spawn_essential_handle().spawn_blocking(
				"pow",
				DEFAULT_GROUP_NAME,
				worker_task,
			);

			// Start Mining
			// mining worker with mutex lock and arc pointer
			let worker = Arc::new(_worker);
			let current_backend = backend.clone();
			thread::spawn(move || {
				// get current pubkey from current block header.
				let blockchain = current_backend.blockchain();
				let find_seal = || -> Option<Seal> {
					let chain_info = blockchain.info();
					let best_hash = chain_info.best_hash;
					let best_num = chain_info.best_number;
					// info!("Current best block number: {}", best_num);
					if best_num == 0 {
						// genesis block does not have a a header, need to create a artificial seal.
						return Some(genesis::genesis_seal(MIN_DIFFICULTY));
					}
					if let Some(header) = blockchain.header(BlockId::Hash(best_hash)).unwrap() {
						let mut digest = header.digest;
						while let Some(item) = digest.pop() {
							if let Some(raw_seal) = item.as_seal() {
								let mut coded_seal = raw_seal.1;
								return Some(Seal::decode(&mut coded_seal).unwrap());
							}
						}
					}
					None
				};
				// WARNING: do not use 0 as initial seed.
				let mut seed = U256::from(1i32);
				loop {
					let worker = Arc::clone(&worker);
					let metadata = worker.metadata();
					let seal = find_seal();
					if let (Some(metadata), Some(seal)) = (metadata, seal) {
						// info!("Found seal!");
						//update keychains
						let blockchain = current_backend.blockchain();
						let chain_info = blockchain.info();
						let block_number = chain_info.best_number;
						let last_pubkey_for_cur_difficulty =
							update_keychains_with_difficulty(&metadata.difficulty, block_number);

						let mut compute = Compute {
							difficulty: metadata.difficulty,
							pre_hash: metadata.pre_hash,
							nonce: U256::from(0i32),
						};
						if let Some(new_seal) =
							seal.try_cpu_mining(&mut compute, seed, last_pubkey_for_cur_difficulty)
						{
							// Found a new seal, reset the mining seed.
							seed = U256::from(1i32);
							block_on(worker.submit(new_seal.encode()));
						} else {
							seed = seed.saturating_add(U256::from(1i32));
							if seed == U256::MAX {
								seed = U256::from(0i32);
							}
						}
					} else {
						// info!("Not found seal or metadata!");
						thread::sleep(Duration::new(1, 0));
					}
				}
			});
		}
	}

	// prepare rpc builder
	let full_client = Arc::clone(&client);
	let tx_pool = Arc::clone(&transaction_pool);
	// here the arc pointer has to be cloned again so that every time this closure is called,
	// the arc pointer counter will be incremented but the pointer is not moved into the closure.
	let rpc_extensions_builder = Box::new(move |deny_unsafe, _| {
		let deps = crate::rpc::FullDeps {
			client: full_client.clone(),
			pool: tx_pool.clone(),
			deny_unsafe,
		};

		Ok(crate::rpc::create_full(deps))
	});

	// spawn rpc tasks
	let _rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		network,
		client,
		keystore: keystore_container.sync_keystore(),
		task_manager: &mut task_manager,
		transaction_pool,
		rpc_extensions_builder,
		backend,
		system_rpc_tx,
		config,
		telemetry: telemetry.as_mut(),
	})?;

	network_starter.start_network();
	Ok(task_manager)
}
