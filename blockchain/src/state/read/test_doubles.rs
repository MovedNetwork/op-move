use {
    crate::state::{
        Balance, BlockHeight, HeightToStateRootIndex, Nonce, ProofResponse, StateQueries,
    },
    eth_trie::{EthTrie, MemoryDB},
    move_core_types::account_address::AccountAddress,
    move_table_extension::TableResolver,
    move_vm_types::resolver::MoveResolver,
    std::{convert::Infallible, sync::Arc},
    umi_evm_ext::state::{self, StorageTrieRepository},
    umi_shared::primitives::{B256, U256},
    umi_state::EthTrieResolver,
};

#[derive(Debug, Clone)]
pub struct MockStateQueries(pub AccountAddress, pub BlockHeight);

impl StateQueries for MockStateQueries {
    fn balance_at(
        &self,
        _evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Result<Balance, state::Error> {
        assert_eq!(account, self.0);
        assert_eq!(height, self.1);

        Ok(U256::from(5))
    }

    fn nonce_at(
        &self,
        _evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Result<Nonce, state::Error> {
        assert_eq!(account, self.0);
        assert_eq!(height, self.1);

        Ok(3)
    }

    fn proof_at(
        &self,
        _evm_storage: &impl StorageTrieRepository,
        _account: AccountAddress,
        _storage_slots: &[U256],
        _height: BlockHeight,
    ) -> Result<ProofResponse, state::Error> {
        Ok(ProofResponse::default())
    }

    fn resolver_at(
        &self,
        _: BlockHeight,
    ) -> Result<impl MoveResolver + TableResolver + '_, state::Error> {
        Ok(EthTrieResolver::new(EthTrie::new(Arc::new(MemoryDB::new(
            true,
        )))))
    }
}

impl HeightToStateRootIndex for Vec<B256> {
    type Err = Infallible;

    fn root_by_height(&self, height: BlockHeight) -> Result<Option<B256>, Self::Err> {
        Ok(self.get(height as usize).cloned())
    }

    fn height(&self) -> Result<BlockHeight, Self::Err> {
        Ok(self.len() as u64 - 1)
    }

    fn push_state_root(&self, _state_root: B256) -> Result<(), Self::Err> {
        Ok(())
    }
}
