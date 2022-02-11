//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use capsule_runtime::{self, opaque::Block, RuntimeApi};
use sc_client_api::ExecutorProvider;
pub use sc_executor::NativeElseWasmExecutor;
use sc_service::{error::Error as ServiceError, Configuration, PartialComponents, TaskManager};
use sc_service::DEFAULT_GROUP_NAME;
use sha3pow::MinimalSha3Algorithm;
use std::{sync::Arc, time::Duration};
use frame_benchmarking::frame_support::codec::Decode;
use sp_inherents::{InherentIdentifier, InherentData};
use async_trait;

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
			return None;
		}

		// For demonstration purposes we are using a `String` as error type. In real
		// implementations it is advised to not use `String`.
		Some(Err(
			sp_inherents::Error::Application(Box::from(String::decode(&mut error).ok()?))
		))
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

type FullClient = sc_service::TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<ExecutorDispatch>>;
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
		sc_consensus_pow::PowBlockImport<
			Block,
			Arc<FullClient>,
			FullClient,
			FullSelectChain,
			MinimalSha3Algorithm,
			impl sp_consensus::CanAuthorWith<Block>,
			impl sp_inherents::CreateInherentDataProviders<Block, ()>,
		>,
	>,
	ServiceError,
> {

	let executor = NativeElseWasmExecutor::<ExecutorDispatch>::new(
		config.wasm_method,
		config.default_heap_pages,
		config.max_runtime_instances,
	);

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts::<Block, RuntimeApi, _>(&config, None, executor)?;
	let select_chain = sc_consensus::LongestChain::new(backend.clone());
	let client = Arc::new(client);

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		config.prometheus_registry(),
		task_manager.spawn_essential_handle(),
		client.clone(),
	);

	let can_author_with = sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());

	// TODO: inner is not well defined here.
	let pow_block_import = sc_consensus_pow::PowBlockImport::new(
		client.clone(),
		client.clone(),
		sha3pow::MinimalSha3Algorithm,
		0, // check inherent starting at block 0
		select_chain.clone(),
		|_parent, ()| async { Ok(()) },
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
		other: pow_block_import,
	})
}

/// Builds a new service for a full client.
pub fn new_full(config: Configuration) -> Result<TaskManager, ServiceError> {
	let sc_service::PartialComponents {
		client,
		backend,
		mut task_manager,
		import_queue,
		keystore_container,
		select_chain,
		transaction_pool,
		other: pow_block_import,
	} = new_partial(&config)?;

	let (network, system_rpc_tx, network_starter) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			block_announce_validator_builder: None,
			warp_sync: None,
		})?;

	if config.offchain_worker.enabled {
		sc_service::build_offchain_workers(
			&config,
			task_manager.spawn_handle(),
			client.clone(),
			network.clone(),
		);
	}

	let rpc_extensions_builder = {
		let client = client.clone();
		let pool = transaction_pool.clone();

		Box::new(move |deny_unsafe, _| {
			let deps =
				crate::rpc::FullDeps { client: client.clone(), pool: pool.clone(), deny_unsafe };

			Ok(crate::rpc::create_full(deps))
		})
	};

	let is_authority = config.role.is_authority();
	let prometheus_registry = config.prometheus_registry().cloned();


	if is_authority {
		let proposer = sc_basic_authorship::ProposerFactory::new(
			task_manager.spawn_handle(),
			client.clone(),
			transaction_pool.clone(),
			prometheus_registry.as_ref(),
			None
		);

		let can_author_with =
			sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());

		// Parameter details:
		//   https://substrate.dev/rustdocs/latest/sc_consensus_pow/fn.start_mining_worker.html
		// Also refer to kulupu config:
		//   https://github.com/kulupu/kulupu/blob/master/src/service.rs
		let (_worker, worker_task) = sc_consensus_pow::start_mining_worker(
			Box::new(pow_block_import),
			client.clone(),
			select_chain,
			MinimalSha3Algorithm,
			proposer,
			network.clone(),
			network.clone(),
			None,
			// For block production we want to provide our inherent data provider
			|_parent, ()| async {
				Ok(InherentDataProvider)
			},
			// time to wait for a new block before starting to mine a new one
			Duration::from_secs(10),
			// how long to take to actually build the block (i.e. executing extrinsics)
			Duration::from_secs(10),
			can_author_with,
		);
		
		task_manager
			.spawn_essential_handle()
			.spawn_blocking("pow", DEFAULT_GROUP_NAME ,worker_task);
	}

	// spawn rpc tasks
	let _rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		network,
		client,
		keystore: keystore_container.sync_keystore(),
		task_manager: &mut task_manager,
		transaction_pool: transaction_pool,
		rpc_extensions_builder,
		backend,
		system_rpc_tx,
		config,
		telemetry: None,
	})?;

	network_starter.start_network();
	Ok(task_manager)
}

// TODO: Builds a new service for a light client.