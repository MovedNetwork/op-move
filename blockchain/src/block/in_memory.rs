use {
    crate::block::ExtendedBlock,
    moved_shared::primitives::B256,
    std::{ops::Deref, sync::Arc},
};

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
}

impl BlockMemory {
    pub fn add(&mut self, block: ExtendedBlock) {
        let block = Arc::new(block);
        self.hashes.insert(block.hash, block.clone());
        self.heights.insert(block.block.header.number, block);
    }

    pub fn by_hash(&self, hash: B256) -> Option<ExtendedBlock> {
        let index = *self.hashes.get(&hash)?;
        self.blocks.get(index).cloned()
    }

    pub fn by_height(&self, height: u64) -> Option<ExtendedBlock> {
        let index = *self.heights.get(&height)?;
        self.blocks.get(index).cloned()
    }

    pub fn height(&self) -> u64 {
        self.heights.len() as u64 - 1
    }

    pub fn last(&self) -> Option<ExtendedBlock> {
        self.by_height(self.height())
    }
}
