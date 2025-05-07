use crate::{
    block::{BlockMemory, BlockMemoryReader},
    transaction::{TransactionMemory, TransactionMemoryReader},
};

#[derive(Debug, Clone)]
pub struct SharedMemoryReader {
    pub block_memory: BlockMemoryReader,
    pub transaction_memory: TransactionMemoryReader,
}

impl SharedMemoryReader {
    pub fn new(
        block_memory: BlockMemoryReader,
        transaction_memory: TransactionMemoryReader,
    ) -> Self {
        Self {
            block_memory,
            transaction_memory,
        }
    }
}

#[derive(Debug)]
pub struct SharedMemory {
    pub block_memory: BlockMemory,
    pub transaction_memory: TransactionMemory,
}

impl SharedMemory {
    pub fn new(block_memory: BlockMemory, transaction_memory: TransactionMemory) -> Self {
        Self {
            block_memory,
            transaction_memory,
        }
    }
}

pub mod shared_memory {
    use crate::{
        block::{BlockMemory, BlockMemoryReader},
        in_memory::{SharedMemory, SharedMemoryReader},
        transaction::{TransactionMemory, TransactionMemoryReader},
    };

    pub fn new() -> (SharedMemoryReader, SharedMemory) {
        let (r1, w1) = evmap::new();
        let (r2, w2) = evmap::new();
        let bw = BlockMemory::new(w1, w2);
        let br = BlockMemoryReader::new(r1, r2);
        let (r1, w1) = evmap::new();
        let tw = TransactionMemory::new(w1);
        let tr = TransactionMemoryReader::new(r1);
        let w = SharedMemory::new(bw, tw);
        let r = SharedMemoryReader::new(br, tr);

        (r, w)
    }
}
