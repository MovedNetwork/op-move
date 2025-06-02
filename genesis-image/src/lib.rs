use {umi_evm_ext::state::StorageTriesChanges, umi_genesis::SerdeAllChanges, umi_state::Changes};

pub fn load() -> (Changes, StorageTriesChanges) {
    let contents = include_bytes!(concat!(env!("OUT_DIR"), "/genesis.bin"));
    let contents: SerdeAllChanges = bcs::from_bytes(contents).expect("File should be bcs encoded");

    (
        Changes::new(contents.changes.into(), contents.tables.into()),
        contents.evm_storage.into(),
    )
}
