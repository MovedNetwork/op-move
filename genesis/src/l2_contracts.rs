use {
    alloy::genesis::Genesis, move_core_types::effects::ChangeSet,
    moved_evm_ext::storage::StorageTrieRepository, moved_state::State,
};

pub fn init_state(
    genesis: Genesis,
    state: &impl State,
    storage_trie: &impl StorageTrieRepository,
) -> ChangeSet {
    moved_evm_ext::genesis_state_changes(genesis, state.resolver(), storage_trie)
}
