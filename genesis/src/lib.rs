pub use {
    framework::{CreateMoveVm, FRAMEWORK_ADDRESS, load_aptos_framework_snapshot},
    serde::{
        SerdeAccountChanges, SerdeAllChanges, SerdeChanges, SerdeOp, SerdeTableChange,
        SerdeTableChangeSet, SerdeTableInfo,
    },
    vm::UmiVm,
};

use {
    self::config::GenesisConfig,
    move_core_types::effects::ChangeSet,
    umi_evm_ext::state::{StorageTrieRepository, StorageTriesChanges},
    umi_state::{Changes, InMemoryState, State},
};

pub mod config;

mod framework;

mod bridged_tokens;
mod l2_contracts;
mod serde;
mod vm;

pub fn build(
    vm: &UmiVm,
    config: &GenesisConfig,
    storage_trie: &impl StorageTrieRepository,
) -> (Changes, StorageTriesChanges) {
    let mut state = InMemoryState::default();
    // Deploy Move/Aptos/Sui frameworks
    let changes_framework = framework::init_state(vm, &mut state);

    // Deploy OP stack L2 contracts
    let mut changes_l2 =
        l2_contracts::init_state(config.l2_contract_genesis.clone(), &state, storage_trie)
            .expect("L2 contracts must deploy");

    // Deploy additional bridged tokens (if any)
    if !config.token_list.is_empty() {
        changes_l2 = bridged_tokens::deploy_bridged_tokens(changes_l2, config.token_list.clone())
            .expect("Bridged tokens must deploy");
    }

    let mut changes = ChangeSet::new();

    changes
        .squash(changes_framework)
        .expect("Framework changes should not be in conflict");

    changes
        .squash(changes_l2.accounts)
        .expect("L2 contract changes should not be in conflict");

    (Changes::without_tables(changes), changes_l2.storage)
}

pub fn apply(
    changes: Changes,
    evm_storage_changes: StorageTriesChanges,
    config: &GenesisConfig,
    state: &mut impl State,
    storage_trie: &mut impl StorageTrieRepository,
) {
    state.apply(changes).expect("Changes should be applicable");
    storage_trie
        .apply(evm_storage_changes)
        .expect("EVM storage changes should be applicable");

    // Validate final state
    let actual_state_root = state.state_root();
    let expected_state_root = config.initial_state_root;

    assert_eq!(
        actual_state_root, expected_state_root,
        "Fatal Error: Genesis state root mismatch"
    );
}

pub fn build_and_apply(
    vm: &UmiVm,
    config: &GenesisConfig,
    state: &mut impl State,
    storage_trie: &mut impl StorageTrieRepository,
) {
    let (changes, evm_storage) = build(vm, config, storage_trie);
    apply(changes, evm_storage, config, state, storage_trie);
}
