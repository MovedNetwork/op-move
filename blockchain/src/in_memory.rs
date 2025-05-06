use crate::{
    block::BlockMemory,
    transaction::{TransactionMemory, TransactionMemoryReader},
};

#[derive(Debug)]
pub struct SharedMemoryReader {
    pub block_memory: BlockMemory,
    pub transaction_memory: TransactionMemoryReader,
}

impl SharedMemoryReader {
    pub fn new(block_memory: BlockMemory, transaction_memory: TransactionMemoryReader) -> Self {
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
