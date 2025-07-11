use {
    aptos_table_natives::TableChangeSet,
    move_core_types::effects::ChangeSet,
    std::io::Write,
    umi_evm_ext::state::{InMemoryStorageTrieRepository, StorageTrieRepository},
    umi_genesis::{
        SerdeAllChanges, SerdeChanges, SerdeTableChangeSet, UmiVm, build, config::GenesisConfig,
    },
};

fn main() {
    // We're particularly interested in Aptos / Sui bundle changes, but wouldn't
    // hurt to rerun whenever anything in genesis changes as it's a separate package
    println!("cargo::rerun-if-changed=../genesis/");
    let storage_trie = InMemoryStorageTrieRepository::new();
    let genesis_config = GenesisConfig::default();
    let vm = UmiVm::new(&genesis_config);

    save(&vm, &genesis_config, &storage_trie);
}

// Safety: genesis-image is only used in tests
#[allow(clippy::unwrap_used)]
pub fn save(
    vm: &UmiVm,
    config: &GenesisConfig,
    storage_trie: &impl StorageTrieRepository,
) -> (ChangeSet, TableChangeSet) {
    let path = std::env::var("OUT_DIR").unwrap() + "/genesis.bin";
    let (changes, evm_storage) = build(vm, config, storage_trie);
    let accounts = SerdeChanges::from(changes.accounts);
    let tables = SerdeTableChangeSet::from(changes.tables);
    let all_changes = SerdeAllChanges::new(accounts, tables, evm_storage.into());
    let contents = bcs::to_bytes(&all_changes).unwrap();
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(contents.as_slice()).unwrap();
    file.flush().unwrap();

    (all_changes.changes.into(), all_changes.tables.into())
}
