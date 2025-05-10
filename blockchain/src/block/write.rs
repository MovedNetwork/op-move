use {
    crate::payload::PayloadId,
    moved_shared::primitives::{B256, U256},
    std::fmt::Debug,
};

pub type Header = alloy::consensus::Header;

#[derive(Debug, Clone, Hash, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct ExtendedBlock {
    /// The block hash is the output of keccak-256 algorithm with RLP encoded block header as input.
    pub hash: B256,
    /// The block value is total value in Wei expected to be received by the fee recipient. It is
    /// the gas paid on top of the base fee.
    ///
    /// The base fee is burned to prevent malicious behavior.
    ///
    /// Burning the base fee hinders a block producer's ability to manipulate transactions. For
    /// example, if block producers received the base fee, they could include their own transactions
    /// for free and raise the base fee for everyone else. Alternatively, they could refund the base
    /// fee to some users off-chain, leading to a more opaque and complex transaction fee market.
    pub value: U256,
    pub payload_id: PayloadId,
    pub block: Block,
}

impl ExtendedBlock {
    pub fn new(hash: B256, value: U256, payload_id: PayloadId, block: Block) -> Self {
        Self {
            hash,
            value,
            payload_id,
            block,
        }
    }

    pub fn with_value(mut self, value: U256) -> Self {
        self.value = value;
        self
    }

    pub fn with_payload_id(mut self, payload_id: PayloadId) -> Self {
        self.payload_id = payload_id;
        self
    }

    pub fn transaction_hashes(&self) -> impl Iterator<Item = B256> + use<'_> {
        self.block.transactions.iter().copied()
    }
}

/// TODO: Add withdrawals
#[derive(Debug, Clone, Hash, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Block {
    pub header: Header,
    pub transactions: Vec<B256>,
}

impl Block {
    pub fn new(header: Header, transactions: Vec<B256>) -> Self {
        Self {
            header,
            transactions,
        }
    }

    pub fn with_hash(self, hash: B256) -> ExtendedBlock {
        ExtendedBlock::new(hash, U256::ZERO, PayloadId::from(0u64), self)
    }
}

pub trait BlockRepository: Debug {
    /// The associated error type for the backing storage access operation.
    type Err: Debug;
    /// The backing storage access handle type.
    type Storage;

    fn add(&mut self, storage: &mut Self::Storage, block: ExtendedBlock) -> Result<(), Self::Err>;

    fn by_hash(
        &self,
        storage: &Self::Storage,
        hash: B256,
    ) -> Result<Option<ExtendedBlock>, Self::Err>;

    fn latest(&self, storage: &Self::Storage) -> Result<Option<ExtendedBlock>, Self::Err>;
}

pub mod in_memory {
    use {
        crate::{
            block::{
                ReadBlockMemory,
                write::{BlockRepository, ExtendedBlock},
            },
            in_memory::SharedMemory,
        },
        moved_shared::primitives::B256,
        std::convert::Infallible,
    };

    /// Block repository that works with in memory backing store [`BlockMemory`].
    ///
    /// [`BlockMemory`]: crate::block::BlockMemory
    #[derive(Debug)]
    pub struct InMemoryBlockRepository;

    impl Default for InMemoryBlockRepository {
        fn default() -> Self {
            Self::new()
        }
    }

    impl InMemoryBlockRepository {
        pub fn new() -> Self {
            Self
        }
    }

    impl BlockRepository for InMemoryBlockRepository {
        type Err = Infallible;
        type Storage = SharedMemory;

        fn add(&mut self, mem: &mut Self::Storage, block: ExtendedBlock) -> Result<(), Self::Err> {
            mem.block_memory.add(block);
            Ok(())
        }

        fn by_hash(
            &self,
            mem: &Self::Storage,
            hash: B256,
        ) -> Result<Option<ExtendedBlock>, Self::Err> {
            Ok(mem.block_memory.by_hash(hash))
        }

        fn latest(&self, mem: &Self::Storage) -> Result<Option<ExtendedBlock>, Self::Err> {
            Ok(mem.block_memory.last())
        }
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use super::*;

    impl BlockRepository for () {
        type Err = ();
        type Storage = ();

        fn add(&mut self, _: &mut Self::Storage, _: ExtendedBlock) -> Result<(), Self::Err> {
            Ok(())
        }

        fn by_hash(&self, _: &Self::Storage, _: B256) -> Result<Option<ExtendedBlock>, Self::Err> {
            Ok(None)
        }

        fn latest(&self, _: &Self::Storage) -> Result<Option<ExtendedBlock>, Self::Err> {
            Ok(None)
        }
    }
}
