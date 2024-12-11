use {
    crate::{
        move_execution::{quick_get_eth_balance, quick_get_nonce},
        primitives::U256,
    },
    aptos_crypto::hash::CryptoHash,
    aptos_jellyfish_merkle::{JellyfishMerkleTree, TreeReader},
    aptos_types::state_store::{state_key::StateKey, state_value::StateValue},
    bytes::Bytes,
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        account_address::AccountAddress,
        effects::ChangeSet,
        language_storage::{ModuleId, StructTag},
        metadata::Metadata,
        resolver::{ModuleResolver, MoveResolver, ResourceResolver},
        value::MoveTypeLayout,
    },
    move_table_extension::{TableHandle, TableResolver},
    std::{collections::HashMap, fmt::Debug},
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
        db: &(impl TreeReader<StateKey> + Sync),
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Balance>;

    /// Queries the blockchain state version corresponding with block `height` for the nonce value
    /// associated with `account`.
    fn nonce_at(
        &self,
        db: &(impl TreeReader<StateKey> + Sync),
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Nonce>;
}

#[derive(Debug)]
pub struct StateMemory {
    mem: InMemoryVersionedState,
    tags: Vec<Version>,
    version: Version,
}

impl StateMemory {
    /// Creates state memory with `genesis_changes` on `version` 0 tagged as block `height` 0.
    pub fn from_genesis(genesis_changes: ChangeSet) -> Self {
        Self {
            mem: InMemoryVersionedState::from_iter(genesis_changes.into_versioned_kv(0)),
            tags: vec![0],
            version: 0,
        }
    }

    fn add(&mut self, changes: ChangeSet) {
        let version = self.next_version();
        for (k, v) in changes.into_versioned_kv(version) {
            self.mem.historic_state.insert(k, v);
        }
    }

    fn tag(&mut self) {
        self.tags.push(self.version);
    }

    fn resolver<'a>(
        &'a self,
        db: &'a (impl TreeReader<StateKey> + Sync),
        height: BlockHeight,
    ) -> Option<impl MoveResolver<PartialVMError> + TableResolver + 'a> {
        Some(HistoricResolver::new(
            db,
            self.tags.get(height as usize).copied()?,
            &self.mem,
        ))
    }

    fn next_version(&mut self) -> Version {
        self.version += 1;
        self.version
    }
}

/// The `IntoVersionedKv` trait is defined by a single operation [`Self::into_versioned_kv`]. See
/// its documentation for details.
trait IntoVersionedKv {
    /// Converts a set of changes into an iterator of key-value pairs where:
    ///
    /// * *Key* is a pair of [`StateKey`] and [`Version`]. It is a fully qualified name of a module
    ///   or a resource paired with a version that corresponds to a transaction where this change
    ///   was made.
    /// * *Value* is [`StateValue`] wrapped in an option. It contains no value in case of deletion.
    ///   In case of addition or modification, it contains the raw contents of the module or
    ///   resource being changed.
    ///
    /// The term "set of changes" assumes some representation of a list of account addresses
    /// associated with a list of operations on its modules and resources.
    ///
    /// The term "operation" is a ternary value of either addition, modification or deletion.
    fn into_versioned_kv(
        self,
        version: Version,
    ) -> impl Iterator<Item = ((StateKey, Version), Option<StateValue>)>;
}

impl IntoVersionedKv for ChangeSet {
    fn into_versioned_kv(
        self,
        version: Version,
    ) -> impl Iterator<Item = ((StateKey, Version), Option<StateValue>)> {
        self.into_inner()
            .into_iter()
            .map(|(address, changes)| (address, changes.into_inner()))
            .flat_map(move |(address, (modules, resources))| {
                modules
                    .into_iter()
                    .map(move |(k, v)| {
                        let value = v.ok().map(StateValue::new_legacy);
                        let key = StateKey::module(&address, k.as_ident_str());

                        ((key, version), value)
                    })
                    .chain(resources.into_iter().map(move |(k, v)| {
                        let value = v.ok().map(StateValue::new_legacy);
                        let key = StateKey::resource(&address, &k).unwrap();

                        ((key, version), value)
                    }))
            })
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
    pub fn from_genesis(genesis_changes: ChangeSet) -> Self {
        Self::new(StateMemory::from_genesis(genesis_changes))
    }

    /// Adds `change` into state memory.
    ///
    /// The internal version number is incremented by this operation.
    pub fn add(&mut self, change: ChangeSet) {
        self.storage.add(change);
    }

    /// Marks current version with current block height.
    ///
    /// The internal block height number is incremented by this operation.
    pub fn tag(&mut self) {
        self.storage.tag();
    }
}

impl StateQueries for InMemoryStateQueries {
    type Storage = StateMemory;

    fn balance_at(
        &self,
        db: &(impl TreeReader<StateKey> + Sync),
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Balance> {
        let resolver = self.storage.resolver(db, height)?;

        Some(quick_get_eth_balance(&account, &resolver))
    }

    fn nonce_at(
        &self,
        db: &(impl TreeReader<StateKey> + Sync),
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Nonce> {
        let resolver = self.storage.resolver(db, height)?;

        Some(quick_get_nonce(&account, &resolver))
    }
}

/// This is a [`MoveResolver`] that reads data generated at `version`.
pub struct HistoricResolver<'a, R, H> {
    db: &'a R,
    version: u64,
    historic_state: &'a H,
}

/// Capable of fetching state value based on its key and version at which the change was registered.
///
/// Defined by a single operation [`Self::get`].
pub trait VersionedState {
    /// Fetches state value in case of addition or modification, or None in case of deletion,
    /// corresponding to state key at given version.
    fn get(&self, key: &(StateKey, Version)) -> Option<Bytes>;
}

/// A [`VersionedState`] that keeps all records locally in an owned unshared memory.
#[derive(Debug)]
pub struct InMemoryVersionedState {
    historic_state: HashMap<(StateKey, Version), Option<StateValue>>,
}

impl InMemoryVersionedState {
    pub fn from_iter(
        iter: impl Iterator<Item = ((StateKey, Version), Option<StateValue>)>,
    ) -> Self {
        Self {
            historic_state: HashMap::from_iter(iter),
        }
    }
}

impl VersionedState for InMemoryVersionedState {
    fn get(&self, key: &(StateKey, Version)) -> Option<Bytes> {
        self.historic_state
            .get(key)
            .cloned()
            .flatten()
            .map(|v| v.unpack().1)
    }
}

impl<'a, R, H> HistoricResolver<'a, R, H> {
    pub fn new(db: &'a R, version: Version, historic_state: &'a H) -> Self {
        Self {
            db,
            version,
            historic_state,
        }
    }
}

impl<'a, R: TreeReader<StateKey> + Sync, H: VersionedState> ModuleResolver
    for HistoricResolver<'a, R, H>
{
    type Error = PartialVMError;

    fn get_module_metadata(&self, _module_id: &ModuleId) -> Vec<Metadata> {
        Vec::new()
    }

    fn get_module(&self, id: &ModuleId) -> Result<Option<Bytes>, Self::Error> {
        let tree = JellyfishMerkleTree::new(self.db);
        let state_key = StateKey::module(id.address(), id.name());
        let (maybe_leaf, _) = tree.get_with_proof(state_key.hash(), self.version).unwrap();
        let Some((_, history_key)) = maybe_leaf else {
            return Ok(None);
        };
        let value = self.historic_state.get(&history_key);

        Ok(value)
    }
}

impl<'a, R: TreeReader<StateKey> + Sync, H: VersionedState> ResourceResolver
    for HistoricResolver<'a, R, H>
{
    type Error = PartialVMError;

    fn get_resource_bytes_with_metadata_and_layout(
        &self,
        address: &AccountAddress,
        struct_tag: &StructTag,
        _metadata: &[Metadata],
        _layout: Option<&MoveTypeLayout>,
    ) -> Result<(Option<Bytes>, usize), Self::Error> {
        let tree = JellyfishMerkleTree::new(self.db);
        let state_key = StateKey::resource(address, struct_tag).unwrap();
        let (maybe_leaf, _) = tree.get_with_proof(state_key.hash(), self.version).unwrap();
        let Some((_, history_key)) = maybe_leaf else {
            return Ok((None, 0));
        };
        let value = self.historic_state.get(&history_key);
        let len = value.as_ref().map(|v| v.len()).unwrap_or_default();

        Ok((value, len))
    }
}

impl<'a, R: TreeReader<StateKey> + Sync, H: VersionedState> TableResolver
    for HistoricResolver<'a, R, H>
{
    fn resolve_table_entry_bytes_with_layout(
        &self,
        _handle: &TableHandle,
        _key: &[u8],
        _maybe_layout: Option<&MoveTypeLayout>,
    ) -> Result<Option<Bytes>, PartialVMError> {
        unimplemented!()
    }
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

        fn db(&self) -> &(impl TreeReader<StateKey> + Sync) {
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

        let (mut state, genesis_changes) = (state.0, state.1);
        let addr = AccountAddress::TWO;

        let mut storage = StateMemory::from_genesis(genesis_changes);

        storage.add(mint_one_eth(&mut state, addr));
        storage.tag();

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

        let (mut state, genesis_changes) = (state.0, state.1);

        let addr = AccountAddress::TWO;

        let mut storage = StateMemory::from_genesis(genesis_changes);

        storage.add(mint_one_eth(&mut state, addr));
        storage.tag();
        storage.add(mint_one_eth(&mut state, addr));
        storage.add(mint_one_eth(&mut state, addr));
        storage.tag();

        let query = InMemoryStateQueries::new(storage);

        let actual_balance = query
            .balance_at(state.db(), addr, 1)
            .expect("Block height should exist");
        let expected_balance = U256::from(1u64);

        assert_eq!(actual_balance, expected_balance);
    }

    #[test]
    fn test_query_fetches_zero_balance_for_non_existent_account() {
        let state = InMemoryState::new();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        init_and_apply(&genesis_config, &mut state);

        let (state, genesis_changes) = (state.0, state.1);

        let addr = AccountAddress::new(hex!(
            "123456136717634683648732647632874638726487fefefefefeefefefefefff"
        ));

        let storage = StateMemory::from_genesis(genesis_changes);

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

        let (mut state, genesis_changes) = (state.0, state.1);
        let addr = AccountAddress::TWO;

        let mut storage = StateMemory::from_genesis(genesis_changes);

        storage.add(inc_one_nonce(0, &mut state, addr));
        storage.tag();

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

        let (mut state, genesis_changes) = (state.0, state.1);

        let addr = AccountAddress::TWO;

        let mut storage = StateMemory::from_genesis(genesis_changes);

        storage.add(inc_one_nonce(0, &mut state, addr));
        storage.tag();
        storage.add(inc_one_nonce(1, &mut state, addr));
        storage.add(inc_one_nonce(2, &mut state, addr));
        storage.tag();

        let query = InMemoryStateQueries::new(storage);

        let actual_nonce = query
            .nonce_at(state.db(), addr, 1)
            .expect("Block height should exist");
        let expected_nonce = 1u64;

        assert_eq!(actual_nonce, expected_nonce);
    }

    #[test]
    fn test_query_fetches_zero_nonce_for_non_existent_account() {
        let state = InMemoryState::new();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        init_and_apply(&genesis_config, &mut state);

        let (state, genesis_changes) = (state.0, state.1);

        let addr = AccountAddress::new(hex!(
            "123456136717634683648732647632874638726487fefefefefeefefefefefff"
        ));

        let storage = StateMemory::from_genesis(genesis_changes);

        let query = InMemoryStateQueries::new(storage);

        let actual_nonce = query
            .nonce_at(state.db(), addr, 0)
            .expect("Block height should exist");
        let expected_nonce = 0u64;

        assert_eq!(actual_nonce, expected_nonce);
    }
}
