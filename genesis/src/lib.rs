pub use {
    framework::{CreateMoveVm, FRAMEWORK_ADDRESS, load_aptos_framework_snapshot},
    serde::{
        SerdeAccountChanges, SerdeAllChanges, SerdeChanges, SerdeOp, SerdeTableChange,
        SerdeTableChangeSet, SerdeTableInfo,
    },
    vm::MovedVm,
};

use {
    self::config::GenesisConfig,
    move_core_types::effects::ChangeSet,
    move_table_extension::TableChangeSet,
    moved_evm_ext::state::{StorageTrieRepository, StorageTriesChanges},
    moved_state::State,
};

pub mod config;

// TODO(#328): LoaderV2 migration
#[allow(deprecated)]
mod framework;

mod l2_contracts;
mod serde;
mod vm;

pub fn build(
    vm: &MovedVm,
    config: &GenesisConfig,
    state: &impl State,
    storage_trie: &impl StorageTrieRepository,
) -> (ChangeSet, TableChangeSet, StorageTriesChanges) {
    // Deploy Move/Aptos/Sui frameworks
    let (changes_framework, table_changes) = framework::init_state(vm, state);

    // Deploy OP stack L2 contracts
    let changes_l2 =
        l2_contracts::init_state(config.l2_contract_genesis.clone(), state, storage_trie);

    let mut changes = ChangeSet::new();

    changes
        .squash(changes_framework)
        .expect("Framework changes should not be in conflict");

    changes
        .squash(changes_l2.accounts)
        .expect("L2 contract changes should not be in conflict");

    (changes, table_changes, changes_l2.storage)
}

pub fn apply(
    changes: ChangeSet,
    table_changes: TableChangeSet,
    evm_storage_changes: StorageTriesChanges,
    config: &GenesisConfig,
    state: &mut impl State,
    storage_trie: &mut impl StorageTrieRepository,
) {
    state
        .apply_with_tables(changes, table_changes)
        .expect("Changes should be applicable");
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
    vm: &MovedVm,
    config: &GenesisConfig,
    state: &mut impl State,
    storage_trie: &mut impl StorageTrieRepository,
) {
    let (changes, table_changes, evm_storage) = build(vm, config, state, storage_trie);
    apply(
        changes,
        table_changes,
        evm_storage,
        config,
        state,
        storage_trie,
    );
}
