use {
    crate::state::{Balance, BlockHeight, Nonce, ProofResponse, StateQueries},
    eth_trie::{EthTrie, MemoryDB},
    move_core_types::account_address::AccountAddress,
    move_table_extension::TableResolver,
    move_vm_types::resolver::MoveResolver,
    moved_evm_ext::state::StorageTrieRepository,
    moved_shared::primitives::U256,
    moved_state::EthTrieResolver,
    std::sync::Arc,
};

#[derive(Debug, Clone)]
pub struct MockStateQueries(pub AccountAddress, pub BlockHeight);

impl StateQueries for MockStateQueries {
    fn balance_at(
        &self,
        _evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Balance> {
        assert_eq!(account, self.0);
        assert_eq!(height, self.1);

        Some(U256::from(5))
    }

    fn nonce_at(
        &self,
        _evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Nonce> {
        assert_eq!(account, self.0);
        assert_eq!(height, self.1);

        Some(3)
    }

    fn proof_at(
        &self,
        _evm_storage: &impl StorageTrieRepository,
        _account: AccountAddress,
        _storage_slots: &[U256],
        _height: BlockHeight,
    ) -> Option<ProofResponse> {
        None
    }

    fn resolver_at(&self, _: BlockHeight) -> impl MoveResolver + TableResolver + '_ {
        EthTrieResolver::new(EthTrie::new(Arc::new(MemoryDB::new(true))))
    }
}
