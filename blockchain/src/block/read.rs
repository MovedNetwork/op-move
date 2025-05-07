use {
    crate::{block::write::ExtendedBlock, transaction::ExtendedTransaction},
    alloy::{eips::eip4895::Withdrawals, network::primitives::BlockTransactions},
    moved_shared::primitives::B256,
    std::{convert::Infallible, fmt::Debug},
};

pub trait BlockQueries: Debug {
    /// The associated error type for the backing storage access operation.
    type Err: Debug;
    /// The backing storage access handle type.
    type Storage;

    fn by_hash(
        &self,
        storage: &Self::Storage,
        hash: B256,
        include_transactions: bool,
    ) -> Result<Option<BlockResponse>, Self::Err>;

    fn by_height(
        &self,
        storage: &Self::Storage,
        height: u64,
        include_transactions: bool,
    ) -> Result<Option<BlockResponse>, Self::Err>;

    fn latest(&self, storage: &Self::Storage) -> Result<Option<u64>, Self::Err>;
}

type RpcBlock = alloy::rpc::types::Block<RpcTransaction>;
type RpcTransaction = op_alloy::rpc_types::Transaction;

#[derive(Debug)]
pub struct BlockResponse(pub RpcBlock);

impl BlockResponse {
    fn new(transactions: BlockTransactions<RpcTransaction>, value: ExtendedBlock) -> Self {
        Self(RpcBlock {
            transactions,
            header: alloy::rpc::types::Header {
                hash: value.hash,
                inner: value.block.header,
                // TODO: review fields below
                total_difficulty: None,
                size: None,
            },
            uncles: Vec::new(),
            withdrawals: Some(Withdrawals(Vec::new())),
        })
    }

    pub fn from_block_with_transaction_hashes(block: ExtendedBlock) -> Self {
        Self::new(
            BlockTransactions::Hashes(block.block.transactions.clone()),
            block,
        )
    }

    pub fn from_block_with_transactions(
        block: ExtendedBlock,
        transactions: Vec<ExtendedTransaction>,
    ) -> Self {
        Self::new(
            BlockTransactions::Full(transactions.into_iter().map(RpcTransaction::from).collect()),
            block,
        )
    }
}

pub mod in_memory {
    use {
        crate::{
            block::{BlockResponse, ReadBlockMemory, read::BlockQueries},
            in_memory::SharedMemoryReader,
            transaction::ReadTransactionMemory,
        },
        moved_shared::primitives::B256,
        std::convert::Infallible,
    };

    /// Block query implementation that works with in memory backing store [`BlockMemory`].
    ///
    /// [`BlockMemory`]: crate::block::BlockMemory
    #[derive(Debug, Clone)]
    pub struct InMemoryBlockQueries;

    impl BlockQueries for InMemoryBlockQueries {
        type Err = Infallible;
        type Storage = SharedMemoryReader;

        fn by_hash(
            &self,
            mem: &Self::Storage,
            hash: B256,
            include_transactions: bool,
        ) -> Result<Option<BlockResponse>, Self::Err> {
            Ok(if include_transactions {
                mem.block_memory.by_hash(hash).map(|block| {
                    let transactions = mem.transaction_memory.by_hashes(block.transaction_hashes());

                    BlockResponse::from_block_with_transactions(block, transactions)
                })
            } else {
                mem.block_memory
                    .by_hash(hash)
                    .map(BlockResponse::from_block_with_transaction_hashes)
            })
        }

        fn by_height(
            &self,
            mem: &Self::Storage,
            height: u64,
            include_transactions: bool,
        ) -> Result<Option<BlockResponse>, Self::Err> {
            Ok(if include_transactions {
                mem.block_memory.by_height(height).map(|block| {
                    let transactions = mem.transaction_memory.by_hashes(block.transaction_hashes());

                    BlockResponse::from_block_with_transactions(block, transactions)
                })
            } else {
                mem.block_memory
                    .by_height(height)
                    .map(BlockResponse::from_block_with_transaction_hashes)
            })
        }

        fn latest(&self, mem: &Self::Storage) -> Result<Option<u64>, Self::Err> {
            Ok(Some(mem.block_memory.height()))
        }
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use super::*;

    impl BlockQueries for () {
        type Err = Infallible;
        type Storage = ();

        fn by_hash(
            &self,
            _: &Self::Storage,
            _: B256,
            _: bool,
        ) -> Result<Option<BlockResponse>, Self::Err> {
            Ok(None)
        }

        fn by_height(
            &self,
            _: &Self::Storage,
            _: u64,
            _: bool,
        ) -> Result<Option<BlockResponse>, Self::Err> {
            Ok(None)
        }

        fn latest(&self, _: &Self::Storage) -> Result<Option<u64>, Self::Err> {
            Ok(None)
        }
    }
}
