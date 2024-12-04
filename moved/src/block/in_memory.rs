use {
    crate::{
        block::{
            root::{BlockQueries, BlockRepository},
            ExtendedBlock,
        },
        primitives::B256,
        types::state::BlockResponse,
    },
    std::collections::HashMap,
};

/// A storage for blocks that keeps data in memory.
///
/// The repository keeps data stored locally and its memory is not shared outside the struct. It
/// maintains a set of indices for efficient lookup.
#[derive(Debug)]
pub struct BlockMemory {
    /// Collection of blocks ordered by insertion.
    blocks: Vec<ExtendedBlock>,
    /// Map where key is a block hash and value is a position in the `blocks` vector.
    hashes: HashMap<B256, usize>,
    /// Map where key is a block height and value is a position in the `blocks` vector.
    heights: HashMap<u64, usize>,
}

impl Default for BlockMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockMemory {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            hashes: HashMap::new(),
            heights: HashMap::new(),
        }
    }
}

impl BlockMemory {
    pub fn add(&mut self, block: ExtendedBlock) {
        let index = self.blocks.len();
        self.hashes.insert(block.hash, index);
        self.heights.insert(block.block.header.number, index);
        self.blocks.push(block);
    }

    pub fn by_hash(&self, hash: B256) -> Option<ExtendedBlock> {
        let index = *self.hashes.get(&hash)?;
        self.blocks.get(index).cloned()
    }

    pub fn by_height(&self, height: u64) -> Option<ExtendedBlock> {
        let index = *self.heights.get(&height)?;
        self.blocks.get(index).cloned()
    }
}

/// Block repository that works with in memory backing store [`BlockMemory`].
#[derive(Debug)]
pub struct InMemoryBlockRepository;

impl Default for InMemoryBlockRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryBlockRepository {
    pub fn new() -> Self {
        Self
    }
}

impl BlockRepository for InMemoryBlockRepository {
    type Storage = BlockMemory;

    fn add(&mut self, mem: &mut BlockMemory, block: ExtendedBlock) {
        mem.add(block)
    }

    fn by_hash(&self, mem: &BlockMemory, hash: B256) -> Option<ExtendedBlock> {
        mem.by_hash(hash)
    }
}

/// Block query implementation that works with in memory backing store [`BlockMemory`].
#[derive(Debug)]
pub struct InMemoryBlockQueries;

impl BlockQueries for InMemoryBlockQueries {
    type Storage = BlockMemory;

    fn by_hash(
        &self,
        mem: &BlockMemory,
        hash: B256,
        include_transactions: bool,
    ) -> Option<BlockResponse> {
        if include_transactions {
            mem.by_hash(hash)
                .map(BlockResponse::from_block_with_transactions)
        } else {
            mem.by_hash(hash)
                .map(BlockResponse::from_block_with_transaction_hashes)
        }
    }

    fn by_height(
        &self,
        mem: &BlockMemory,
        height: u64,
        include_transactions: bool,
    ) -> Option<BlockResponse> {
        if include_transactions {
            mem.by_height(height)
                .map(BlockResponse::from_block_with_transactions)
        } else {
            mem.by_height(height)
                .map(BlockResponse::from_block_with_transaction_hashes)
        }
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use super::*;

    impl BlockQueries for () {
        type Storage = ();

        fn by_hash(&self, _: &Self::Storage, _: B256, _: bool) -> Option<BlockResponse> {
            None
        }

        fn by_height(&self, _: &Self::Storage, _: u64, _: bool) -> Option<BlockResponse> {
            None
        }
    }

    impl BlockRepository for () {
        type Storage = ();

        fn add(&mut self, _storage: &mut Self::Storage, _block: ExtendedBlock) {}

        fn by_hash(&self, _storage: &Self::Storage, _hash: B256) -> Option<ExtendedBlock> {
            None
        }
    }
}
