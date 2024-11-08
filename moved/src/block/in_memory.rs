use {
    crate::{
        block::{root::BlockRepository, ExtendedBlock},
        primitives::B256,
    },
    dashmap::DashMap,
    std::sync::Arc,
};

#[derive(Debug, Default)]
pub struct BlockMemory {
    /// Collection of blocks ordered indexed by an order of insertion.
    blocks: DashMap<usize, ExtendedBlock>,
    /// Map where key is a block hash and value is a position in the `blocks` vector.
    hashes: DashMap<B256, usize>,
    /// Map where key is a block number and value is a position in the `blocks` vector.
    numbers: DashMap<u64, usize>,
}

impl BlockMemory {
    pub fn insert(&self, block: ExtendedBlock) {
        let index = self.blocks.len();
        self.hashes.insert(block.hash, index);
        self.numbers.insert(block.block.header.number, index);
        self.blocks.insert(index, block);
    }

    pub fn by_hash(&self, hash: B256) -> Option<ExtendedBlock> {
        let index = *self.hashes.get(&hash)?;
        self.blocks.get(&index).map(|v| v.clone())
    }

    pub fn by_number(&self, number: u64) -> Option<ExtendedBlock> {
        let index = *self.numbers.get(&number)?;
        self.blocks.get(&index).map(|v| v.clone())
    }
}

/// Block repository that keeps data in memory.
///
/// The repository keeps data stored locally and its memory is not shared outside the struct.
#[derive(Debug)]
pub struct InMemoryBlockRepository {
    mem: Arc<BlockMemory>,
}

impl InMemoryBlockRepository {
    pub fn new(mem: Arc<BlockMemory>) -> Self {
        Self { mem }
    }
}

impl BlockRepository for InMemoryBlockRepository {
    fn add(&mut self, block: ExtendedBlock) {
        self.mem.insert(block)
    }

    fn by_hash(&self, hash: B256) -> Option<ExtendedBlock> {
        self.mem.by_hash(hash)
    }
}
