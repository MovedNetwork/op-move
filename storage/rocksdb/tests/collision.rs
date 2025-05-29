use {
    eth_trie::DB,
    hex_literal::hex,
    umi_evm_ext::state::DbWithRoot,
    umi_shared::primitives::B256,
    umi_storage_rocksdb::{ROOT_KEY, RocksEthTrieDb},
};

mod common;

#[test]
fn test_column_families_do_not_collide() {
    let rocks = common::create_db();
    let db = RocksEthTrieDb::new(&rocks);

    let random_32_bytes = B256::new(hex!(
        "50596cee391a497683672d9396379f56cd8e96476b844557933f48039c483a81"
    ));

    db.put_root(random_32_bytes).unwrap();

    // We assume that `put_root` uses `ROOT_KEY` to store the state root under the hood
    // If the keys are not separated by column families, then this overwrites the state root
    let random_value = hex!("feef");

    db.insert(ROOT_KEY.as_bytes(), random_value.as_slice().to_vec())
        .unwrap();

    let expected_root = random_32_bytes;
    let actual_root = db.root().unwrap().expect("Root should exist in database");

    assert_eq!(actual_root, expected_root);

    let expected_value = random_value;
    let actual_value = db
        .get(ROOT_KEY.as_bytes())
        .unwrap()
        .expect("Key should exist in database");

    assert_eq!(actual_value, expected_value);
}
