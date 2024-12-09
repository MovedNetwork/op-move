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

pub type Balance = U256;
pub type Nonce = u64;
pub type BlockHeight = u64;

pub trait StateQueries {
    /// The associated storage type for querying the blockchain state.
    type Storage;

    /// Queries the blockchain state version corresponding with block `height` for amount of base
    /// token associated with `account`.
    fn balance_at(
        &self,
        db: &(impl TreeReader<StateKey> + Sync),
        account: AccountAddress,
        height: BlockHeight,
    ) -> Balance;

    /// Queries the blockchain state version corresponding with block `height` for the nonce value
    /// associated with `account`.
    fn nonce_at(
        &self,
        db: &(impl TreeReader<StateKey> + Sync),
        account: AccountAddress,
        height: BlockHeight,
    ) -> Nonce;
}

#[derive(Debug)]
pub struct StateMemory {
    mem: InMemoryVersionedState,
    height_to_version: HashMap<usize, u64>,
}

impl Default for StateMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl StateMemory {
    pub fn from_genesis(changes: ChangeSet) -> Self {
        Self {
            mem: InMemoryVersionedState::from_iter(Self::convert(changes, 0)),
            height_to_version: HashMap::from([(0, 0)]),
        }
    }

    pub fn new() -> Self {
        Self {
            mem: InMemoryVersionedState::new(),
            height_to_version: HashMap::new(),
        }
    }

    pub fn add(&mut self, change: ChangeSet, version: u64) {
        for (k, v) in Self::convert(change, version) {
            self.mem.historic_state.insert(k, v);
        }
    }

    pub fn resolver<'a>(
        &'a self,
        db: &'a (impl TreeReader<StateKey> + Sync),
        height: usize,
    ) -> impl MoveResolver<PartialVMError> + TableResolver + 'a {
        HistoricResolver::new(
            db,
            self.height_to_version
                .get(&height)
                .copied()
                .unwrap_or_default(),
            &self.mem,
        )
    }

    fn convert(
        changes: ChangeSet,
        version: u64,
    ) -> impl Iterator<Item = ((StateKey, u64), Option<StateValue>)> {
        changes
            .into_inner()
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

    pub fn add(&mut self, change: ChangeSet, version: u64) {
        self.storage.add(change, version);
    }
}

impl StateQueries for InMemoryStateQueries {
    type Storage = StateMemory;

    fn balance_at(
        &self,
        db: &(impl TreeReader<StateKey> + Sync),
        account: AccountAddress,
        height: BlockHeight,
    ) -> Balance {
        let resolver = self.storage.resolver(db, height as usize);

        quick_get_eth_balance(&account, &resolver)
    }

    fn nonce_at(
        &self,
        db: &(impl TreeReader<StateKey> + Sync),
        account: AccountAddress,
        height: BlockHeight,
    ) -> Nonce {
        let resolver = self.storage.resolver(db, height as usize);

        quick_get_nonce(&account, &resolver)
    }
}

pub struct HistoricResolver<'a, R, H> {
    db: &'a R,
    version: u64,
    historic_state: &'a H,
}

pub trait VersionedState {
    fn get(&self, key: &(StateKey, u64)) -> Option<Bytes>;
}

#[derive(Debug)]
pub struct InMemoryVersionedState {
    historic_state: HashMap<(StateKey, u64), Option<StateValue>>,
}

impl InMemoryVersionedState {
    pub fn from_iter(iter: impl Iterator<Item = ((StateKey, u64), Option<StateValue>)>) -> Self {
        Self {
            historic_state: HashMap::from_iter(iter),
        }
    }

    pub fn new() -> Self {
        Self {
            historic_state: HashMap::new(),
        }
    }
}

impl VersionedState for InMemoryVersionedState {
    fn get(&self, key: &(StateKey, u64)) -> Option<Bytes> {
        self.historic_state
            .get(key)
            .cloned()
            .flatten()
            .map(|v| v.unpack().1)
    }
}

impl<'a, R, H> HistoricResolver<'a, R, H> {
    pub fn new(db: &'a R, version: u64, historic_state: &'a H) -> Self {
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

        eprintln!("{:#?}", tree.get_root_hash(0));
        eprintln!("{:#?}", tree.get_root_hash(1));
        eprintln!("{:#?}", tree.get_root_hash(2));
        eprintln!("{:#?}", tree.get_root_hash(3));
        eprintln!("{:#?}", tree.get_leaf_count(0));
        eprintln!("{:#?}", tree.get_leaf_count(1));
        eprintln!("{:#?}", tree.get_leaf_count(2));
        eprintln!("{:#?}", tree.get_leaf_count(3));
        eprintln!("{:#?}", tree.get_all_nodes_referenced(0));
        eprintln!("{:#?}", tree.get_all_nodes_referenced(1));
        eprintln!("{:#?}", tree.get_all_nodes_referenced(2));
        eprintln!("{:#?}", tree.get_all_nodes_referenced(3));

        let (maybe_leaf, _) = tree.get_with_proof(state_key.hash(), 2).unwrap();
        let (_, history_key) = maybe_leaf.unwrap();
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
        let (_, history_key) = maybe_leaf.unwrap();
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
            move_execution::{
                create_move_vm, create_vm_session, eth_token::mint_eth, nonces::check_nonce,
            },
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

        let mut tree = InMemoryState::new();
        tree.apply(genesis_changes.clone()).unwrap();
        tree.apply(mint_one_eth(&mut state, addr)).unwrap();

        let mut storage = StateMemory::default();

        storage.add(genesis_changes, 0);
        storage.add(mint_one_eth(&mut state, addr), 1);

        let query = InMemoryStateQueries::new(storage);

        let actual_balance = query.balance_at(tree.db(), addr, 999);
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

        let changes_1 = inc_one_nonce(0, &mut state, addr);
        let changes_2 = inc_one_nonce(1, &mut state, addr);
        let changes_3 = inc_one_nonce(2, &mut state, addr);

        let mut tree = InMemoryState::new();
        tree.apply(genesis_changes.clone()).unwrap();
        tree.apply(changes_1.clone()).unwrap();
        tree.apply(changes_2.clone()).unwrap();
        tree.apply(changes_3.clone()).unwrap();

        let mut storage = StateMemory::default();

        storage.add(genesis_changes, 0);
        storage.add(changes_1, 1);
        storage.add(changes_2, 2);
        storage.add(changes_3, 3);

        let query = InMemoryStateQueries::new(storage);

        let actual_balance = query.balance_at(tree.db(), addr, 1);
        let expected_balance = U256::from(1u64);

        assert_eq!(actual_balance, expected_balance);
    }

    #[test]
    fn test_query_fetches_zero_balance_for_non_existent_account() {
        let state = InMemoryState::new();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        init_and_apply(&genesis_config, &mut state);

        let genesis_changes = state.1;

        let addr = AccountAddress::new(hex!(
            "123456136717634683648732647632874638726487fefefefefeefefefefefff"
        ));

        let mut tree = InMemoryState::new();
        tree.apply(genesis_changes.clone()).unwrap();

        let mut storage = StateMemory::default();

        storage.add(genesis_changes, 0);

        let query = InMemoryStateQueries::new(storage);

        let actual_balance = query.balance_at(tree.db(), addr, 0);
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
        let changes_1 = inc_one_nonce(0, &mut state, addr);

        let mut tree = InMemoryState::new();
        tree.apply(genesis_changes.clone()).unwrap();
        tree.apply(changes_1.clone()).unwrap();

        let mut storage = StateMemory::default();

        storage.add(genesis_changes, 0);
        storage.add(changes_1, 1);

        let query = InMemoryStateQueries::new(storage);

        let actual_nonce = query.nonce_at(tree.db(), addr, 999);
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

        let changes_1 = inc_one_nonce(0, &mut state, addr);
        let changes_2 = inc_one_nonce(1, &mut state, addr);
        let changes_3 = inc_one_nonce(2, &mut state, addr);

        let mut tree = InMemoryState::new();
        tree.apply(genesis_changes.clone()).unwrap();
        tree.apply(changes_1.clone()).unwrap();
        tree.apply(changes_2.clone()).unwrap();
        tree.apply(changes_3.clone()).unwrap();

        let mut storage = StateMemory::default();

        storage.add(genesis_changes, 0);
        storage.add(changes_1, 1);
        storage.add(changes_2, 2);
        storage.add(changes_3, 3);

        let query = InMemoryStateQueries::new(storage);

        let actual_nonce = query.nonce_at(tree.db(), addr, 1);
        let expected_nonce = 1u64;

        assert_eq!(actual_nonce, expected_nonce);
    }

    #[test]
    fn test_query_fetches_zero_nonce_for_non_existent_account() {
        let state = InMemoryState::new();
        let mut state = StateSpy(state, ChangeSet::new());

        let genesis_config = GenesisConfig::default();
        init_and_apply(&genesis_config, &mut state);

        let genesis_changes = state.1;

        let addr = AccountAddress::new(hex!(
            "123456136717634683648732647632874638726487fefefefefeefefefefefff"
        ));

        let mut tree = InMemoryState::new();
        tree.apply(genesis_changes.clone()).unwrap();

        let mut storage = StateMemory::default();

        storage.add(genesis_changes, 0);

        let query = InMemoryStateQueries::new(storage);

        let actual_nonce = query.nonce_at(tree.db(), addr, 0);
        let expected_nonce = 0u64;

        assert_eq!(actual_nonce, expected_nonce);
    }
}
