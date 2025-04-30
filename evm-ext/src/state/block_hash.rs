use revm::primitives::B256;

/// Specialized trait to serve the EVM opcode 0x40:
/// https://www.evm.codes/?fork=cancun#40
pub trait BlockHashLookup {
    fn hash_by_number(&self, number: u64) -> Option<B256>;
}

impl BlockHashLookup for () {
    fn hash_by_number(&self, _number: u64) -> Option<B256> {
        None
    }
}
