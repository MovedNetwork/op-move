use {
    move_core_types::effects::ChangeSet,
    move_table_extension::TableChangeSet,
    moved_evm_ext::state::{InMemoryStorageTrieRepository, StorageTrieRepository},
    moved_genesis::{
        MovedVm, SerdeAllChanges, SerdeChanges, SerdeTableChangeSet, build, config::GenesisConfig,
    },
    moved_state::{InMemoryState, State},
    std::io::Write,
};

fn main() {
    // We're particularly interested in Aptos / Sui bundle changes, but wouldn't
    // hurt to rerun whenever anything in genesis changes as it's a separate package
    println!("cargo::rerun-if-changed=../genesis/");
    let state = InMemoryState::default();
    let storage_trie = InMemoryStorageTrieRepository::new();
    let genesis_config = GenesisConfig::default();
    let vm = MovedVm::default();

    save(&vm, &genesis_config, &state, &storage_trie);
}

pub fn save(
    vm: &MovedVm,
    config: &GenesisConfig,
    state: &impl State,
    storage_trie: &impl StorageTrieRepository,
) -> (ChangeSet, TableChangeSet) {
    let path = std::env::var("OUT_DIR").unwrap() + "/genesis.bin";
    let (changes, tables, evm_storage) = build(vm, config, state, storage_trie);
    let changes = SerdeChanges::from(changes);
    let tables = SerdeTableChangeSet::from(tables);
    let all_changes = SerdeAllChanges::new(changes, tables, evm_storage.into());
    let contents = bcs::to_bytes(&all_changes).unwrap();
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(contents.as_slice()).unwrap();
    file.flush().unwrap();

    (all_changes.changes.into(), all_changes.tables.into())
}
