use {
    crate::{
        move_execution::{quick_get_eth_balance, quick_get_nonce},
        primitives::U256,
    },
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        account_address::AccountAddress, effects::ChangeSet, resolver::MoveResolver,
    },
    move_table_extension::TableResolver,
    move_vm_test_utils::InMemoryStorage,
    std::ops::Bound,
};

pub type Balance = U256;
pub type Nonce = u64;
pub type BlockHeight = u64;

pub trait StateQueries {
    /// The associated storage type for querying the blockchain state.
    type Storage;

    /// Queries the current blockchain state for amount of base token associated with `account`.
    fn balance(&self, account: AccountAddress) -> Balance;

    /// Queries the blockchain state version corresponding with block `height` for amount of base
    /// token associated with `account`.
    fn balance_at(&self, account: AccountAddress, height: BlockHeight) -> Balance;

    /// Queries the current blockchain state for the nonce value associated with `account`.
    fn nonce(&self, account: AccountAddress) -> Nonce;

    /// Queries the blockchain state version corresponding with block `height` for the nonce value
    /// associated with `account`.
    fn nonce_at(&self, account: AccountAddress, height: BlockHeight) -> Nonce;

    fn add(&mut self, changes: ChangeSet);
}

#[derive(Debug)]
pub struct StateMemory {
    changes: Vec<ChangeSet>,
}

impl Default for StateMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl StateMemory {
    pub fn from_genesis(changes: ChangeSet) -> Self {
        Self {
            changes: vec![changes],
        }
    }

    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
        }
    }

    pub fn add(&mut self, change: ChangeSet) {
        self.changes.push(change);
    }

    pub fn resolver(
        &self,
        upper: Bound<usize>,
    ) -> impl MoveResolver<PartialVMError> + TableResolver {
        let mut state = InMemoryStorage::new();
        let upper = upper.map(|n| n.min(self.changes.len().saturating_sub(1)));

        for change in self.changes[(Bound::Included(0), upper)].iter() {
            state.apply(change.clone()).expect(
                "The changeset should be applicable as it was previously applied in write storage",
            );
        }

        state
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
}

impl StateQueries for InMemoryStateQueries {
    type Storage = StateMemory;

    fn balance(&self, account: AccountAddress) -> Balance {
        let resolver = self.storage.resolver(Bound::Unbounded);

        quick_get_eth_balance(&account, &resolver)
    }

    fn balance_at(&self, account: AccountAddress, height: BlockHeight) -> Balance {
        let resolver = self.storage.resolver(Bound::Included(height as usize));

        quick_get_eth_balance(&account, &resolver)
    }

    fn nonce(&self, account: AccountAddress) -> Nonce {
        let resolver = self.storage.resolver(Bound::Unbounded);

        quick_get_nonce(&account, &resolver)
    }

    fn nonce_at(&self, account: AccountAddress, height: BlockHeight) -> Nonce {
        let resolver = self.storage.resolver(Bound::Included(height as usize));

        quick_get_nonce(&account, &resolver)
    }

    fn add(&mut self, change: ChangeSet) {
        self.storage.add(change);
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

        let mut storage = StateMemory::default();

        storage.add(genesis_changes);
        storage.add(mint_one_eth(&mut state, addr));

        let query = InMemoryStateQueries::new(storage);

        let actual_balance = query.balance(addr);
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
        let mut storage = StateMemory::default();

        storage.add(genesis_changes);
        storage.add(mint_one_eth(&mut state, addr));
        storage.add(mint_one_eth(&mut state, addr));
        storage.add(mint_one_eth(&mut state, addr));

        let query = InMemoryStateQueries::new(storage);

        let actual_balance = query.balance_at(addr, 1);
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

        let mut storage = StateMemory::default();

        storage.add(genesis_changes);

        let query = InMemoryStateQueries::new(storage);

        let actual_balance = query.balance(addr);
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

        let mut storage = StateMemory::default();

        storage.add(genesis_changes);
        storage.add(inc_one_nonce(0, &mut state, addr));

        let query = InMemoryStateQueries::new(storage);

        let actual_nonce = query.nonce(addr);
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
        let mut storage = StateMemory::default();

        storage.add(genesis_changes);
        storage.add(inc_one_nonce(0, &mut state, addr));
        storage.add(inc_one_nonce(1, &mut state, addr));
        storage.add(inc_one_nonce(2, &mut state, addr));

        let query = InMemoryStateQueries::new(storage);

        let actual_nonce = query.nonce_at(addr, 1);
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

        let mut storage = StateMemory::default();

        storage.add(genesis_changes);

        let query = InMemoryStateQueries::new(storage);

        let actual_nonce = query.nonce(addr);
        let expected_nonce = 0u64;

        assert_eq!(actual_nonce, expected_nonce);
    }
}
