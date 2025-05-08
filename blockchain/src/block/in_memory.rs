use {crate::block::ExtendedBlock, moved_shared::primitives::B256, std::sync::Arc};

pub type WriteHashes = evmap::WriteHandle<B256, Arc<ExtendedBlock>>;
pub type ReadHashes = evmap::ReadHandle<B256, Arc<ExtendedBlock>>;
pub type WriteHeight = evmap::WriteHandle<u64, Arc<ExtendedBlock>>;
pub type ReadHeights = evmap::ReadHandle<u64, Arc<ExtendedBlock>>;

/// A storage for blocks that keeps data in memory.
///
/// The repository keeps data stored locally and its memory is not shared outside the struct. It
/// maintains a set of indices for efficient lookup.
#[derive(Debug)]
pub struct BlockMemory {
    hashes: WriteHashes,
    heights: WriteHeight,
}

impl BlockMemory {
    pub fn new(hashes: WriteHashes, heights: WriteHeight) -> Self {
        Self { hashes, heights }
    }

    pub fn add(&mut self, block: ExtendedBlock) {
        let block = Arc::new(block);
        self.hashes.insert(block.hash, block.clone());
        self.heights.insert(block.block.header.number, block);
    }
}

impl AsRef<ReadHeights> for BlockMemory {
    fn as_ref(&self) -> &ReadHeights {
        &self.heights
    }
}

impl AsRef<ReadHashes> for BlockMemory {
    fn as_ref(&self) -> &ReadHashes {
        &self.hashes
    }
}

#[derive(Debug, Clone)]
pub struct BlockMemoryReader {
    hashes: ReadHashes,
    heights: ReadHeights,
}

impl BlockMemoryReader {
    pub fn new(hashes: ReadHashes, heights: ReadHeights) -> Self {
        Self { hashes, heights }
    }
}

impl AsRef<ReadHeights> for BlockMemoryReader {
    fn as_ref(&self) -> &ReadHeights {
        &self.heights
    }
}

impl AsRef<ReadHashes> for BlockMemoryReader {
    fn as_ref(&self) -> &ReadHashes {
        &self.hashes
    }
}

pub trait ReadBlockMemory {
    fn by_hash(&self, hash: B256) -> Option<ExtendedBlock>;
    fn by_height(&self, height: u64) -> Option<ExtendedBlock> {
        self.map_by_height(height, Clone::clone)
    }
    fn map_by_height<U>(&self, height: u64, f: impl FnOnce(&'_ ExtendedBlock) -> U) -> Option<U>;
    fn height(&self) -> u64;
    fn last(&self) -> Option<ExtendedBlock> {
        self.by_height(self.height())
    }
}

impl<T: AsRef<ReadHashes> + AsRef<ReadHeights>> ReadBlockMemory for T {
    fn by_hash(&self, hash: B256) -> Option<ExtendedBlock> {
        <T as AsRef<ReadHashes>>::as_ref(self)
            .get_one(&hash)
            .map(|v| ExtendedBlock::clone(&v))
    }

    fn map_by_height<U>(&self, height: u64, f: impl FnOnce(&'_ ExtendedBlock) -> U) -> Option<U> {
        <T as AsRef<ReadHeights>>::as_ref(self)
            .get_one(&height)
            .map(|v| f(&v))
    }

    fn height(&self) -> u64 {
        <T as AsRef<ReadHeights>>::as_ref(self).len() as u64 - 1
    }
}
