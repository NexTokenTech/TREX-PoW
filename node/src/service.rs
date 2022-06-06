//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use blake3::Hash;
use capsule_pow::genesis::genesis_seal;
use capsule_pow::{genesis, CapsuleAlgorithm, Compute, Seal};
use capsule_runtime::{self, opaque::Block, BlockNumber, RuntimeApi};
use cp_constants::{
	Difficulty, KEYCHAIN_HASH_FILE_PATH, KEYCHAIN_HASH_KEY, KEYCHAIN_MAP_FILE_PATH, MAX_DIFFICULTY,
	MINNING_WORKER_BUILD_TIME, MINNING_WORKER_TIMEOUT, MIN_DIFFICULTY,
};
use elgamal_capsule::{KeyGenerator, RawPublicKey};
use futures::executor::block_on;
use futures::join;
use rug::rand::RandState;
use sc_client_api::{Backend, ExecutorProvider};
pub use sc_executor::NativeElseWasmExecutor;
use sc_service::{
	error::Error as ServiceError, Configuration, PartialComponents, TaskManager, DEFAULT_GROUP_NAME,
};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sp_blockchain::HeaderBackend;
use sp_core::{
	crypto::{Ss58Codec, UncheckedFrom},
	Pair, H256,
	Decode, Encode, U256
};
use sp_runtime::{
	generic::{BlockId}
};
use std::{collections::HashMap, io::Read, sync::Arc, thread, time::Duration};

use async_std::{
	fs::{File, OpenOptions},
	prelude::*,
};

use sp_keystore::{SyncCryptoStore, SyncCryptoStorePtr};
use std::path::PathBuf;
use log::warn;
use std::str::FromStr;

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

pub fn decode_author(
	author: Option<&str>,
	keystore: SyncCryptoStorePtr,
	keystore_path: Option<PathBuf>,
) -> Result<capsule_pow::app::Public, String> {
	if let Some(author) = author {
		if author.starts_with("0x") {
			Ok(capsule_pow::app::Public::unchecked_from(
				H256::from_str(&author[2..]).map_err(|_| "Invalid author account".to_string())?,
			)
				.into())
		} else {
			let (address, _) = capsule_pow::app::Public::from_ss58check_with_version(author)
				.map_err(|_| "Invalid author address".to_string())?;
			Ok(address)
		}
	} else {
		dbg!("The node is configured for mining, but no author key is provided.");

		let (pair, phrase, _) = capsule_pow::app::Pair::generate_with_phrase(None);

		SyncCryptoStore::insert_unknown(
			&*keystore.as_ref(),
			capsule_pow::app::ID,
			&phrase,
			pair.public().as_ref(),
		)
			.map_err(|e| format!("Registering mining key failed: {:?}", e))?;

		match keystore_path {
			Some(path) => {
				dbg!("You can go to {:?} to find the seed phrase of the mining key.", path);
			},
			None => {
				warn!("Keystore is not local. This means that your mining key will be lost when exiting the program. This should only happen if you are in dev mode.");
			}
		}

		Ok(pair.public())
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

/// return pubkey for current difficulty at best number.
pub fn get_updated_pubkey(
	difficulty: &Difficulty,
	keychain_map: &HashMap<Difficulty, HashMap<u32, String>>,
) -> RawPublicKey {
	let last_pubkey = match keychain_map.get(difficulty) {
		Some(last_number_and_difficulty) => {
			// get last number
			let mut last_number = 0u32;
			for item in last_number_and_difficulty.keys() {
				last_number = item.to_owned();
				break;
			}
			// get last number's pubkey for hex format
			let last_pubkey_hex = last_number_and_difficulty.get(&last_number).unwrap();
			// get last number's pubkey for bytes format
			let last_pubkey_bytes = hex::decode(last_pubkey_hex).unwrap();
			// decode to RawPublicKey struct
			let last_pubkey = RawPublicKey::decode(&mut &last_pubkey_bytes[..]).unwrap();
			last_pubkey
		},
		None => {
			// Genesis generates pubkey
			let seal = genesis_seal(*difficulty);
			seal.pubkey
		},
	};
	last_pubkey
}

/// update keychain's pubkey and overwrite to dest json file.
pub fn update_keychains(
	keychain_map: &mut HashMap<Difficulty, HashMap<u32, String>>,
	best_number: BlockNumber,
) {
	dbg!("start update pubkey");
	let mut keychain_map_clone = keychain_map.clone();
	let option_file = block_on(run_tasks(keychain_map, best_number));
	if let Some(file) = option_file {
		block_on(write_keychain_to_file(&mut keychain_map_clone, file));
	}
	dbg!("finished");
}
async fn run_tasks(
	keychain_map: &mut HashMap<Difficulty, HashMap<u32, String>>,
	best_number: BlockNumber,
) -> Option<File> {
	// Join the two futures together
	let result = join!(task_open_file(), task_pubkey(keychain_map, best_number));
	result.0
}
pub async fn task_pubkey(
	keychain_map: &mut HashMap<Difficulty, HashMap<u32, String>>,
	best_number: BlockNumber,
) {
	for difficulty_tmp in MIN_DIFFICULTY..(MAX_DIFFICULTY - 1) {
		update_pubkey(keychain_map, &difficulty_tmp, &best_number);
	}
	dbg!("task1");
}

pub async fn task_open_file() -> Option<File> {
	let f = OpenOptions::new()
		.write(true)
		.read(true)
		.create(true)
		.open(KEYCHAIN_MAP_FILE_PATH)
		.await;
	dbg!("task2 finished");
	if f.is_ok() {
		Some(f.unwrap())
	} else {
		None
	}
}

pub async fn write_keychain_to_file(
	keychain_map: &mut HashMap<Difficulty, HashMap<u32, String>>,
	mut keychain_file: File,
) {
	let json_str = serde_json::to_string(keychain_map).unwrap_or("".to_string());
	let json_str_bytes = json_str.as_bytes();
	let _write_result = keychain_file.write_all(json_str_bytes).await;

	let contents = String::from_utf8(json_str_bytes.to_vec()).unwrap_or("".to_string());
	let hash = string_to_blake3(&contents);

	let f_hash = OpenOptions::new()
		.write(true)
		.create(true)
		.read(true)
		.truncate(true)
		.open(KEYCHAIN_HASH_FILE_PATH)
		.await;

	if f_hash.is_ok() {
		let mut file_hash = f_hash.unwrap();
		let hash_value = serde_json::to_string(&hash.as_bytes()).unwrap_or("".to_string());
		let _result = file_hash.write_all(hash_value.as_bytes()).await;
		// write file using serde
		dbg!("Successfully updated hash for keychain file");
	}
	dbg!("Successfully updated Keychain at best number");
}

/// update pubkey for dest difficulty at best_number
pub fn update_pubkey(
	keychain_map: &mut HashMap<Difficulty, HashMap<u32, String>>,
	difficulty: &Difficulty,
	best_number: &u32,
) {
	// init rand instance
	let mut rand = RandState::new_mersenne_twister();

	// get difficult and last_number for specified difficulty
	let last_pubkey = match keychain_map.get(difficulty) {
		Some(last_number_and_difficulty) => {
			// get last number
			let mut last_number = 0u32;
			for item in last_number_and_difficulty.keys() {
				last_number = item.to_owned();
				break;
			}
			// get last number's pubkey for hex format
			let last_pubkey_hex = last_number_and_difficulty.get(&last_number).unwrap();
			// get last number's pubkey for bytes format
			let last_pubkey_bytes = hex::decode(last_pubkey_hex).unwrap();
			// decode to RawPublicKey struct
			let last_pubkey = RawPublicKey::decode(&mut &last_pubkey_bytes[..]).unwrap();
			// define pubkey for iteration
			let mut iter_pubkey = last_pubkey;
			// Iteratively generate the pubkey corresponding to bestnumber for current difficulty
			for _ in last_number..*best_number {
				let next_pubkey = iter_pubkey.yield_pubkey(&mut rand, *difficulty as u32);
				iter_pubkey = next_pubkey;
			}
			// return pubkey
			iter_pubkey
		},
		None => {
			// Genesis generates pubkey
			let seal = genesis_seal(*difficulty);
			// define pubkey for iteration
			let mut iter_pubkey = seal.pubkey;
			// Iteratively generate the pubkey corresponding to bestnumber for current difficulty
			for _ in 0..*best_number {
				let next_pubkey = iter_pubkey.yield_pubkey(&mut rand, *difficulty as u32);
				iter_pubkey = next_pubkey;
			}
			// return pubkey
			iter_pubkey
		},
	};

	let last_pubkey_hex = hex::encode(&last_pubkey.encode());
	// update(mutate or insert) keychain_map
	if keychain_map.contains_key(&difficulty) {
		if let Some(keymap) = keychain_map.get_mut(&difficulty) {
			let mut keymap_tmp = HashMap::<u32, String>::new();
			keymap_tmp.insert(best_number.to_owned(), last_pubkey_hex);
			*keymap = keymap_tmp;
		}
	} else {
		let mut keymap = HashMap::<u32, String>::new();
		keymap.insert(best_number.to_owned(), last_pubkey_hex);
		keychain_map.insert(difficulty.to_owned(), keymap);
	}
}

pub fn keychain_map_from_json() -> HashMap<Difficulty, HashMap<u32, String>> {
	// open file which stored old file hash
	let f_hash = std::fs::OpenOptions::new()
		.write(true)
		.create(true)
		.read(true)
		.open(KEYCHAIN_HASH_FILE_PATH);

	// default keychain map
	let default_keychain_map: HashMap<Difficulty, HashMap<u32, String>> =
		HashMap::<Difficulty, HashMap<u32, String>>::new();

	// old hash bytes
	let mut old_file_hash_bytes = vec![];
	if f_hash.is_ok() {
		let file_hash = f_hash.as_ref().unwrap();
		old_file_hash_bytes = match serde_json::from_reader(file_hash) {
			Ok(file_hash_bytes) => file_hash_bytes,
			Err(_) => vec![],
		};
	}
	// create a file instance for write and read.
	let f_keychain = std::fs::OpenOptions::new()
		.write(true)
		.create(true)
		.read(true)
		.open(KEYCHAIN_MAP_FILE_PATH);
	// get keychain map from json file.
	if f_keychain.is_ok() {
		let mut keychain_file = f_keychain.unwrap();
		// use to record cur hash == old hash or not.
		let mut is_validate = true;

		// get cur keychain map file hash
		let mut contents = String::new();
		keychain_file.read_to_string(&mut contents).unwrap();
		let cur_file_hash = string_to_blake3(&contents);

		// get is_validate if old hash file is not empty.
		if old_file_hash_bytes.len() != 0 {
			let old_file_hash_u8_bytes: &[u8; 32] = &(&old_file_hash_bytes[..]).try_into().unwrap();
			let old_file_hash = Hash::from(*old_file_hash_u8_bytes);
			is_validate = old_file_hash == cur_file_hash;
		}

		if is_validate == true {
			let keychain_map_read =
				serde_json::from_str(&contents).unwrap_or(default_keychain_map.clone());
			return keychain_map_read;
		}
	}
	default_keychain_map
}

pub fn string_to_blake3(keychain_file_str: &str) -> Hash {
	let hashmap_bytes = keychain_file_str.as_bytes();
	let hash = blake3::keyed_hash(&KEYCHAIN_HASH_KEY, hashmap_bytes);
	hash
}

/// Builds a new service for a full client.
pub fn new_full(
	config: Configuration,
	mining: bool,
	author: Option<&str>
) -> Result<TaskManager, ServiceError> {
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

	let keystore_path = config.keystore.path().map(|p| p.to_owned());

	if is_authority {
		let author = decode_author(author, keystore_container.sync_keystore(), keystore_path)?;
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
				Some(author.encode()),
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
			let mut keychain_map = keychain_map_from_json().clone();
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
						update_keychains(&mut keychain_map, block_number);
						dbg!("I get keychain here");
						let updated_pubkey =
							get_updated_pubkey(&metadata.difficulty, &keychain_map);

						let mut compute = Compute {
							difficulty: metadata.difficulty,
							pre_hash: metadata.pre_hash,
							nonce: U256::from(0i32),
						};

						if let Some(new_seal) =
							seal.try_cpu_mining(&mut compute, seed, updated_pubkey)
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
