use {
    crate::{
        in_memory::SharedMemoryReader,
        payload::{PayloadId, PayloadQueries, PayloadResponse},
    },
    moved_shared::primitives::B256,
    std::{collections::HashMap, convert::Infallible},
};

#[derive(Debug)]
pub struct InMemoryPayloadQueries {
    block_hashes: HashMap<PayloadId, B256>,
}

impl Default for InMemoryPayloadQueries {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryPayloadQueries {
    pub fn new() -> Self {
        Self {
            block_hashes: Default::default(),
        }
    }

    pub fn add_block_hash(&mut self, id: PayloadId, block_hash: B256) {
        self.block_hashes.insert(id, block_hash);
    }
}

impl PayloadQueries for InMemoryPayloadQueries {
    type Err = Infallible;
    type Storage = SharedMemoryReader;

    fn by_hash(
        &self,
        storage: &Self::Storage,
        block_hash: B256,
    ) -> Result<Option<PayloadResponse>, Self::Err> {
        Ok(storage.block_memory.by_hash(block_hash).map(|block| {
            let transactions = storage
                .transaction_memory
                .by_hashes(block.transaction_hashes())
                .into_iter()
                .map(|v| v.inner);

            PayloadResponse::from_block_with_transactions(block, transactions)
        }))
    }

    fn by_id(
        &self,
        storage: &Self::Storage,
        id: PayloadId,
    ) -> Result<Option<PayloadResponse>, Self::Err> {
        let Some(hash) = self.block_hashes.get(&id) else {
            return Ok(None);
        };

        self.by_hash(storage, *hash)
    }
}
