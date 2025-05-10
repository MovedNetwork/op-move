use {
    crate::{
        block::{ExtendedBlock, ReadBlockMemory},
        in_memory::SharedMemoryReader,
        payload::{PayloadId, PayloadQueries, PayloadResponse},
        transaction::ReadTransactionMemory,
    },
    moved_shared::primitives::B256,
    std::convert::Infallible,
};

#[derive(Debug, Clone)]
pub struct InMemoryPayloadQueries;

impl Default for InMemoryPayloadQueries {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryPayloadQueries {
    pub fn new() -> Self {
        Self
    }

    fn convert(storage: &SharedMemoryReader, block: ExtendedBlock) -> PayloadResponse {
        let transactions = storage
            .transaction_memory
            .by_hashes(block.transaction_hashes())
            .into_iter()
            .map(|v| v.inner);

        PayloadResponse::from_block_with_transactions(block, transactions)
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
        Ok(storage
            .block_memory
            .by_hash(block_hash)
            .map(|block| Self::convert(storage, block)))
    }

    fn by_id(
        &self,
        storage: &Self::Storage,
        id: PayloadId,
    ) -> Result<Option<PayloadResponse>, Self::Err> {
        Ok(storage
            .block_memory
            .by_payload_id(id)
            .map(|block| Self::convert(storage, block)))
    }
}
