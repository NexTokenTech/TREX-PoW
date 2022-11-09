//! RPC interface for the transaction payment module.

use jsonrpsee::{
	core::{async_trait, Error as JsonRpseeError, RpcResult},
	proc_macros::rpc,
	types::error::{CallError, ErrorCode, ErrorObject},
};
pub use pallet_difficulty_runtime_api::DiffAdjustmentApi as DiffAdjustmentRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{traits::{Block as BlockT}};
use std::sync::Arc;
use sp_runtime::generic::BlockId;

#[rpc(server,client)]
pub trait DiffAdjustmentApi<BlockHash> {
	#[method(name = "difficulty_getAvgBlockTime")]
	fn get_avg_blocktime(&self, at: Option<BlockHash>) -> RpcResult<u32>;
}

pub struct DiffAdjustment<C, B> {
	// If you have more generics, no need to SumStorage<C, M, N, P, ...>
	// just use a tuple like SumStorage<C, (M, N, P, ...)>
	client: Arc<C>,
	_marker: std::marker::PhantomData<B>,
}

impl<C, B> DiffAdjustment<C, B> {
	/// Create new `SumStorage` instance with the given reference to the client.
	pub fn new(client: Arc<C>) -> Self {
		Self { client, _marker: Default::default() }
	}
}

#[async_trait]
impl<C,B> DiffAdjustmentApiServer<<B as BlockT>::Hash> for DiffAdjustment<C,B>
	where
		B: BlockT + 'static,
		C: HeaderBackend<B> + ProvideRuntimeApi<B>
		+ Send
		+ Sync
		+ 'static,
		C::Api: DiffAdjustmentRuntimeApi<B>,
{
	fn get_avg_blocktime(&self,at: Option<<B as BlockT>::Hash>) -> RpcResult<u32> {
		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or_else(||
			// If the block hash is not supplied assume the best block.
			self.client.info().best_hash));

		let runtime_api_result = api.get_avg_blocktime(&at);
		runtime_api_result.map_err(|e| {
			JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
				ErrorCode::InvalidParams.code(),
				format!("doesn't fit in NumberOrHex representation {:?}",e),
				None::<()>,
			)))
		})
	}
}