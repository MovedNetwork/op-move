use {
    alloy::genesis::Genesis,
    umi_evm_ext::{Changes, state::StorageTrieRepository},
    umi_state::State,
};

pub fn init_state(
    genesis: Genesis,
    state: &impl State,
    storage_trie: &impl StorageTrieRepository,
) -> Changes {
    umi_evm_ext::genesis_state_changes(genesis, state.resolver(), storage_trie)
}
