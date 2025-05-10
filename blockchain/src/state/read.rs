use {
    crate::{block::ReadBlockMemory, in_memory::SharedMemoryReader},
    alloy::{
        primitives::keccak256,
        rpc::types::{EIP1186AccountProofResponse, EIP1186StorageProof},
    },
    aptos_types::state_store::{state_key::StateKey, state_value::StateValue},
    bytes::Bytes,
    eth_trie::{DB, EthTrie, Trie, TrieError},
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        account_address::AccountAddress,
        language_storage::{ModuleId, StructTag},
        metadata::Metadata,
        value::MoveTypeLayout,
        vm_status::StatusCode,
    },
    move_table_extension::{TableHandle, TableResolver},
    move_vm_types::resolver::{ModuleResolver, MoveResolver, ResourceResolver},
    moved_evm_ext::{
        ResolverBackedDB,
        state::{self, StorageTrieRepository},
    },
    moved_execution::{
        quick_get_eth_balance, quick_get_nonce,
        transaction::{L2_HIGHEST_ADDRESS, L2_LOWEST_ADDRESS},
    },
    moved_shared::primitives::{Address, B256, KeyHashable, ToEthAddress, U256},
    moved_state::{
        IN_MEMORY_EXPECT_MSG, evm_key_address, is_evm_storage_or_account_key, nodes::TreeKey,
    },
    std::{fmt::Debug, sync::Arc},
};

pub type ProofResponse = EIP1186AccountProofResponse;
pub type StorageProof = EIP1186StorageProof;

/// A non-negative integer for indicating the amount of base token on an account.
pub type Balance = U256;

/// A non-negative integer for indicating the nonce used for sending transactions by an account.
pub type Nonce = u64;

/// A non-negative integer for indicating the order of a block in the blockchain, used as a tag for
/// [`Version`].
pub type BlockHeight = u64;

/// A non-negative integer for versioning a set of changes in a historical order.
///
/// Typically, each version matches one transaction, but there is an exception for changes generated
/// on genesis.
pub type Version = u64;

/// Accesses blockchain state in any particular point in history to fetch some account values.
///
/// It is defined by these operations:
/// * [`Self::balance_at`] - To fetch an amount of base token in an account read in its smallest
///   denomination at given block height.
/// * [`Self::nonce_at`] - To fetch the nonce value set for an account at given block height.
pub trait StateQueries {
    /// Queries the blockchain state version corresponding with block `height` for the amount of
    /// base token associated with `account`.
    fn balance_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Balance>;

    /// Queries the blockchain state version corresponding with block `height` for the nonce value
    /// associated with `account`.
    fn nonce_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Nonce>;

    fn proof_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        storage_slots: &[U256],
        height: BlockHeight,
    ) -> Option<ProofResponse>;

    fn resolver_at<'a>(&'a self, height: BlockHeight) -> impl MoveResolver + TableResolver + 'a;
}

pub trait ReadStateRoot {
    fn root_by_height(&self, height: BlockHeight) -> Option<B256>;
    fn height(&self) -> BlockHeight;
}

impl ReadStateRoot for SharedMemoryReader {
    fn root_by_height(&self, height: BlockHeight) -> Option<B256> {
        self.block_memory
            .map_by_height(height, |v| v.block.header.state_root)
    }

    fn height(&self) -> BlockHeight {
        self.block_memory.height()
    }
}

#[derive(Debug)]
pub struct InMemoryStateQueries<
    R: ReadStateRoot = SharedMemoryReader,
    D: DB = moved_state::InMemoryTrieDb,
> {
    memory: R,
    db: Arc<D>,
}

impl<R: ReadStateRoot + Clone, D: DB> Clone for InMemoryStateQueries<R, D> {
    fn clone(&self) -> Self {
        Self::new(self.memory.clone(), self.db.clone())
    }
}

impl<R: ReadStateRoot, D: DB> InMemoryStateQueries<R, D> {
    pub fn new(memory: R, db: Arc<D>) -> Self {
        Self { memory, db }
    }

    fn root_by_height(&self, height: BlockHeight) -> Option<B256> {
        self.memory.root_by_height(height)
    }

    fn resolver<'a>(
        &'a self,
        height: BlockHeight,
    ) -> Option<impl MoveResolver + TableResolver + 'a> {
        Some(EthTrieResolver::new(
            EthTrie::from(self.db.clone(), self.root_by_height(height)?)
                .expect("State root should be valid"),
        ))
    }
}

pub fn proof_from_trie_and_resolver(
    address: Address,
    storage_slots: &[U256],
    tree: &mut EthTrie<impl DB>,
    resolver: &impl MoveResolver,
    storage_trie: &impl StorageTrieRepository,
) -> Option<ProofResponse> {
    let evm_db = ResolverBackedDB::new(storage_trie, resolver, &(), 0);

    // All L2 contract account data is part of the EVM state
    let account_info = evm_db.get_account(&address).ok()??;

    let account_key = TreeKey::Evm(address);
    let account_proof = tree
        .get_proof(account_key.key_hash().0.as_slice())
        .ok()?
        .into_iter()
        .map(Into::into)
        .collect();

    let storage_proof = if storage_slots.is_empty() {
        Vec::new()
    } else {
        let mut storage = storage_trie
            .for_account_with_root(&address, &account_info.inner.storage_root)
            .ok()?;

        storage_slots
            .iter()
            .filter_map(|index| {
                let key = keccak256::<[u8; 32]>(index.to_be_bytes());
                storage.proof(key.as_slice()).ok().map(|proof| {
                    let value = storage.get(index)?.unwrap_or_default();

                    Ok::<StorageProof, state::Error>(StorageProof {
                        key: (*index).into(),
                        value,
                        proof: proof.into_iter().map(Into::into).collect(),
                    })
                })
            })
            .collect::<Result<_, _>>()
            .unwrap()
    };

    Some(ProofResponse {
        address,
        balance: account_info.inner.balance,
        code_hash: account_info.inner.code_hash,
        nonce: account_info.inner.nonce,
        storage_hash: account_info.inner.storage_root,
        account_proof,
        storage_proof,
    })
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

    fn resolver_at<'a>(&'a self, height: BlockHeight) -> impl MoveResolver + TableResolver + 'a {
        self.resolver(height).unwrap()
    }
}

/// This is a [`MoveResolver`] that accesses blockchain state via [`EthTrie`].
///
/// If you pass it an [`EthTrie`] initialized at state root corresponding to certain older block
/// height, it will read from the blockchain state version at that block.
pub struct EthTrieResolver<D: DB> {
    tree: EthTrie<D>,
}

impl<D: DB> EthTrieResolver<D> {
    pub fn new(tree: EthTrie<D>) -> Self {
        Self { tree }
    }
}

impl<D: DB> ModuleResolver for EthTrieResolver<D> {
    fn get_module_metadata(&self, _module_id: &ModuleId) -> Vec<Metadata> {
        Vec::new()
    }

    fn get_module(&self, id: &ModuleId) -> Result<Option<Bytes>, PartialVMError> {
        let state_key = StateKey::module(id.address(), id.name());
        let key_hash = TreeKey::StateKey(state_key).key_hash();
        let value = self.tree.get(key_hash.0.as_slice()).map_err(trie_err)?;

        Ok(deserialize_state_value(value))
    }
}

impl<D: DB> ResourceResolver for EthTrieResolver<D> {
    fn get_resource_bytes_with_metadata_and_layout(
        &self,
        address: &AccountAddress,
        struct_tag: &StructTag,
        _metadata: &[Metadata],
        _layout: Option<&MoveTypeLayout>,
    ) -> Result<(Option<Bytes>, usize), PartialVMError> {
        let tree_key = if let Some(address) = evm_key_address(struct_tag) {
            TreeKey::Evm(address)
        } else {
            let state_key = StateKey::resource(address, struct_tag)
                .inspect_err(|e| print!("{e:?}"))
                .map_err(|_| PartialVMError::new(StatusCode::DATA_FORMAT_ERROR))?;
            TreeKey::StateKey(state_key)
        };
        let key_hash = tree_key.key_hash();
        let value = self.tree.get(key_hash.0.as_slice()).map_err(trie_err)?;
        let value = if is_evm_storage_or_account_key(struct_tag) {
            // In the case of EVM there is no additional serialization
            value.map(Into::into)
        } else {
            deserialize_state_value(value)
        };
        let len = value.as_ref().map(|v| v.len()).unwrap_or_default();

        Ok((value, len))
    }
}

impl<D: DB> TableResolver for EthTrieResolver<D> {
    fn resolve_table_entry_bytes_with_layout(
        &self,
        _handle: &TableHandle,
        _key: &[u8],
        _maybe_layout: Option<&MoveTypeLayout>,
    ) -> Result<Option<Bytes>, PartialVMError> {
        unimplemented!()
    }
}

fn deserialize_state_value(bytes: Option<Vec<u8>>) -> Option<Bytes> {
    let value: StateValue = bcs::from_bytes(&bytes?).expect("Bytes must be serialized StateValue");
    let (_, inner) = value.unpack();
    Some(inner)
}

fn trie_err(e: TrieError) -> PartialVMError {
    PartialVMError::new(StatusCode::STORAGE_ERROR).with_message(format!("{e:?}"))
}

#[cfg(any(feature = "test-doubles", test))]
pub mod test_doubles {
    use {
        super::*,
        crate::state::{Balance, BlockHeight, Nonce, ProofResponse},
        move_core_types::account_address::AccountAddress,
        moved_shared::primitives::U256,
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

        fn resolver_at<'a>(&'a self, _: BlockHeight) -> impl MoveResolver + TableResolver + 'a {
            EthTrieResolver::new(EthTrie::new(Arc::new(eth_trie::MemoryDB::new(true))))
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::hex,
        move_core_types::effects::ChangeSet,
        move_table_extension::TableChangeSet,
        move_vm_runtime::{
            AsUnsyncCodeStorage,
            module_traversal::{TraversalContext, TraversalStorage},
        },
        move_vm_types::gas::UnmeteredGasMeter,
        moved_evm_ext::state::InMemoryStorageTrieRepository,
        moved_execution::{check_nonce, create_vm_session, mint_eth, session_id::SessionId},
        moved_genesis::{CreateMoveVm, MovedVm, config::GenesisConfig},
        moved_shared::primitives::B256,
        moved_state::{InMemoryState, ResolverBasedModuleBytesStorage, State},
    };

    impl ReadStateRoot for Vec<B256> {
        fn root_by_height(&self, height: BlockHeight) -> Option<B256> {
            self.get(height as usize).cloned()
        }

        fn height(&self) -> BlockHeight {
            self.len() as u64 - 1
        }
    }

    struct StateSpy(InMemoryState, ChangeSet);

    impl State for StateSpy {
        type Err = <InMemoryState as State>::Err;

        fn apply(&mut self, changes: ChangeSet) -> Result<(), Self::Err> {
            self.1.squash(changes.clone()).unwrap();
            self.0.apply(changes)
        }

        fn apply_with_tables(
            &mut self,
            changes: ChangeSet,
            table_changes: TableChangeSet,
        ) -> Result<(), Self::Err> {
            self.1.squash(changes.clone()).unwrap();
            self.0.apply_with_tables(changes, table_changes)
        }

        fn db(&self) -> Arc<impl DB> {
            self.0.db()
        }

        fn resolver(&self) -> &(impl MoveResolver + TableResolver) {
            self.0.resolver()
        }

        fn state_root(&self) -> B256 {
            self.0.state_root()
        }
    }

    fn mint_one_eth(state: &mut impl State, addr: AccountAddress) -> ChangeSet {
        let evm_storage = InMemoryStorageTrieRepository::new();
        let moved_vm = MovedVm::new(&Default::default());
        let module_bytes_storage = ResolverBasedModuleBytesStorage::new(state.resolver());
        let code_storage = module_bytes_storage.as_unsync_code_storage(&moved_vm);
        let vm = moved_vm.create_move_vm().unwrap();
        let mut session = create_vm_session(
            &vm,
            state.resolver(),
            SessionId::default(),
            &evm_storage,
            &(),
            &(),
        );
        let traversal_storage = TraversalStorage::new();
        let mut traversal_context = TraversalContext::new(&traversal_storage);
        let mut gas_meter = UnmeteredGasMeter;

        mint_eth(
            &addr,
            U256::from(1u64),
            &mut session,
            &mut traversal_context,
            &mut gas_meter,
            &code_storage,
        )
        .unwrap();

        let changes = session.finish(&code_storage).unwrap();

        state.apply(changes.clone()).unwrap();

        changes
    }

    #[test]
    fn test_query_fetches_latest_balance() {
        let mut evm_storage = InMemoryStorageTrieRepository::new();
        let state = InMemoryState::default();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        let (changes, tables, evm_storage_changes) = moved_genesis_image::load();
        moved_genesis::apply(
            changes,
            tables,
            evm_storage_changes,
            &genesis_config,
            &mut state,
            &mut evm_storage,
        );

        let mut state = state.0;
        let addr = AccountAddress::TWO;

        let mut storage = vec![genesis_config.initial_state_root];

        mint_one_eth(&mut state, addr);
        storage.push(state.state_root());

        let query = InMemoryStateQueries::new(storage, state.db());

        let actual_balance = query
            .balance_at(&evm_storage, addr, 1)
            .expect("Block height should exist");
        let expected_balance = U256::from(1u64);

        assert_eq!(actual_balance, expected_balance);
    }

    #[test]
    fn test_query_fetches_older_balance() {
        let mut evm_storage = InMemoryStorageTrieRepository::new();
        let state = InMemoryState::default();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        let (changes, tables, evm_storage_changes) = moved_genesis_image::load();
        moved_genesis::apply(
            changes,
            tables,
            evm_storage_changes,
            &genesis_config,
            &mut state,
            &mut evm_storage,
        );

        let mut state = state.0;

        let addr = AccountAddress::TWO;

        let mut storage = vec![genesis_config.initial_state_root];

        mint_one_eth(&mut state, addr);
        storage.push(state.state_root());
        mint_one_eth(&mut state, addr);
        mint_one_eth(&mut state, addr);
        storage.push(state.state_root());

        let query = InMemoryStateQueries::new(storage, state.db());

        let actual_balance = query
            .balance_at(&evm_storage, addr, 1)
            .expect("Block height should exist");
        let expected_balance = U256::from(1u64);

        assert_eq!(actual_balance, expected_balance);
    }

    #[test]
    fn test_query_fetches_latest_and_previous_balance() {
        let mut evm_storage = InMemoryStorageTrieRepository::new();
        let state = InMemoryState::default();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        let (changes, tables, evm_storage_changes) = moved_genesis_image::load();
        moved_genesis::apply(
            changes,
            tables,
            evm_storage_changes,
            &genesis_config,
            &mut state,
            &mut evm_storage,
        );

        let mut state = state.0;

        let addr = AccountAddress::TWO;

        let mut storage = vec![genesis_config.initial_state_root];

        mint_one_eth(&mut state, addr);
        storage.push(state.state_root());
        mint_one_eth(&mut state, addr);
        mint_one_eth(&mut state, addr);
        storage.push(state.state_root());

        let query = InMemoryStateQueries::new(storage, state.db());

        let actual_balance = query
            .balance_at(&evm_storage, addr, 1)
            .expect("Block height should exist");
        let expected_balance = U256::from(1u64);

        assert_eq!(actual_balance, expected_balance);

        let actual_balance = query
            .balance_at(&evm_storage, addr, 2)
            .expect("Block height should exist");
        let expected_balance = U256::from(3u64);

        assert_eq!(actual_balance, expected_balance);
    }

    #[test]
    fn test_query_fetches_zero_balance_for_non_existent_account() {
        let mut evm_storage = InMemoryStorageTrieRepository::new();
        let state = InMemoryState::default();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        let (changes, tables, evm_storage_changes) = moved_genesis_image::load();
        moved_genesis::apply(
            changes,
            tables,
            evm_storage_changes,
            &genesis_config,
            &mut state,
            &mut evm_storage,
        );

        let state = state.0;

        let addr = AccountAddress::new(hex!(
            "123456136717634683648732647632874638726487fefefefefeefefefefefff"
        ));

        let storage = vec![genesis_config.initial_state_root];

        let query = InMemoryStateQueries::new(storage, state.db());

        let actual_balance = query
            .balance_at(&evm_storage, addr, 0)
            .expect("Block height should exist");
        let expected_balance = U256::ZERO;

        assert_eq!(actual_balance, expected_balance);
    }

    fn inc_one_nonce(old_nonce: u64, state: &mut impl State, addr: AccountAddress) -> ChangeSet {
        let evm_storage = InMemoryStorageTrieRepository::new();
        let moved_vm = MovedVm::new(&Default::default());
        let module_bytes_storage = ResolverBasedModuleBytesStorage::new(state.resolver());
        let code_storage = module_bytes_storage.as_unsync_code_storage(&moved_vm);
        let vm = moved_vm.create_move_vm().unwrap();
        let mut session = create_vm_session(
            &vm,
            state.resolver(),
            SessionId::default(),
            &evm_storage,
            &(),
            &(),
        );
        let traversal_storage = TraversalStorage::new();
        let mut traversal_context = TraversalContext::new(&traversal_storage);
        let mut gas_meter = UnmeteredGasMeter;

        check_nonce(
            old_nonce,
            &addr,
            &mut session,
            &mut traversal_context,
            &mut gas_meter,
            &code_storage,
        )
        .unwrap();

        let changes = session.finish(&code_storage).unwrap();

        state.apply(changes.clone()).unwrap();

        changes
    }

    #[test]
    fn test_query_fetches_latest_nonce() {
        let mut evm_storage = InMemoryStorageTrieRepository::new();
        let state = InMemoryState::default();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        let (changes, tables, evm_storage_changes) = moved_genesis_image::load();
        moved_genesis::apply(
            changes,
            tables,
            evm_storage_changes,
            &genesis_config,
            &mut state,
            &mut evm_storage,
        );

        let mut state = state.0;
        let addr = AccountAddress::TWO;

        let mut storage = vec![genesis_config.initial_state_root];

        inc_one_nonce(0, &mut state, addr);
        storage.push(state.state_root());

        let query = InMemoryStateQueries::new(storage, state.db());

        let actual_nonce = query
            .nonce_at(&evm_storage, addr, 1)
            .expect("Block height should exist");
        let expected_nonce = 1u64;

        assert_eq!(actual_nonce, expected_nonce);
    }

    #[test]
    fn test_query_fetches_older_nonce() {
        let mut evm_storage = InMemoryStorageTrieRepository::new();
        let state = InMemoryState::default();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        let (changes, tables, evm_storage_changes) = moved_genesis_image::load();
        moved_genesis::apply(
            changes,
            tables,
            evm_storage_changes,
            &genesis_config,
            &mut state,
            &mut evm_storage,
        );

        let mut state = state.0;

        let addr = AccountAddress::TWO;

        let mut storage = vec![genesis_config.initial_state_root];

        inc_one_nonce(0, &mut state, addr);
        storage.push(state.state_root());
        inc_one_nonce(1, &mut state, addr);
        inc_one_nonce(2, &mut state, addr);
        storage.push(state.state_root());

        let query = InMemoryStateQueries::new(storage, state.db());

        let actual_nonce = query
            .nonce_at(&evm_storage, addr, 1)
            .expect("Block height should exist");
        let expected_nonce = 1u64;

        assert_eq!(actual_nonce, expected_nonce);
    }

    #[test]
    fn test_query_fetches_latest_and_previous_nonce() {
        let mut evm_storage = InMemoryStorageTrieRepository::new();
        let state = InMemoryState::default();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        let (changes, tables, evm_storage_changes) = moved_genesis_image::load();
        moved_genesis::apply(
            changes,
            tables,
            evm_storage_changes,
            &genesis_config,
            &mut state,
            &mut evm_storage,
        );

        let mut state = state.0;

        let addr = AccountAddress::TWO;

        let mut storage = vec![genesis_config.initial_state_root];

        inc_one_nonce(0, &mut state, addr);
        storage.push(state.state_root());
        inc_one_nonce(1, &mut state, addr);
        inc_one_nonce(2, &mut state, addr);
        storage.push(state.state_root());

        let query = InMemoryStateQueries::new(storage, state.db());

        let actual_nonce = query
            .nonce_at(&evm_storage, addr, 1)
            .expect("Block height should exist");
        let expected_nonce = 1u64;

        assert_eq!(actual_nonce, expected_nonce);

        let actual_nonce = query
            .nonce_at(&evm_storage, addr, 2)
            .expect("Block height should exist");
        let expected_nonce = 3u64;

        assert_eq!(actual_nonce, expected_nonce);
    }

    #[test]
    fn test_query_fetches_zero_nonce_for_non_existent_account() {
        let mut evm_storage = InMemoryStorageTrieRepository::new();
        let state = InMemoryState::default();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        let (changes, tables, evm_storage_changes) = moved_genesis_image::load();
        moved_genesis::apply(
            changes,
            tables,
            evm_storage_changes,
            &genesis_config,
            &mut state,
            &mut evm_storage,
        );

        let state = state.0;

        let addr = AccountAddress::new(hex!(
            "123456136717634683648732647632874638726487fefefefefeefefefefefff"
        ));

        let storage = vec![genesis_config.initial_state_root];

        let query = InMemoryStateQueries::new(storage, state.db());

        let actual_nonce = query
            .nonce_at(&evm_storage, addr, 0)
            .expect("Block height should exist");
        let expected_nonce = 0u64;

        assert_eq!(actual_nonce, expected_nonce);
    }
}
