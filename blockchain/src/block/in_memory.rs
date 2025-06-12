use {
    crate::{block::ExtendedBlock, payload::PayloadId},
    std::{
        collections::HashMap,
        sync::{Arc, RwLock},
    },
    umi_shared::primitives::B256,
};

/// A storage for blocks that keeps data in memory.
///
/// The repository keeps data stored locally and its memory is not shared outside the struct. It
/// maintains a set of indices for efficient lookup.
#[derive(Debug, Default, Clone)]
pub struct BlockMemory {
    hashes: Arc<RwLock<HashMap<B256, Arc<ExtendedBlock>>>>,
    heights: Arc<RwLock<HashMap<u64, Arc<ExtendedBlock>>>>,
    payload_ids: Arc<RwLock<HashMap<PayloadId, Arc<ExtendedBlock>>>>,
}

impl BlockMemory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, block: ExtendedBlock) {
        let block = Arc::new(block);
        self.hashes
            .write()
            .unwrap()
            .insert(block.hash, block.clone());
        self.heights
            .write()
            .unwrap()
            .insert(block.block.header.number, block.clone());
        self.payload_ids
            .write()
            .unwrap()
            .insert(block.payload_id, block.clone());
    }
}

pub trait ReadBlockMemory {
    fn by_hash(&self, hash: B256) -> Option<ExtendedBlock>;
    fn by_payload_id(&self, payload_id: PayloadId) -> Option<ExtendedBlock>;
    fn by_height(&self, height: u64) -> Option<ExtendedBlock> {
        self.map_by_height(height, Clone::clone)
    }
    fn map_by_height<U>(&self, height: u64, f: impl FnOnce(&'_ ExtendedBlock) -> U) -> Option<U>;
    fn height(&self) -> Option<u64>;
    fn last(&self) -> Option<ExtendedBlock> {
        self.by_height(self.height()?)
    }
}

impl ReadBlockMemory for BlockMemory {
    fn by_hash(&self, hash: B256) -> Option<ExtendedBlock> {
        self.hashes
            .read()
            .unwrap()
            .get(&hash)
            .map(|b| ExtendedBlock::clone(b))
    }

    fn by_payload_id(&self, payload_id: PayloadId) -> Option<ExtendedBlock> {
        self.payload_ids
            .read()
            .unwrap()
            .get(&payload_id)
            .map(|b| ExtendedBlock::clone(b))
    }

    fn map_by_height<U>(&self, height: u64, f: impl FnOnce(&'_ ExtendedBlock) -> U) -> Option<U> {
        self.heights.read().unwrap().get(&height).map(|b| f(b))
    }

    fn height(&self) -> Option<u64> {
        let n = self.heights.read().unwrap().len() as u64;
        n.checked_sub(1)
    }
}
