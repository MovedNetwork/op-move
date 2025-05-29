mod in_memory;
mod model;
#[cfg(any(feature = "test-doubles", test))]
mod test_doubles;
#[cfg(test)]
mod tests;

#[cfg(any(feature = "test-doubles", test))]
pub use test_doubles::MockStateQueries;
pub use {
    in_memory::InMemoryStateQueries,
    model::{
        Balance, BlockHeight, Nonce, ProofResponse, ReadStateRoot, StateQueries, StorageProof,
        Version, proof_from_trie_and_resolver,
    },
};
