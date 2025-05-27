mod read;

#[cfg(any(feature = "test-doubles", test))]
pub use read::test_doubles::MockStateQueries;
pub use {
    moved_state::EthTrieResolver,
    read::{
        Balance, BlockHeight, InMemoryStateQueries, Nonce, ProofResponse, StateQueries,
        StorageProof, Version, proof_from_trie_and_resolver,
    },
};
