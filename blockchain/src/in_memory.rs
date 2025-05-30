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
    pub const fn new(
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
    pub const fn new(block_memory: BlockMemory, transaction_memory: TransactionMemory) -> Self {
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
        let (r3, w3) = evmap::new();
        let bw = BlockMemory::new(w1, w2, w3);
        let br = BlockMemoryReader::new(r1, r2, r3);
        let (r1, w1) = evmap::new();
        let tw = TransactionMemory::new(w1);
        let tr = TransactionMemoryReader::new(r1);
        let w = SharedMemory::new(bw, tw);
        let r = SharedMemoryReader::new(br, tr);

        (r, w)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::block::{Block, ExtendedBlock, Header, ReadBlockMemory},
        alloy::hex,
        umi_shared::primitives::B256,
    };

    #[test]
    fn test_block_reader_is_connected_to_block_writer() {
        let (r, mut w) = shared_memory::new();

        w.block_memory.add(ExtendedBlock::default());
        let actual_block = r.block_memory.by_height(0);
        let expected_block = Some(ExtendedBlock::default());

        assert_eq!(actual_block, expected_block);
    }

    #[test]
    fn test_block_reader_counts_height_based_on_additions_to_block_writer() {
        let (r, mut w) = shared_memory::new();

        w.block_memory.add(ExtendedBlock::default());
        let actual_height = r.block_memory.height();
        let expected_height = Some(0);

        assert_eq!(actual_height, expected_height);

        let block = ExtendedBlock {
            hash: B256::new(hex!(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            )),
            block: Block {
                header: Header {
                    number: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        w.block_memory.add(block);

        let actual_height = r.block_memory.height();
        let expected_height = Some(1);

        assert_eq!(actual_height, expected_height);
    }
}
