use {
    crate::{InMemoryTrieDb, state::EthTrieState},
    std::sync::Arc,
};

pub type InMemoryState = EthTrieState<InMemoryTrieDb>;

impl Default for InMemoryState {
    fn default() -> Self {
        Self::empty(Arc::new(InMemoryTrieDb::empty()))
    }
}
