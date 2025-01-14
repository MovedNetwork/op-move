use {
    crate::{
        move_execution::{
            evm_native::{self, EVM_NATIVE_ADDRESS},
            quick_get_eth_balance, quick_get_nonce,
        },
        primitives::{KeyHashable, ToEthAddress, B256, U256},
        storage::{evm_key_address, IN_MEMORY_EXPECT_MSG},
        types::{
            queries::{ProofResponse, StorageProof},
            state::TreeKey,
            transactions::{L2_HIGHEST_ADDRESS, L2_LOWEST_ADDRESS},
        },
    },
    alloy::primitives::Bytes as AlloyBytes,
    aptos_types::state_store::{state_key::StateKey, state_value::StateValue},
    bytes::Bytes,
    eth_trie::{EthTrie, Trie, TrieError, DB},
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        account_address::AccountAddress,
        language_storage::{ModuleId, StructTag},
        metadata::Metadata,
        resolver::{ModuleResolver, MoveResolver, ResourceResolver},
        value::MoveTypeLayout,
        vm_status::StatusCode,
    },
    move_table_extension::{TableHandle, TableResolver},
    revm::DatabaseRef,
    std::{fmt::Debug, sync::Arc},
};

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
    /// The associated storage type for querying the blockchain state.
    type Storage;

    /// Queries the blockchain state version corresponding with block `height` for the amount of
    /// base token associated with `account`.
    fn balance_at(
        &self,
        db: Arc<impl DB>,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Balance>;

    /// Queries the blockchain state version corresponding with block `height` for the nonce value
    /// associated with `account`.
    fn nonce_at(
        &self,
        db: Arc<impl DB>,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Nonce>;

    fn get_proof(
        &self,
        db: Arc<impl DB>,
        account: AccountAddress,
        storage_slots: &[U256],
        height: BlockHeight,
    ) -> Option<ProofResponse>;
}

#[derive(Debug)]
pub struct StateMemory {
    state_roots: Vec<B256>,
}

impl StateMemory {
    /// Creates state memory with `genesis_changes` on `version` 0 tagged as block `height` 0.
    pub fn from_genesis(genesis_state_root: B256) -> Self {
        Self {
            state_roots: vec![genesis_state_root],
        }
    }

    fn push_state_root(&mut self, root: B256) {
        self.state_roots.push(root);
    }

    fn get_root_by_height(&self, height: BlockHeight) -> Option<B256> {
        self.state_roots.get(height as usize).copied()
    }

    fn resolver<'a>(
        &'a self,
        db: Arc<impl DB + 'a>,
        height: BlockHeight,
    ) -> Option<impl MoveResolver<PartialVMError> + TableResolver + 'a> {
        Some(HistoricResolver::new(db, self.get_root_by_height(height)?))
    }
}

#[derive(Debug)]
pub struct InMemoryStateQueries {
    storage: StateMemory,
}

impl InMemoryStateQueries {
    pub fn new(storage: StateMemory) -> Self {
        Self { storage }
    }

    /// Creates state memory with `genesis_changes` on `version` 0 tagged as block `height` 0.
    pub fn from_genesis(genesis_state_root: B256) -> Self {
        Self::new(StateMemory::from_genesis(genesis_state_root))
    }

    /// Marks current state root with current block height.
    ///
    /// The internal block height number is incremented by this operation.
    pub fn push_state_root(&mut self, root: B256) {
        self.storage.push_state_root(root);
    }
}

impl StateQueries for InMemoryStateQueries {
    type Storage = StateMemory;

    fn balance_at(
        &self,
        db: Arc<impl DB>,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Balance> {
        let resolver = self.storage.resolver(db, height)?;

        Some(quick_get_eth_balance(&account, &resolver))
    }

    fn nonce_at(
        &self,
        db: Arc<impl DB>,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Nonce> {
        let resolver = self.storage.resolver(db, height)?;

        Some(quick_get_nonce(&account, &resolver))
    }

    fn get_proof(
        &self,
        db: Arc<impl DB>,
        account: AccountAddress,
        storage_slots: &[U256],
        height: BlockHeight,
    ) -> Option<ProofResponse> {
        let address = account.to_eth_address();

        // Only L2 contract addresses supported at this time
        if address < L2_LOWEST_ADDRESS || L2_HIGHEST_ADDRESS < address {
            return None;
        }

        // All L2 contract account data is part of the EVM state
        let resolver = self.storage.resolver(db.clone(), height)?;
        let evm_db = evm_native::ResolverBackedDB::new(&resolver);
        let account_info = evm_db.basic_ref(address).ok()??;

        let root = self.storage.get_root_by_height(height)?;
        let mut tree = EthTrie::from(db, root).expect(IN_MEMORY_EXPECT_MSG);
        let account_key = TreeKey::Evm(address);
        let account_proof = tree
            .get_proof(account_key.key_hash().0.as_slice())
            .ok()?
            .into_iter()
            .map(Into::into)
            .collect();

        let mut storage_proof = Vec::new();
        for index in storage_slots {
            let storage_struct =
                evm_native::type_utils::account_storage_struct_tag(&address, index);
            let value = evm_db.storage_ref(address, *index).ok()?;
            // TODO: need to revisit EVM storage proofs.
            let proof = get_proof(&mut tree, &EVM_NATIVE_ADDRESS, &storage_struct)?;
            storage_proof.push(StorageProof {
                key: (*index).into(),
                value,
                proof,
            });
        }

        Some(ProofResponse {
            address,
            balance: account_info.balance,
            code_hash: account_info.code_hash,
            nonce: account_info.nonce,
            storage_hash: root,
            account_proof,
            storage_proof,
        })
    }
}

fn get_proof<R>(
    tree: &mut EthTrie<R>,
    account: &AccountAddress,
    resource: &StructTag,
) -> Option<Vec<AlloyBytes>>
where
    R: DB,
{
    let state_key = StateKey::resource(account, resource).ok()?;
    let key_hash = TreeKey::StateKey(state_key).key_hash();
    let proof = tree.get_proof(key_hash.0.as_slice()).ok()?;
    Some(proof.into_iter().map(Into::into).collect())
}

/// This is a [`MoveResolver`] that reads data generated at `version`.
pub struct HistoricResolver<D> {
    db: Arc<D>,
    root: B256,
}

impl<D> HistoricResolver<D> {
    pub fn new(db: Arc<D>, root: B256) -> Self {
        Self { db, root }
    }
}

impl<D: DB> ModuleResolver for HistoricResolver<D> {
    type Error = PartialVMError;

    fn get_module_metadata(&self, _module_id: &ModuleId) -> Vec<Metadata> {
        Vec::new()
    }

    fn get_module(&self, id: &ModuleId) -> Result<Option<Bytes>, Self::Error> {
        let tree = EthTrie::from(self.db.clone(), self.root).expect(IN_MEMORY_EXPECT_MSG);
        let state_key = StateKey::module(id.address(), id.name());
        let key_hash = TreeKey::StateKey(state_key).key_hash();
        let value = tree.get(key_hash.0.as_slice()).map_err(trie_err)?;

        Ok(deserialize_state_value(value))
    }
}

impl<D: DB> ResourceResolver for HistoricResolver<D> {
    type Error = PartialVMError;

    fn get_resource_bytes_with_metadata_and_layout(
        &self,
        address: &AccountAddress,
        struct_tag: &StructTag,
        _metadata: &[Metadata],
        _layout: Option<&MoveTypeLayout>,
    ) -> Result<(Option<Bytes>, usize), Self::Error> {
        let tree = EthTrie::from(self.db.clone(), self.root).expect(IN_MEMORY_EXPECT_MSG);
        let tree_key = if let Some(address) = evm_key_address(struct_tag) {
            TreeKey::Evm(address)
        } else {
            let state_key = StateKey::resource(address, struct_tag)
                .inspect_err(|e| print!("{e:?}"))
                .map_err(|_| PartialVMError::new(StatusCode::DATA_FORMAT_ERROR))?;
            TreeKey::StateKey(state_key)
        };
        let key_hash = tree_key.key_hash();
        let value = tree.get(key_hash.0.as_slice()).map_err(trie_err)?;
        let value = deserialize_state_value(value);
        let len = value.as_ref().map(|v| v.len()).unwrap_or_default();

        Ok((value, len))
    }
}

impl<D: DB> TableResolver for HistoricResolver<D> {
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            genesis::{config::GenesisConfig, init_and_apply},
            move_execution::{check_nonce, create_move_vm, create_vm_session, mint_eth},
            primitives::B256,
            storage::{InMemoryState, State},
            types::session_id::SessionId,
        },
        alloy::hex,
        move_core_types::effects::ChangeSet,
        move_table_extension::TableChangeSet,
        move_vm_runtime::module_traversal::{TraversalContext, TraversalStorage},
        move_vm_types::gas::UnmeteredGasMeter,
    };

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

        fn resolver(&self) -> &(impl MoveResolver<Self::Err> + TableResolver) {
            self.0.resolver()
        }

        fn state_root(&self) -> B256 {
            self.0.state_root()
        }
    }

    fn mint_one_eth(
        state: &mut impl State<Err = PartialVMError>,
        addr: AccountAddress,
    ) -> ChangeSet {
        let move_vm = create_move_vm().unwrap();
        let mut session = create_vm_session(&move_vm, state.resolver(), SessionId::default());
        let traversal_storage = TraversalStorage::new();
        let mut traversal_context = TraversalContext::new(&traversal_storage);
        let mut gas_meter = UnmeteredGasMeter;

        mint_eth(
            &addr,
            U256::from(1u64),
            &mut session,
            &mut traversal_context,
            &mut gas_meter,
        )
        .unwrap();

        let changes = session.finish().unwrap();

        state.apply(changes.clone()).unwrap();

        changes
    }

    #[test]
    fn test_query_fetches_latest_balance() {
        let state = InMemoryState::new();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        init_and_apply(&genesis_config, &mut state);

        let mut state = state.0;
        let addr = AccountAddress::TWO;

        let mut storage = StateMemory::from_genesis(genesis_config.initial_state_root);

        mint_one_eth(&mut state, addr);
        storage.push_state_root(state.state_root());

        let query = InMemoryStateQueries::new(storage);

        let actual_balance = query
            .balance_at(state.db(), addr, 1)
            .expect("Block height should exist");
        let expected_balance = U256::from(1u64);

        assert_eq!(actual_balance, expected_balance);
    }

    #[test]
    fn test_query_fetches_older_balance() {
        let state = InMemoryState::new();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        init_and_apply(&genesis_config, &mut state);

        let mut state = state.0;

        let addr = AccountAddress::TWO;

        let mut storage = StateMemory::from_genesis(genesis_config.initial_state_root);

        mint_one_eth(&mut state, addr);
        storage.push_state_root(state.state_root());
        mint_one_eth(&mut state, addr);
        mint_one_eth(&mut state, addr);
        storage.push_state_root(state.state_root());

        let query = InMemoryStateQueries::new(storage);

        let actual_balance = query
            .balance_at(state.db(), addr, 1)
            .expect("Block height should exist");
        let expected_balance = U256::from(1u64);

        assert_eq!(actual_balance, expected_balance);
    }

    #[test]
    fn test_query_fetches_latest_and_previous_balance() {
        let state = InMemoryState::new();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        init_and_apply(&genesis_config, &mut state);

        let mut state = state.0;

        let addr = AccountAddress::TWO;

        let mut storage = StateMemory::from_genesis(genesis_config.initial_state_root);

        mint_one_eth(&mut state, addr);
        storage.push_state_root(state.state_root());
        mint_one_eth(&mut state, addr);
        mint_one_eth(&mut state, addr);
        storage.push_state_root(state.state_root());

        let query = InMemoryStateQueries::new(storage);

        let actual_balance = query
            .balance_at(state.db(), addr, 1)
            .expect("Block height should exist");
        let expected_balance = U256::from(1u64);

        assert_eq!(actual_balance, expected_balance);

        let actual_balance = query
            .balance_at(state.db(), addr, 2)
            .expect("Block height should exist");
        let expected_balance = U256::from(3u64);

        assert_eq!(actual_balance, expected_balance);
    }

    #[test]
    fn test_query_fetches_zero_balance_for_non_existent_account() {
        let state = InMemoryState::new();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        init_and_apply(&genesis_config, &mut state);

        let state = state.0;

        let addr = AccountAddress::new(hex!(
            "123456136717634683648732647632874638726487fefefefefeefefefefefff"
        ));

        let storage = StateMemory::from_genesis(genesis_config.initial_state_root);

        let query = InMemoryStateQueries::new(storage);

        let actual_balance = query
            .balance_at(state.db(), addr, 0)
            .expect("Block height should exist");
        let expected_balance = U256::ZERO;

        assert_eq!(actual_balance, expected_balance);
    }

    fn inc_one_nonce(
        old_nonce: u64,
        state: &mut impl State<Err = PartialVMError>,
        addr: AccountAddress,
    ) -> ChangeSet {
        let move_vm = create_move_vm().unwrap();
        let mut session = create_vm_session(&move_vm, state.resolver(), SessionId::default());
        let traversal_storage = TraversalStorage::new();
        let mut traversal_context = TraversalContext::new(&traversal_storage);
        let mut gas_meter = UnmeteredGasMeter;

        check_nonce(
            old_nonce,
            &addr,
            &mut session,
            &mut traversal_context,
            &mut gas_meter,
        )
        .unwrap();

        let changes = session.finish().unwrap();

        state.apply(changes.clone()).unwrap();

        changes
    }

    #[test]
    fn test_query_fetches_latest_nonce() {
        let state = InMemoryState::new();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        init_and_apply(&genesis_config, &mut state);

        let mut state = state.0;
        let addr = AccountAddress::TWO;

        let mut storage = StateMemory::from_genesis(genesis_config.initial_state_root);

        inc_one_nonce(0, &mut state, addr);
        storage.push_state_root(state.state_root());

        let query = InMemoryStateQueries::new(storage);

        let actual_nonce = query
            .nonce_at(state.db(), addr, 1)
            .expect("Block height should exist");
        let expected_nonce = 1u64;

        assert_eq!(actual_nonce, expected_nonce);
    }

    #[test]
    fn test_query_fetches_older_nonce() {
        let state = InMemoryState::new();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        init_and_apply(&genesis_config, &mut state);

        let mut state = state.0;

        let addr = AccountAddress::TWO;

        let mut storage = StateMemory::from_genesis(genesis_config.initial_state_root);

        inc_one_nonce(0, &mut state, addr);
        storage.push_state_root(state.state_root());
        inc_one_nonce(1, &mut state, addr);
        inc_one_nonce(2, &mut state, addr);
        storage.push_state_root(state.state_root());

        let query = InMemoryStateQueries::new(storage);

        let actual_nonce = query
            .nonce_at(state.db(), addr, 1)
            .expect("Block height should exist");
        let expected_nonce = 1u64;

        assert_eq!(actual_nonce, expected_nonce);
    }

    #[test]
    fn test_query_fetches_latest_and_previous_nonce() {
        let state = InMemoryState::new();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        init_and_apply(&genesis_config, &mut state);

        let mut state = state.0;

        let addr = AccountAddress::TWO;

        let mut storage = StateMemory::from_genesis(genesis_config.initial_state_root);

        inc_one_nonce(0, &mut state, addr);
        storage.push_state_root(state.state_root());
        inc_one_nonce(1, &mut state, addr);
        inc_one_nonce(2, &mut state, addr);
        storage.push_state_root(state.state_root());

        let query = InMemoryStateQueries::new(storage);

        let actual_nonce = query
            .nonce_at(state.db(), addr, 1)
            .expect("Block height should exist");
        let expected_nonce = 1u64;

        assert_eq!(actual_nonce, expected_nonce);

        let actual_nonce = query
            .nonce_at(state.db(), addr, 2)
            .expect("Block height should exist");
        let expected_nonce = 3u64;

        assert_eq!(actual_nonce, expected_nonce);
    }

    #[test]
    fn test_query_fetches_zero_nonce_for_non_existent_account() {
        let state = InMemoryState::new();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        init_and_apply(&genesis_config, &mut state);

        let state = state.0;

        let addr = AccountAddress::new(hex!(
            "123456136717634683648732647632874638726487fefefefefeefefefefefff"
        ));

        let storage = StateMemory::from_genesis(genesis_config.initial_state_root);

        let query = InMemoryStateQueries::new(storage);

        let actual_nonce = query
            .nonce_at(state.db(), addr, 0)
            .expect("Block height should exist");
        let expected_nonce = 0u64;

        assert_eq!(actual_nonce, expected_nonce);
    }
}
