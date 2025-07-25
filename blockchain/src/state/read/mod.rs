mod eth_trie;
mod in_memory;
mod model;
mod response;
#[cfg(any(feature = "test-doubles", test))]
mod test_doubles;
#[cfg(test)]
mod tests;

#[cfg(any(feature = "test-doubles", test))]
pub use test_doubles::MockStateQueries;
pub use {
    eth_trie::EthTrieStateQueries,
    in_memory::InMemoryStateQueries,
    model::{
        Balance, BlockHeight, HeightToStateRootIndex, Nonce, ProofResponse, StateQueries,
        StorageProof, Version, proof_from_trie_and_resolver,
    },
    response::*,
};
