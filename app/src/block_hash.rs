use {
    alloy::primitives::B256,
    std::{
        collections::VecDeque,
        sync::{Arc, RwLock},
    },
    umi_blockchain::block::BlockQueries,
    umi_evm_ext::state::{BlockHashLookup, BlockHashWriter},
};

const BLOCKHASH_HISTORY_SIZE: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
struct BlockHashEntry {
    number: u64,
    hash: B256,
}

#[derive(Debug, Clone)]
pub struct BlockHashRingBuffer {
    entries: VecDeque<BlockHashEntry>,
    /// Block number of the latest block stored for validation purposes
    latest_block: u64,
}

impl Default for BlockHashRingBuffer {
    fn default() -> Self {
        Self {
            entries: VecDeque::with_capacity(BLOCKHASH_HISTORY_SIZE),
            latest_block: 0,
        }
    }
}

impl BlockHashRingBuffer {
    pub fn push(&mut self, block_number: u64, block_hash: B256) {
        // Ensure blocks are added in sequence (detect potential reorgs)
        if block_number > 0 && self.latest_block > 0 && block_number != self.latest_block + 1 {
            eprintln!(
                "WARN: Block hash buffer - expected block {}, got {}",
                self.latest_block + 1,
                block_number
            );
        }

        self.entries.push_back(BlockHashEntry {
            number: block_number,
            hash: block_hash,
        });

        while self.entries.len() > BLOCKHASH_HISTORY_SIZE {
            self.entries.pop_front();
        }

        self.latest_block = block_number;
    }

    // TODO: make this a builder method?
    pub fn initialize_from_storage<S, B>(&mut self, storage: &S, block_query: &B, latest_block: u64)
    where
        B: BlockQueries<Storage = S>,
    {
        self.entries.clear();
        self.latest_block = 0;

        let start_block = if latest_block >= BLOCKHASH_HISTORY_SIZE as u64 {
            latest_block - BLOCKHASH_HISTORY_SIZE as u64 + 1
        } else {
            0
        };

        for block_num in start_block..=latest_block {
            if let Ok(Some(block)) = block_query.by_height(storage, block_num, false) {
                self.push(block_num, block.0.header.hash);
            }
        }
    }
}

impl BlockHashLookup for BlockHashRingBuffer {
    fn hash_by_number(&self, block_number: u64) -> Option<B256> {
        if block_number > self.latest_block {
            // Future block
            return None;
        }

        let blocks_ago = self.latest_block - block_number;
        if blocks_ago > BLOCKHASH_HISTORY_SIZE as u64 {
            // Too old
            return None;
        }

        for entry in self.entries.iter().rev() {
            if entry.number == block_number {
                return Some(entry.hash);
            }
            // Since we're going backwards chronologically, if we've passed the target block, it's not there
            if entry.number < block_number {
                break;
            }
        }

        None
    }
}

impl BlockHashWriter for BlockHashRingBuffer {
    fn push(&mut self, height: u64, hash: B256) {
        self.push(height, hash);
    }
}

#[derive(Debug, Clone)]
pub struct SharedBlockHashCache {
    inner: Arc<RwLock<BlockHashRingBuffer>>,
}

impl SharedBlockHashCache {
    pub fn initialize_from_storage<S, B>(&mut self, storage: &S, block_query: &B, latest_block: u64)
    where
        B: BlockQueries<Storage = S>,
    {
        if let Ok(mut cache) = self.inner.write() {
            cache.initialize_from_storage(storage, block_query, latest_block);
        }
    }
}

impl Default for SharedBlockHashCache {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(BlockHashRingBuffer::default())),
        }
    }
}

impl BlockHashLookup for SharedBlockHashCache {
    fn hash_by_number(&self, number: u64) -> Option<B256> {
        if let Ok(cache) = self.inner.read() {
            cache.hash_by_number(number)
        } else {
            // TODO: deal with poisoning?
            None
        }
    }
}

impl BlockHashWriter for SharedBlockHashCache {
    fn push(&mut self, height: u64, hash: B256) {
        if let Ok(mut cache) = self.inner.write() {
            cache.push(height, hash)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_basic_operations() {
        let mut buffer = BlockHashRingBuffer::default();
        assert!(buffer.entries.is_empty());

        let hash1 = B256::from([1u8; 32]);
        let hash2 = B256::from([2u8; 32]);

        buffer.push(1, hash1);
        buffer.push(2, hash2);

        assert_eq!(buffer.entries.len(), 2);
        assert_eq!(buffer.hash_by_number(1), Some(hash1));
        assert_eq!(buffer.hash_by_number(2), Some(hash2));
        assert_eq!(buffer.latest_block, 2);
    }

    #[test]
    fn test_ring_buffer_capacity() {
        let mut buffer = BlockHashRingBuffer::default();

        // Fill beyond capacity
        for i in 0..300u64 {
            let hash = B256::from([(i % 256) as u8; 32]);
            buffer.push(i, hash);
        }

        assert_eq!(buffer.entries.len(), BLOCKHASH_HISTORY_SIZE);

        // Should only have the most recent 256 blocks
        // Last stored block is 299, and 299 - 255 = 44
        assert_eq!(
            buffer.entries.front(),
            Some(BlockHashEntry {
                number: 44,
                hash: B256::from([44; 32])
            })
            .as_ref()
        );
        assert_eq!(buffer.latest_block, 299);
    }

    #[test]
    fn test_out_of_range_blocks() {
        let mut buffer = BlockHashRingBuffer::default();

        buffer.push(100, B256::from([1u8; 32]));
        buffer.push(101, B256::from([2u8; 32]));

        // Block too far in the past (more than 256 blocks ago) should return None
        // Current block is 101, so after the loop block 399 - 256 = 143 would be too old
        for i in 102..400u64 {
            buffer.push(i, B256::from([(i % 256) as u8; 32]));
        }

        // Block 100 should be too old
        assert_eq!(buffer.hash_by_number(100), None);

        // Block 143 should be the latest one inaccessible
        assert_eq!(buffer.hash_by_number(143), None);

        // Block 144 right after 143 is still accessible
        assert!(buffer.hash_by_number(144).is_some());
    }
}
