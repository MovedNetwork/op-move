use {rocksdb::Options, umi_storage_rocksdb::COLUMN_FAMILIES};

pub fn create_db() -> rocksdb::DB {
    let path = concat!(
        concat!(env!("CARGO_TARGET_TMPDIR"), "/"),
        env!("CARGO_CRATE_NAME")
    );

    if std::fs::exists(path).unwrap_or(false) {
        std::fs::remove_dir_all(path)
            .expect("Removing non-empty database directory should succeed");
    }

    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);

    rocksdb::DB::open_cf(&options, path, COLUMN_FAMILIES).expect("Database should open in tmpdir")
}
