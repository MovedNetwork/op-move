use {
    crate::{block, evm_storage_trie, payload, receipt, state, transaction, trie},
    heed::{types::LazyDecode, BytesDecode, BytesEncode, RoTxn, RwTxn},
};

pub const DATABASES: [&str; 11] = [
    block::DB,
    block::HEIGHT_DB,
    state::DB,
    state::HEIGHT_DB,
    trie::DB,
    trie::ROOT_DB,
    evm_storage_trie::DB,
    evm_storage_trie::ROOT_DB,
    transaction::DB,
    receipt::DB,
    payload::DB,
];

#[derive(Debug)]
pub struct HeedDb<Key, Value>(pub heed::Database<Key, Value>);

impl<Key, Value> HeedDb<Key, Value> {
    pub fn put<'a>(
        &self,
        txn: &mut RwTxn,
        key: &'a Key::EItem,
        value: &'a Value::EItem,
    ) -> heed::Result<()>
    where
        Key: BytesEncode<'a>,
        Value: BytesEncode<'a>,
    {
        self.0.put(txn, key, value)
    }

    pub fn get<'a, 'txn>(
        &self,
        txn: &'txn RoTxn,
        key: &'a Key::EItem,
    ) -> heed::Result<Option<Value::DItem>>
    where
        Key: BytesEncode<'a>,
        Value: BytesDecode<'txn>,
    {
        self.0.get(txn, key)
    }

    pub fn lazily_decode_data(&self) -> HeedDb<Key, LazyDecode<Value>> {
        HeedDb(self.0.lazily_decode_data())
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::collections::HashSet};

    #[test]
    fn test_databases_have_unique_names() {
        let expected_unique_len = DATABASES.len();
        let actual_unique_len = HashSet::from(DATABASES).len();

        assert_eq!(actual_unique_len, expected_unique_len);
    }
}
