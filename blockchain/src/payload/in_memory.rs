use {
    crate::{
        block::{ExtendedBlock, ReadBlockMemory},
        in_memory::SharedMemoryReader,
        payload::{
            PayloadId, PayloadQueries, PayloadResponse,
            read::{InProgressPayloads, MaybePayloadResponse},
        },
        transaction::ReadTransactionMemory,
    },
    std::convert::Infallible,
    umi_shared::primitives::B256,
};

#[derive(Debug, Clone)]
pub struct InMemoryPayloadQueries {
    in_progress: InProgressPayloads,
}

impl InMemoryPayloadQueries {
    pub fn new(in_progress: InProgressPayloads) -> Self {
        Self { in_progress }
    }

    fn block_into_payload(storage: &SharedMemoryReader, block: ExtendedBlock) -> PayloadResponse {
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
            .map(|block| Self::block_into_payload(storage, block)))
    }

    fn by_id(
        &self,
        storage: &Self::Storage,
        id: PayloadId,
    ) -> Result<MaybePayloadResponse, Self::Err> {
        if let Some(delayed) = self.in_progress.get_delayed(&id) {
            return Ok(MaybePayloadResponse::Delayed(delayed));
        }

        let response = storage
            .block_memory
            .by_payload_id(id)
            .map_or(MaybePayloadResponse::Unknown, |block| {
                MaybePayloadResponse::Some(Self::block_into_payload(storage, block))
            });
        Ok(response)
    }

    fn get_in_progress(&self) -> InProgressPayloads {
        self.in_progress.clone()
    }
}
