use {
    crate::{block::ExtendedBlock, payload::PayloadId},
    moved_shared::primitives::B256,
    std::sync::Arc,
};

pub type WriteHashes = evmap::WriteHandle<B256, Arc<ExtendedBlock>>;
pub type ReadHashes = evmap::ReadHandle<B256, Arc<ExtendedBlock>>;
pub type WriteHeights = evmap::WriteHandle<u64, Arc<ExtendedBlock>>;
pub type ReadHeights = evmap::ReadHandle<u64, Arc<ExtendedBlock>>;
pub type WritePayloadIds = evmap::WriteHandle<PayloadId, Arc<ExtendedBlock>>;
pub type ReadPayloadIds = evmap::ReadHandle<PayloadId, Arc<ExtendedBlock>>;

/// A storage for blocks that keeps data in memory.
///
/// The repository keeps data stored locally and its memory is not shared outside the struct. It
/// maintains a set of indices for efficient lookup.
#[derive(Debug)]
pub struct BlockMemory {
    hashes: WriteHashes,
    heights: WriteHeights,
    payload_ids: WritePayloadIds,
}

impl BlockMemory {
    pub const fn new(
        hashes: WriteHashes,
        heights: WriteHeights,
        payload_ids: WritePayloadIds,
    ) -> Self {
        Self {
            hashes,
            heights,
            payload_ids,
        }
    }

    pub fn add(&mut self, block: ExtendedBlock) {
        let block = Arc::new(block);
        self.hashes.insert(block.hash, block.clone());
        self.heights
            .insert(block.block.header.number, block.clone());
        self.payload_ids.insert(block.payload_id, block);
        self.hashes.refresh();
        self.heights.refresh();
        self.payload_ids.refresh();
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

impl AsRef<ReadPayloadIds> for BlockMemory {
    fn as_ref(&self) -> &ReadPayloadIds {
        &self.payload_ids
    }
}

#[derive(Debug, Clone)]
pub struct BlockMemoryReader {
    hashes: ReadHashes,
    heights: ReadHeights,
    payload_ids: ReadPayloadIds,
}

impl BlockMemoryReader {
    pub const fn new(
        hashes: ReadHashes,
        heights: ReadHeights,
        payload_ids: ReadPayloadIds,
    ) -> Self {
        Self {
            hashes,
            heights,
            payload_ids,
        }
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

impl AsRef<ReadPayloadIds> for BlockMemoryReader {
    fn as_ref(&self) -> &ReadPayloadIds {
        &self.payload_ids
    }
}

pub trait ReadBlockMemory {
    fn by_hash(&self, hash: B256) -> Option<ExtendedBlock>;
    fn by_payload_id(&self, payload_id: PayloadId) -> Option<ExtendedBlock>;
    fn by_height(&self, height: u64) -> Option<ExtendedBlock> {
        self.map_by_height(height, Clone::clone)
    }
    fn map_by_height<U>(&self, height: u64, f: impl FnOnce(&'_ ExtendedBlock) -> U) -> Option<U>;
    fn height(&self) -> u64;
    fn last(&self) -> Option<ExtendedBlock> {
        self.by_height(self.height())
    }
}

impl<T: AsRef<ReadHashes> + AsRef<ReadHeights> + AsRef<ReadPayloadIds>> ReadBlockMemory for T {
    fn by_hash(&self, hash: B256) -> Option<ExtendedBlock> {
        <T as AsRef<ReadHashes>>::as_ref(self)
            .get_one(&hash)
            .map(|v| ExtendedBlock::clone(&v))
    }

    fn by_payload_id(&self, payload_id: PayloadId) -> Option<ExtendedBlock> {
        <T as AsRef<ReadPayloadIds>>::as_ref(self)
            .get_one(&payload_id)
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
