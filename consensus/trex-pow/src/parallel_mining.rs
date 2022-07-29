use sc_client_api::{AuxStore, BlockOf};
use sc_consensus::{
    BlockCheckParams, BlockImport, BlockImportParams, ImportResult,
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::{well_known_cache_keys::Id as CacheKeyId, HeaderMetadata};
use sp_consensus::{Error as ConsensusError};
use sp_runtime::traits::{Block as BlockT};
use std::{collections::HashMap, fmt::Debug, marker::PhantomData, sync::Arc};
use std::sync::{
    atomic::{AtomicBool, Ordering},
};

/// Block import for weak subjectivity. It must be combined with a PoW block import.
pub struct ParallelBlockImport<B: BlockT, I, C> {
    inner: I,
    client: Arc<C>,
    found: Arc<AtomicBool>,
    _marker: PhantomData<B>,
}

impl<B: BlockT, I: Clone, C> Clone
for ParallelBlockImport<B, I, C>
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            client: self.client.clone(),
            found: self.found.clone(),
            _marker: PhantomData,
        }
    }
}

impl<B, I, C> ParallelBlockImport<B, I, C>
    where
        B: BlockT,
        I: BlockImport<B, Transaction = sp_api::TransactionFor<C, B>> + Send + Sync,
        I::Error: Into<ConsensusError>,
        C: ProvideRuntimeApi<B> + HeaderMetadata<B> + BlockOf + AuxStore + Send + Sync,
        C::Error: Debug
{
    /// Create a new block import for weak subjectivity.
    pub fn new(
        inner: I,
        client: Arc<C>,
        found: Arc<AtomicBool>
    ) -> Self {
        Self {
            inner,
            client,
            found,
            _marker: PhantomData,
        }
    }
}

#[async_trait::async_trait]
impl<B, I, C> BlockImport<B> for ParallelBlockImport<B, I, C>
    where
        B: BlockT,
        I: BlockImport<B, Transaction = sp_api::TransactionFor<C, B>> + Send + Sync,
        I::Error: Into<ConsensusError>,
        C: ProvideRuntimeApi<B> + HeaderMetadata<B> + BlockOf + AuxStore + Send + Sync + 'static,
        C::Error: Debug,
{
    type Error = ConsensusError;
    type Transaction = sp_api::TransactionFor<C, B>;

    async fn check_block(
        &mut self,
        block: BlockCheckParams<B>,
    ) -> Result<ImportResult, Self::Error> {
        self.inner.check_block(block).await.map_err(Into::into)
    }

    async fn import_block(
        &mut self,
        mut block: BlockImportParams<B, Self::Transaction>,
        new_cache: HashMap<CacheKeyId, Vec<u8>>,
    ) -> Result<ImportResult, Self::Error> {

        //Processing logic after imported a new block
        dbg!("!!!!!!!!!!!!!!!!!!!!!!!!before imported block");
        self.inner
            .import_block(block, new_cache)
            .await
            .map_err(Into::into)
    }
}