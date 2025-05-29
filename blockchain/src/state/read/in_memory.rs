use {
    crate::{
        block::ReadBlockMemory,
        in_memory::SharedMemoryReader,
        state::{
            Balance, BlockHeight, Nonce, ProofResponse, StateQueries, proof_from_trie_and_resolver,
            read::model::ReadStateRoot,
        },
    },
    eth_trie::{DB, EthTrie},
    move_core_types::account_address::AccountAddress,
    move_table_extension::TableResolver,
    move_vm_types::resolver::MoveResolver,
    moved_evm_ext::state::StorageTrieRepository,
    moved_execution::{
        quick_get_eth_balance, quick_get_nonce,
        transaction::{L2_HIGHEST_ADDRESS, L2_LOWEST_ADDRESS},
    },
    moved_shared::primitives::{B256, ToEthAddress, U256},
    moved_state::{EthTrieResolver, IN_MEMORY_EXPECT_MSG},
    std::sync::Arc,
};

impl ReadStateRoot for SharedMemoryReader {
    fn root_by_height(&self, height: BlockHeight) -> Option<B256> {
        self.block_memory
            .map_by_height(height, |v| v.block.header.state_root)
    }

    fn height(&self) -> BlockHeight {
        self.block_memory
            .height()
            .expect("Genesis should not be missing")
    }
}

#[derive(Debug)]
pub struct InMemoryStateQueries<
    R: ReadStateRoot = SharedMemoryReader,
    D: DB = moved_state::InMemoryTrieDb,
> {
    memory: R,
    db: Arc<D>,
    genesis_state_root: B256,
}

impl<R: ReadStateRoot + Clone, D: DB> Clone for InMemoryStateQueries<R, D> {
    fn clone(&self) -> Self {
        Self::new(
            self.memory.clone(),
            self.db.clone(),
            self.genesis_state_root,
        )
    }
}

impl<R: ReadStateRoot, D: DB> InMemoryStateQueries<R, D> {
    pub fn new(memory: R, db: Arc<D>, genesis_state_root: B256) -> Self {
        Self {
            memory,
            db,
            genesis_state_root,
        }
    }

    fn root_by_height(&self, height: BlockHeight) -> Option<B256> {
        if height == 0 {
            return Some(self.genesis_state_root);
        }

        self.memory.root_by_height(height)
    }

    fn resolver(&self, height: BlockHeight) -> Option<impl MoveResolver + TableResolver + '_> {
        Some(EthTrieResolver::new(
            EthTrie::from(self.db.clone(), self.root_by_height(height)?)
                .expect("State root should be valid"),
        ))
    }
}

impl<R: ReadStateRoot, D: DB> StateQueries for InMemoryStateQueries<R, D> {
    fn balance_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Balance> {
        let resolver = self.resolver(height)?;

        Some(quick_get_eth_balance(&account, &resolver, evm_storage))
    }

    fn nonce_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Nonce> {
        let resolver = self.resolver(height)?;

        Some(quick_get_nonce(&account, &resolver, evm_storage))
    }

    fn proof_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        storage_slots: &[U256],
        height: BlockHeight,
    ) -> Option<ProofResponse> {
        let address = account.to_eth_address();

        // Only L2 contract addresses supported at this time
        if address < L2_LOWEST_ADDRESS || L2_HIGHEST_ADDRESS < address {
            return None;
        }

        let root = self.root_by_height(height)?;
        let resolver = self.resolver(height)?;
        let mut tree = EthTrie::from(self.db.clone(), root).expect(IN_MEMORY_EXPECT_MSG);

        proof_from_trie_and_resolver(address, storage_slots, &mut tree, &resolver, evm_storage)
    }

    fn resolver_at(&self, height: BlockHeight) -> impl MoveResolver + TableResolver + '_ {
        self.resolver(height).unwrap()
    }
}
