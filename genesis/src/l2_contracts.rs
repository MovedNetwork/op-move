use {
    alloy::genesis::Genesis,
    moved_evm_ext::{storage::StorageTrieRepository, Changes},
    moved_state::State,
};

pub fn init_state(
    genesis: Genesis,
    state: &impl State,
    storage_trie: &impl StorageTrieRepository,
) -> Changes {
    moved_evm_ext::genesis_state_changes(genesis, state.resolver(), storage_trie)
}
