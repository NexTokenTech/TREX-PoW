//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use async_trait;
use capsule_runtime::{self, opaque::Block, RuntimeApi};
use futures::executor::block_on;
use sc_client_api::ExecutorProvider;
pub use sc_executor::NativeElseWasmExecutor;
use sc_service::{
	error::Error as ServiceError, Configuration, PartialComponents, TaskManager, DEFAULT_GROUP_NAME,
};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sha3pow::{hash_meets_difficulty, Compute, MinimalSha3Algorithm};
use sp_core::{Decode, Encode, U256};
use sp_inherents::{InherentData, InherentIdentifier};
use std::{sync::Arc, thread, time::Duration};

// This needs to be unique for the runtime.
const INHERENT_IDENTIFIER: InherentIdentifier = *b"testinh0";

/// Some custom inherent data provider
struct InherentDataProvider;

#[async_trait::async_trait]
impl sp_inherents::InherentDataProvider for InherentDataProvider {
	fn provide_inherent_data(
		&self,
		inherent_data: &mut InherentData,
	) -> Result<(), sp_inherents::Error> {
		// We can insert any data that implements [`codec::Encode`].
		inherent_data.put_data(INHERENT_IDENTIFIER, &"hello")
	}

	/// When validating the inherents, the runtime implementation can throw errors. We support
	/// two error modes, fatal and non-fatal errors. A fatal error means that the block is invalid
	/// and this function here should return `Err(_)` to not import the block. Non-fatal errors
	/// are allowed to be handled here in this function and the function should return `Ok(())`
	/// if it could be handled. A non-fatal error is for example that a block is in the future
	/// from the point of view of the local node. In such a case the block import for example
	/// should be delayed until the block is valid.
	///
	/// If this functions returns `None`, it means that it is not responsible for this error or
	/// that the error could not be interpreted.
	async fn try_handle_error(
		&self,
		identifier: &InherentIdentifier,
		mut error: &[u8],
	) -> Option<Result<(), sp_inherents::Error>> {
		// Check if this error belongs to us.
		if *identifier != INHERENT_IDENTIFIER {
			return None
		}

		// For demonstration purposes we are using a `String` as error type. In real
		// implementations it is advised to not use `String`.
		Some(Err(sp_inherents::Error::Application(Box::from(String::decode(&mut error).ok()?))))
	}
}

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
				MinimalSha3Algorithm,
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

	let pow_block_import = sc_consensus_pow::PowBlockImport::new(
		Arc::clone(&client),
		Arc::clone(&client),
		sha3pow::MinimalSha3Algorithm,
		0, // check inherent starting at block 0
		select_chain.clone(),
		move |_, ()| async move {
			let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
			Ok(timestamp)
		},
		can_author_with,
	);

	let import_queue = sc_consensus_pow::import_queue(
		Box::new(pow_block_import.clone()),
		None,
		sha3pow::MinimalSha3Algorithm,
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
			// Parameter details:
			//   https://substrate.dev/rustdocs/latest/sc_consensus_pow/fn.start_mining_worker.html
			// Also refer to kulupu config:
			//   https://github.com/kulupu/kulupu/blob/master/src/service.rs
			let (_worker, worker_task) = sc_consensus_pow::start_mining_worker(
				Box::new(pow_block_import),
				Arc::clone(&client),
				select_chain,
				MinimalSha3Algorithm,
				proposer,
				Arc::clone(&network),
				Arc::clone(&network),
				None,
				// For block production we want to provide our inherent data provider
				move |_, ()| async move {
					let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
					Ok(timestamp)
				},
				// time to wait for a new block before starting to mine a new one
				Duration::from_secs(30),
				// how long to take to actually build the block (i.e. executing extrinsics)
				Duration::from_secs(30),
				can_author_with,
			);

			task_manager.spawn_essential_handle().spawn_blocking(
				"pow",
				DEFAULT_GROUP_NAME,
				worker_task,
			);

			// Start Mining
			let mut nonce: U256 = U256::from(0i32);
			// mining worker with mutex lock and arc pointer
			let worker = Arc::new(_worker);
			thread::spawn(move || loop {
				let worker = Arc::clone(&worker);
				let metadata = worker.metadata();
				if let Some(metadata) = metadata {
					let compute = Compute {
						difficulty: metadata.difficulty,
						pre_hash: metadata.pre_hash,
						nonce,
					};
					let seal = compute.compute();
					//TODO: print nonce,maybe difficulty is to high?
					if hash_meets_difficulty(&seal.work, seal.difficulty) {
						nonce = U256::from(0i32);
						// blocking on the block import,
						// since the Mutex cannot be sent to another thread.
						block_on(worker.submit(seal.encode()));
					} else {
						nonce = nonce.saturating_add(U256::from(1i32));
						if nonce == U256::MAX {
							nonce = U256::from(0i32);
						}
					}
				} else {
					thread::sleep(Duration::new(1, 0));
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
