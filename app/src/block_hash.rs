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

impl BlockHashRingBuffer {
    pub const fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            latest_block: 0,
        }
    }

    pub fn push(&mut self, block_number: u64, block_hash: B256) {
        // Ensure blocks are added in sequence (detect potential reorgs)
        if block_number > 0 && self.latest_block > 0 && block_number != self.latest_block + 1 {
            // TODO: what to do here?
            println!(
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

    pub fn try_from_storage<S, B>(storage: &S, block_query: &B) -> Result<Self, B::Err>
    where
        B: BlockQueries<Storage = S>,
    {
        let latest_block = block_query.latest(storage)?.unwrap_or(0);

        let mut cache = Self::default();

        let start_block = if latest_block >= BLOCKHASH_HISTORY_SIZE as u64 {
            latest_block - BLOCKHASH_HISTORY_SIZE as u64 + 1
        } else {
            0
        };

        for block_num in start_block..=latest_block {
            if let Ok(Some(block)) = block_query.by_height(storage, block_num, false) {
                cache.push(block_num, block.0.header.hash);
            }
        }

        Ok(cache)
    }
}

impl Default for BlockHashRingBuffer {
    fn default() -> Self {
        Self {
            entries: VecDeque::with_capacity(BLOCKHASH_HISTORY_SIZE),
            latest_block: 0,
        }
    }
}

impl BlockHashLookup for BlockHashRingBuffer {
    fn hash_by_number(&self, block_number: u64) -> Option<B256> {
        if block_number > self.latest_block {
            return None;
        }

        let blocks_ago = (self.latest_block - block_number) as usize;

        // second condition to guard against cases when the buffer is not full
        // and starts from non-zero block
        if blocks_ago > BLOCKHASH_HISTORY_SIZE || blocks_ago >= self.entries.len() {
            return None;
        }

        let block_index = self.entries.len() - 1 - blocks_ago;

        self.entries.get(block_index).map(|e| e.hash)
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
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(BlockHashRingBuffer::new())),
        }
    }

    pub fn try_from_storage<S, B>(storage: &S, block_query: &B) -> Result<Self, B::Err>
    where
        B: BlockQueries<Storage = S>,
    {
        let buf = BlockHashRingBuffer::try_from_storage(storage, block_query)?;

        Ok(Self {
            inner: Arc::new(RwLock::new(buf)),
        })
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

#[derive(Debug, Clone)]
pub struct HybridBlockHashCache<S, B> {
    ring_buffer: BlockHashRingBuffer,
    storage: S,
    block_query: B,
}

impl<S, B> HybridBlockHashCache<S, B>
where
    B: BlockQueries<Storage = S>,
{
    pub const fn new(storage: S, block_query: B) -> Self {
        Self {
            ring_buffer: BlockHashRingBuffer::new(),
            storage,
            block_query,
        }
    }

    pub fn try_from_storage(storage: S, block_query: B) -> Result<Self, B::Err> {
        let ring_buffer = BlockHashRingBuffer::try_from_storage(&storage, &block_query)?;

        Ok(Self {
            ring_buffer,
            storage,
            block_query,
        })
    }
}

impl<S, B> BlockHashLookup for HybridBlockHashCache<S, B>
where
    B: BlockQueries<Storage = S>,
{
    fn hash_by_number(&self, block_number: u64) -> Option<B256> {
        if let Some(hash) = self.ring_buffer.hash_by_number(block_number) {
            return Some(hash);
        }

        if let Ok(Some(block)) = self
            .block_query
            .by_height(&self.storage, block_number, false)
        {
            Some(block.0.header.hash)
        } else {
            None
        }
    }
}

impl<S, B> BlockHashWriter for HybridBlockHashCache<S, B>
where
    B: BlockQueries<Storage = S>,
{
    fn push(&mut self, height: u64, hash: B256) {
        self.ring_buffer.push(height, hash);
    }
}

#[derive(Debug, Clone)]
pub struct SharedHybridBlockHashCache<S, B> {
    inner: Arc<RwLock<HybridBlockHashCache<S, B>>>,
}

impl<S, B> SharedHybridBlockHashCache<S, B>
where
    S: Clone,
    B: Clone + BlockQueries<Storage = S>,
{
    pub fn new(storage: S, block_query: B) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HybridBlockHashCache::new(storage, block_query))),
        }
    }

    pub fn try_from_storage(storage: S, block_query: B) -> Result<Self, B::Err> {
        let cache = HybridBlockHashCache::try_from_storage(storage, block_query)?;

        Ok(Self {
            inner: Arc::new(RwLock::new(cache)),
        })
    }
}

impl<S, B> BlockHashLookup for SharedHybridBlockHashCache<S, B>
where
    S: Clone,
    B: Clone + BlockQueries<Storage = S>,
{
    fn hash_by_number(&self, number: u64) -> Option<B256> {
        if let Ok(cache) = self.inner.read() {
            cache.hash_by_number(number)
        } else {
            // TODO: deal with poisoning?
            None
        }
    }
}

impl<S, B> BlockHashWriter for SharedHybridBlockHashCache<S, B>
where
    S: Clone,
    B: Clone + BlockQueries<Storage = S>,
{
    fn push(&mut self, height: u64, hash: B256) {
        if let Ok(mut cache) = self.inner.write() {
            cache.push(height, hash)
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        alloy::{
            consensus::Header,
            primitives::{U64, ruint::aliases::U256},
        },
        umi_blockchain::block::{Block, BlockResponse, ExtendedBlock},
    };

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

    #[derive(Debug)]
    struct MockBlockQueries;
    #[derive(Debug)]
    struct MockStorage;

    impl BlockQueries for MockBlockQueries {
        type Storage = MockStorage;
        type Err = ();

        fn by_hash(
            &self,
            _storage: &Self::Storage,
            _hash: B256,
            _include_transactions: bool,
        ) -> Result<Option<BlockResponse>, Self::Err> {
            Ok(None)
        }

        fn by_height(
            &self,
            _storage: &Self::Storage,
            height: u64,
            _include_transactions: bool,
        ) -> Result<Option<BlockResponse>, Self::Err> {
            // Mock storage returns a predictable hash for block numbers >= 1000
            if height >= 1000 {
                let hash = B256::from([(height % 256) as u8; 32]);
                let header = Header::default();
                let block = Block::new(header, Vec::new());
                let extended_block = ExtendedBlock::new(hash, U256::ZERO, U64::ZERO, block);
                let response =
                    BlockResponse::from_block_with_transactions(extended_block, Vec::new());
                Ok(Some(response))
            } else {
                Ok(None)
            }
        }

        fn latest(&self, _storage: &Self::Storage) -> Result<Option<u64>, Self::Err> {
            Ok(Some(2000))
        }
    }

    #[test]
    fn test_hybrid_cache_ring_buffer_hit() {
        let mut cache =
            HybridBlockHashCache::try_from_storage(&MockStorage, &MockBlockQueries).unwrap();

        let hash = B256::from([42u8; 32]);
        cache.push(100, hash);

        assert_eq!(cache.hash_by_number(100), Some(hash));
    }

    #[test]
    fn test_hybrid_cache_storage_fallback() {
        let cache =
            HybridBlockHashCache::try_from_storage(&MockStorage, &MockBlockQueries).unwrap();

        // Should fall back to storage for block 1500
        let expected_hash = B256::from([((1500 % 256) as u8); 32]);
        assert_eq!(cache.hash_by_number(1500), Some(expected_hash));
    }

    #[test]
    fn test_hybrid_cache_storage_miss() {
        let cache =
            HybridBlockHashCache::try_from_storage(&MockStorage, &MockBlockQueries).unwrap();

        // Should return None for blocks not in ring buffer or storage
        assert_eq!(cache.hash_by_number(500), None);
    }

    #[test]
    fn test_hybrid_cache_priority() {
        let mut cache =
            HybridBlockHashCache::try_from_storage(&MockStorage, &MockBlockQueries).unwrap();

        // Add block 1500 to ring buffer with different hash than storage would return
        let ring_buffer_hash = B256::from([99u8; 32]);
        cache.push(1500, ring_buffer_hash);

        // Should prefer ring buffer over storage
        assert_eq!(cache.hash_by_number(1500), Some(ring_buffer_hash));
    }
}
