//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use trex_pow::{genesis, TrexAlgorithm, Compute, Seal};
use trex_runtime::{self, opaque::Block, RuntimeApi};
use trex_constants::{
	MINING_WORKER_BUILD_TIME, MINING_WORKER_TIMEOUT, INIT_DIFFICULTY,
};
use futures::{executor::block_on};
use sc_client_api::{Backend, ExecutorProvider};
pub use sc_executor::NativeElseWasmExecutor;
use sc_service::{
	error::Error as ServiceError, Configuration, PartialComponents, TaskManager, DEFAULT_GROUP_NAME,
};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sp_blockchain::HeaderBackend;
use sp_core::{
	crypto::{Ss58Codec, UncheckedFrom},
	Decode, Encode, Pair, H256, U256,
};
use sp_runtime::generic::BlockId;
use std::{sync::Arc, thread, time::Duration};

use log::warn;
use sp_keystore::{SyncCryptoStore, SyncCryptoStorePtr};
use std::{path::PathBuf, str::FromStr};

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
		trex_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		trex_runtime::native_version()
	}
}

pub fn decode_author(
	author: Option<&str>,
	keystore: SyncCryptoStorePtr,
	keystore_path: Option<PathBuf>,
) -> Result<trex_pow::app::Public, String> {
	if let Some(author) = author {
		if author.starts_with("0x") {
			Ok(trex_pow::app::Public::unchecked_from(
				H256::from_str(&author[2..]).map_err(|_| "Invalid author account".to_string())?,
			)
			.into())
		} else {
			let (address, _) = trex_pow::app::Public::from_ss58check_with_version(author)
				.map_err(|_| "Invalid author address".to_string())?;
			Ok(address)
		}
	} else {
		dbg!("The node is configured for mining, but no author key is provided.");

		let (pair, phrase, _) = trex_pow::app::Pair::generate_with_phrase(None);

		SyncCryptoStore::insert_unknown(
			&*keystore.as_ref(),
			trex_pow::app::ID,
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
			},
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
				TrexAlgorithm<FullClient>,
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

	let algorithm = trex_pow::TrexAlgorithm::new(client.clone());

	let pow_block_import = sc_consensus_pow::PowBlockImport::new(
		Arc::clone(&client),
		Arc::clone(&client),
		algorithm.clone(),
		0, // check inherent starting at block 0
		select_chain.clone(),
		|_parent, ()| async {
			let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
			// let trex_data = trex_inherent::InherentDataProvider::from_default_value();
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

/// Builds a new service for a full client.
pub fn new_full(
	config: Configuration,
	mining: bool,
	author: Option<&str>,
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
			let algorithm = trex_pow::TrexAlgorithm::new(client.clone());
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
					// let trex_data = trex_inherent::InherentDataProvider::from_default_value();
					Ok(timestamp)
				},
				// time to wait for a new block before starting to mine a new one
				Duration::from_secs(MINING_WORKER_TIMEOUT),
				// how long to take to actually build the block (i.e. executing extrinsics)
				Duration::from_secs(MINING_WORKER_BUILD_TIME),
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
						return Some(genesis::genesis_seal(INIT_DIFFICULTY))
					}
					if let Some(header) = blockchain.header(BlockId::Hash(best_hash)).unwrap() {
						let mut digest = header.digest;
						while let Some(item) = digest.pop() {
							if let Some(raw_seal) = item.as_seal() {
								let mut coded_seal = raw_seal.1;
								return Some(Seal::decode(&mut coded_seal).unwrap())
							}
						}
					}
					None
				};
				// WARNING: do not use 0 as initial seed.
				let mut mining_seed = U256::from(1i32);
				loop {
					let worker = Arc::clone(&worker);
					let metadata = worker.metadata();
					let seal = find_seal();
					if let (Some(metadata), Some(seal)) = (metadata, seal) {
						// dbg!("Found seal!");
						let mut compute = Compute {
							difficulty: metadata.difficulty,
							pre_hash: metadata.pre_hash,
							nonce: U256::from(0i32),
						};

						if let Some(new_seal) = seal.try_cpu_mining(&mut compute, mining_seed) {
							// Found a new seal, reset the mining seed.
							mining_seed = U256::from(1i32);
							block_on(worker.submit(new_seal.encode()));
						} else {
							mining_seed = mining_seed.saturating_add(U256::from(1i32));
							if mining_seed == U256::MAX {
								mining_seed = U256::from(0i32);
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

	let rpc_extensions_builder = {
		let client = client.clone();
		let pool = transaction_pool.clone();

		Box::new(move |deny_unsafe, _| {
			let deps =
				crate::rpc::FullDeps { client: client.clone(), pool: pool.clone(), deny_unsafe };
			crate::rpc::create_full(deps).map_err(Into::into)
		})
	};

	// spawn rpc tasks
	let _rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		network,
		client,
		keystore: keystore_container.sync_keystore(),
		task_manager: &mut task_manager,
		transaction_pool,
		rpc_builder: rpc_extensions_builder,
		backend,
		system_rpc_tx,
		config,
		telemetry: telemetry.as_mut(),
	})?;

	network_starter.start_network();
	Ok(task_manager)
}
