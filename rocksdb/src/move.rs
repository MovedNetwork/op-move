use {
    bytes::Bytes,
    eth_trie::DB,
    move_binary_format::{errors::PartialVMError, CompiledModule},
    move_core_types::{
        account_address::AccountAddress,
        language_storage::{ModuleId, StructTag},
        metadata::Metadata,
        resolver::{resource_size, ModuleResolver, ResourceResolver},
        value::MoveTypeLayout,
    },
    move_table_extension::{TableHandle, TableResolver},
    rocksdb::DB as RocksDb,
};

pub struct RocksEthTrieDb<'db> {
    db: &'db RocksDb,
}

impl<'db> RocksEthTrieDb<'db> {
    pub fn new(db: &'db RocksDb) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &'db RocksDb {
        self.db
    }
}

impl<'db> ModuleResolver for RocksEthTrieDb<'db> {
    type Error = rocksdb::Error;

    fn get_module_metadata(&self, module_id: &ModuleId) -> Vec<Metadata> {
        if let Ok(Some(bytes)) = self.get_module(module_id) {
            if let Ok(module) = CompiledModule::deserialize(&bytes) {
                return module.metadata;
            }
        }

        Vec::new()
    }

    fn get_module(&self, id: &ModuleId) -> Result<Option<Bytes>, Self::Error> {
        let key = id.address().to_canonical_string();
        let db = self.db();

        db.get(key).map(|v| v.map(|v| Bytes::copy_from_slice(&v)))
    }
}

impl<'db> ResourceResolver for RocksEthTrieDb<'db> {
    type Error = rocksdb::Error;

    fn get_resource_bytes_with_metadata_and_layout(
        &self,
        address: &AccountAddress,
        _struct_tag: &StructTag,
        _metadata: &[Metadata],
        _layout: Option<&MoveTypeLayout>,
    ) -> Result<(Option<Bytes>, usize), Self::Error> {
        let key = address.to_canonical_string();
        let db = self.db();

        let bytes = db.get(key)?.map(|v| Bytes::copy_from_slice(&v));
        let size = resource_size(&bytes);

        Ok((bytes, size))
    }
}

impl<'db> TableResolver for RocksEthTrieDb<'db> {
    fn resolve_table_entry_bytes_with_layout(
        &self,
        _handle: &TableHandle,
        _key: &[u8],
        _maybe_layout: Option<&MoveTypeLayout>,
    ) -> Result<Option<Bytes>, PartialVMError> {
        todo!()
    }
}

impl<'db> DB for RocksEthTrieDb<'db> {
    type Error = rocksdb::Error;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        self.db().get(key)
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> Result<(), Self::Error> {
        self.db().put(key, value)
    }

    fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
        self.db().delete(key)
    }

    fn flush(&self) -> Result<(), Self::Error> {
        self.db().flush()
    }
}
