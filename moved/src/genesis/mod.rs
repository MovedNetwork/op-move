use {
    self::config::GenesisConfig, crate::storage::State, move_binary_format::errors::PartialVMError,
    move_core_types::effects::ChangeSet, move_table_extension::TableChangeSet,
};

pub use framework::FRAMEWORK_ADDRESS;

pub mod config;
mod framework;
mod l2_contracts;

pub fn init(
    config: &GenesisConfig,
    state: &impl State<Err = PartialVMError>,
) -> (ChangeSet, TableChangeSet) {
    // Read L2 contract data
    let l2_genesis_file = std::fs::File::open(&config.l2_contract_genesis)
        .expect("L2 contracts genesis file must exist");
    let l2_contract_genesis =
        serde_json::from_reader(l2_genesis_file).expect("L2 genesis file must parse successfully");

    let mut changes = ChangeSet::new();

    // Deploy Move/Aptos/Sui frameworks
    let (changes_framework, table_changes) = framework::init_state(state);

    // Deploy OP stack L2 contracts
    let changes_l2 = l2_contracts::init_state(l2_contract_genesis, state);

    changes
        .squash(changes_framework)
        .expect("Framework changes should not be in conflict");

    changes
        .squash(changes_l2)
        .expect("L2 contract changes should not be in conflict");

    (changes, table_changes)
}

pub fn apply(
    changes: ChangeSet,
    table_changes: TableChangeSet,
    config: &GenesisConfig,
    state: &mut impl State<Err = PartialVMError>,
) {
    state
        .apply_with_tables(changes, table_changes)
        .expect("Changes should be applicable");

    // Validate final state
    let actual_state_root = state.state_root();
    let expected_state_root = config.initial_state_root;

    assert_eq!(
        actual_state_root, expected_state_root,
        "Fatal Error: Genesis state root mismatch"
    );
}

pub fn init_and_apply(config: &GenesisConfig, state: &mut impl State<Err = PartialVMError>) {
    let (changes, table_changes) = init(config, state);
    apply(changes, table_changes, config, state);
}
