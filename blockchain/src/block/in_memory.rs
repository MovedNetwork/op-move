use {crate::block::ExtendedBlock, moved_shared::primitives::B256, std::sync::Arc};

/// A storage for blocks that keeps data in memory.
///
/// The repository keeps data stored locally and its memory is not shared outside the struct. It
/// maintains a set of indices for efficient lookup.
#[derive(Debug)]
pub struct BlockMemory {
    hashes: evmap::WriteHandle<B256, Arc<ExtendedBlock>>,
    heights: evmap::WriteHandle<u64, Arc<ExtendedBlock>>,
}

impl BlockMemory {
    pub fn new(
        hashes: evmap::WriteHandle<B256, Arc<ExtendedBlock>>,
        heights: evmap::WriteHandle<u64, Arc<ExtendedBlock>>,
    ) -> Self {
        Self { hashes, heights }
    }

    pub fn add(&mut self, block: ExtendedBlock) {
        let block = Arc::new(block);
        self.hashes.insert(block.hash, block.clone());
        self.heights.insert(block.block.header.number, block);
    }

    pub fn by_hash(&self, hash: B256) -> Option<ExtendedBlock> {
        self.hashes.get_one(&hash).map(|v| ExtendedBlock::clone(&v))
    }

    pub fn by_height(&self, height: u64) -> Option<ExtendedBlock> {
        self.heights
            .get_one(&height)
            .map(|v| ExtendedBlock::clone(&v))
    }

    pub fn height(&self) -> u64 {
        self.heights.len() as u64 - 1
    }

    pub fn last(&self) -> Option<ExtendedBlock> {
        self.by_height(self.height())
    }
}

#[derive(Debug)]
pub struct BlockMemoryReader {
    hashes: evmap::ReadHandle<B256, Arc<ExtendedBlock>>,
    heights: evmap::ReadHandle<u64, Arc<ExtendedBlock>>,
}

impl BlockMemoryReader {
    pub fn new(
        hashes: evmap::ReadHandle<B256, Arc<ExtendedBlock>>,
        heights: evmap::ReadHandle<u64, Arc<ExtendedBlock>>,
    ) -> Self {
        Self { hashes, heights }
    }
}

impl BlockMemoryReader {
    pub fn by_hash(&self, hash: B256) -> Option<ExtendedBlock> {
        self.hashes.get_one(&hash).map(|v| ExtendedBlock::clone(&v))
    }

    pub fn by_height(&self, height: u64) -> Option<ExtendedBlock> {
        self.heights
            .get_one(&height)
            .map(|v| ExtendedBlock::clone(&v))
    }

    pub fn height(&self) -> u64 {
        self.heights.len() as u64 - 1
    }

    pub fn last(&self) -> Option<ExtendedBlock> {
        self.by_height(self.height())
    }
}
