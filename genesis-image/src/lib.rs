use {
    move_core_types::effects::ChangeSet, move_table_extension::TableChangeSet,
    moved_evm_ext::storage::StorageTriesChanges, moved_genesis::SerdeAllChanges,
};

pub fn load() -> (ChangeSet, TableChangeSet, StorageTriesChanges) {
    let contents = include_bytes!(concat!(env!("OUT_DIR"), "/genesis.bin"));
    let contents: SerdeAllChanges = bcs::from_bytes(contents).expect("File should be bcs encoded");

    (
        contents.changes.into(),
        contents.tables.into(),
        contents.evm_storage.into(),
    )
}
