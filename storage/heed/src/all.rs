use {
    crate::{block, payload, receipt, state, transaction, trie},
    heed::{types::LazyDecode, BytesDecode, BytesEncode, RoTxn, RwTxn},
};

pub const DATABASES: [&str; 9] = [
    block::DB,
    block::HEIGHT_DB,
    state::DB,
    state::HEIGHT_DB,
    trie::DB,
    trie::ROOT_DB,
    transaction::DB,
    receipt::DB,
    payload::DB,
];

#[derive(Debug)]
pub struct HeedDb<KC, DC>(pub heed::Database<KC, DC>);

impl<KC, DC> HeedDb<KC, DC> {
    pub fn put<'a>(
        &self,
        txn: &mut RwTxn,
        key: &'a KC::EItem,
        data: &'a DC::EItem,
    ) -> heed::Result<()>
    where
        KC: BytesEncode<'a>,
        DC: BytesEncode<'a>,
    {
        self.0.put(txn, key, data)
    }

    pub fn get<'a, 'txn>(
        &self,
        txn: &'txn RoTxn,
        key: &'a KC::EItem,
    ) -> heed::Result<Option<DC::DItem>>
    where
        KC: BytesEncode<'a>,
        DC: BytesDecode<'txn>,
    {
        self.0.get(txn, key)
    }

    pub fn lazily_decode_data(&self) -> HeedDb<KC, LazyDecode<DC>> {
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
