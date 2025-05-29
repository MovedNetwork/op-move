use {
    alloy::primitives::B256, umi_blockchain::block::BlockQueries,
    umi_evm_ext::state::BlockHashLookup,
};

pub struct StorageBasedProvider<'a, S, B> {
    storage: &'a S,
    block_query: &'a B,
}

impl<'a, S, B> StorageBasedProvider<'a, S, B>
where
    B: BlockQueries<Storage = S>,
{
    pub fn new(storage: &'a S, block_query: &'a B) -> Self {
        Self {
            storage,
            block_query,
        }
    }
}

// TODO: looking up the entire block just to get its hash is inefficient;
// especially since we do not need the whole history, only the most recent 256
// block hashes. This implementation should be replaced with something better.
impl<S, B> BlockHashLookup for StorageBasedProvider<'_, S, B>
where
    B: BlockQueries<Storage = S>,
{
    fn hash_by_number(&self, number: u64) -> Option<B256> {
        let block = self
            .block_query
            .by_height(self.storage, number, false)
            .ok()??;
        Some(block.0.header.hash)
    }
}
