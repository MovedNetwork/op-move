use {
    super::*,
    alloy::hex,
    move_core_types::{account_address::AccountAddress, effects::ChangeSet},
    move_table_extension::TableResolver,
    move_vm_runtime::{
        AsUnsyncCodeStorage,
        module_traversal::{TraversalContext, TraversalStorage},
    },
    move_vm_types::{gas::UnmeteredGasMeter, resolver::MoveResolver},
    std::{convert::Infallible, sync::Arc},
    umi_evm_ext::state::InMemoryStorageTrieRepository,
    umi_execution::{check_nonce, create_vm_session, mint_eth, session_id::SessionId},
    umi_genesis::{CreateMoveVm, UmiVm, config::GenesisConfig},
    umi_shared::primitives::{B256, U256},
    umi_state::{Changes, InMemoryState, InMemoryTrieDb, ResolverBasedModuleBytesStorage, State},
};

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

struct StateSpy(InMemoryState, ChangeSet);

impl State for StateSpy {
    type Err = <InMemoryState as State>::Err;

    fn apply(&mut self, changes: Changes) -> Result<(), Self::Err> {
        self.1.squash(changes.accounts.clone()).unwrap();
        self.0.apply(changes)
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
    let umi_vm = UmiVm::new(&Default::default());
    let module_bytes_storage = ResolverBasedModuleBytesStorage::new(state.resolver());
    let code_storage = module_bytes_storage.as_unsync_code_storage(&umi_vm);
    let vm = umi_vm.create_move_vm().unwrap();
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

    state.apply(changes.clone().into()).unwrap();

    changes
}

#[test]
fn test_query_fetches_latest_balance() {
    let mut evm_storage = InMemoryStorageTrieRepository::new();
    let trie_db = Arc::new(InMemoryTrieDb::empty());
    let state = InMemoryState::empty(trie_db.clone());
    let mut state = StateSpy(state, ChangeSet::new());

    let genesis_config = GenesisConfig::default();
    let (changes, evm_storage_changes) = umi_genesis_image::load();
    umi_genesis::apply(
        changes,
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

    let query = InMemoryStateQueries::new(storage, trie_db, genesis_config.initial_state_root);

    let actual_balance = query
        .balance_at(&evm_storage, addr, 1)
        .expect("Block height should exist");
    let expected_balance = U256::from(1u64);

    assert_eq!(actual_balance, expected_balance);
}

#[test]
fn test_query_fetches_older_balance() {
    let mut evm_storage = InMemoryStorageTrieRepository::new();
    let trie_db = Arc::new(InMemoryTrieDb::empty());
    let state = InMemoryState::empty(trie_db.clone());
    let mut state = StateSpy(state, ChangeSet::new());

    let genesis_config = GenesisConfig::default();
    let (changes, evm_storage_changes) = umi_genesis_image::load();
    umi_genesis::apply(
        changes,
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

    let query = InMemoryStateQueries::new(storage, trie_db, genesis_config.initial_state_root);

    let actual_balance = query
        .balance_at(&evm_storage, addr, 1)
        .expect("Block height should exist");
    let expected_balance = U256::from(1u64);

    assert_eq!(actual_balance, expected_balance);
}

#[test]
fn test_query_fetches_latest_and_previous_balance() {
    let mut evm_storage = InMemoryStorageTrieRepository::new();
    let trie_db = Arc::new(InMemoryTrieDb::empty());
    let state = InMemoryState::empty(trie_db.clone());
    let mut state = StateSpy(state, ChangeSet::new());

    let genesis_config = GenesisConfig::default();
    let (changes, evm_storage_changes) = umi_genesis_image::load();
    umi_genesis::apply(
        changes,
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

    let query = InMemoryStateQueries::new(storage, trie_db, genesis_config.initial_state_root);

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
    let trie_db = Arc::new(InMemoryTrieDb::empty());
    let state = InMemoryState::empty(trie_db.clone());
    let mut state = StateSpy(state, ChangeSet::new());

    let genesis_config = GenesisConfig::default();
    let (changes, evm_storage_changes) = umi_genesis_image::load();
    umi_genesis::apply(
        changes,
        evm_storage_changes,
        &genesis_config,
        &mut state,
        &mut evm_storage,
    );

    let addr = AccountAddress::new(hex!(
        "123456136717634683648732647632874638726487fefefefefeefefefefefff"
    ));

    let storage = vec![genesis_config.initial_state_root];

    let query = InMemoryStateQueries::new(storage, trie_db, genesis_config.initial_state_root);

    let actual_balance = query
        .balance_at(&evm_storage, addr, 0)
        .expect("Block height should exist");
    let expected_balance = U256::ZERO;

    assert_eq!(actual_balance, expected_balance);
}

fn inc_one_nonce(old_nonce: u64, state: &mut impl State, addr: AccountAddress) -> ChangeSet {
    let evm_storage = InMemoryStorageTrieRepository::new();
    let umi_vm = UmiVm::new(&Default::default());
    let module_bytes_storage = ResolverBasedModuleBytesStorage::new(state.resolver());
    let code_storage = module_bytes_storage.as_unsync_code_storage(&umi_vm);
    let vm = umi_vm.create_move_vm().unwrap();
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

    state.apply(changes.clone().into()).unwrap();

    changes
}

#[test]
fn test_query_fetches_latest_nonce() {
    let mut evm_storage = InMemoryStorageTrieRepository::new();
    let trie_db = Arc::new(InMemoryTrieDb::empty());
    let state = InMemoryState::empty(trie_db.clone());
    let mut state = StateSpy(state, ChangeSet::new());

    let genesis_config = GenesisConfig::default();
    let (changes, evm_storage_changes) = umi_genesis_image::load();
    umi_genesis::apply(
        changes,
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

    let query = InMemoryStateQueries::new(storage, trie_db, genesis_config.initial_state_root);

    let actual_nonce = query
        .nonce_at(&evm_storage, addr, 1)
        .expect("Block height should exist");
    let expected_nonce = 1u64;

    assert_eq!(actual_nonce, expected_nonce);
}

#[test]
fn test_query_fetches_older_nonce() {
    let mut evm_storage = InMemoryStorageTrieRepository::new();
    let trie_db = Arc::new(InMemoryTrieDb::empty());
    let state = InMemoryState::empty(trie_db.clone());
    let mut state = StateSpy(state, ChangeSet::new());

    let genesis_config = GenesisConfig::default();
    let (changes, evm_storage_changes) = umi_genesis_image::load();
    umi_genesis::apply(
        changes,
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

    let query = InMemoryStateQueries::new(storage, trie_db, genesis_config.initial_state_root);

    let actual_nonce = query
        .nonce_at(&evm_storage, addr, 1)
        .expect("Block height should exist");
    let expected_nonce = 1u64;

    assert_eq!(actual_nonce, expected_nonce);
}

#[test]
fn test_query_fetches_latest_and_previous_nonce() {
    let mut evm_storage = InMemoryStorageTrieRepository::new();
    let trie_db = Arc::new(InMemoryTrieDb::empty());
    let state = InMemoryState::empty(trie_db.clone());
    let mut state = StateSpy(state, ChangeSet::new());

    let genesis_config = GenesisConfig::default();
    let (changes, evm_storage_changes) = umi_genesis_image::load();
    umi_genesis::apply(
        changes,
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

    let query = InMemoryStateQueries::new(storage, trie_db, genesis_config.initial_state_root);

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
    let trie_db = Arc::new(InMemoryTrieDb::empty());
    let state = InMemoryState::empty(trie_db.clone());
    let mut state = StateSpy(state, ChangeSet::new());

    let genesis_config = GenesisConfig::default();
    let (changes, evm_storage_changes) = umi_genesis_image::load();
    umi_genesis::apply(
        changes,
        evm_storage_changes,
        &genesis_config,
        &mut state,
        &mut evm_storage,
    );

    let addr = AccountAddress::new(hex!(
        "123456136717634683648732647632874638726487fefefefefeefefefefefff"
    ));

    let storage = vec![genesis_config.initial_state_root];

    let query = InMemoryStateQueries::new(storage, trie_db, genesis_config.initial_state_root);

    let actual_nonce = query
        .nonce_at(&evm_storage, addr, 0)
        .expect("Block height should exist");
    let expected_nonce = 0u64;

    assert_eq!(actual_nonce, expected_nonce);
}
