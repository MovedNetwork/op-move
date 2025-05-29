use {
    crate::{
        block::block_cf,
        generic::{FromValue, ToKey},
        transaction,
    },
    rocksdb::{AsColumnFamilyRef, DB as RocksDb},
    umi_blockchain::{
        block::ExtendedBlock,
        payload::{PayloadId, PayloadQueries, PayloadResponse},
        transaction::ExtendedTransaction,
    },
    umi_shared::primitives::B256,
};

pub const COLUMN_FAMILY: &str = "payload";

impl ToKey for PayloadId {
    fn to_key(&self) -> impl AsRef<[u8]> {
        self.to_be_bytes::<8>()
    }
}

#[derive(Debug, Clone)]
pub struct RocksDbPayloadQueries {
    db: &'static RocksDb,
}

impl RocksDbPayloadQueries {
    pub fn new(db: &'static RocksDb) -> Self {
        Self { db }
    }

    pub fn add_block_hash(&self, id: PayloadId, block_hash: B256) -> Result<(), rocksdb::Error> {
        self.db.put_cf(&self.cf(), id.to_key(), block_hash)
    }

    fn cf(&self) -> impl AsColumnFamilyRef + use<'_> {
        cf(self.db)
    }
}

impl PayloadQueries for RocksDbPayloadQueries {
    type Err = rocksdb::Error;
    type Storage = &'static RocksDb;

    fn by_hash(
        &self,
        db: &Self::Storage,
        hash: B256,
    ) -> Result<Option<PayloadResponse>, Self::Err> {
        db.get_pinned_cf(&block_cf(db), hash)?
            .map(|bytes| {
                let block = ExtendedBlock::from_value(bytes.as_ref());
                let transaction_cf = transaction::cf(db);
                let transactions = block
                    .transaction_hashes()
                    .map(|hash| {
                        Ok(db
                            .get_pinned_cf(&transaction_cf, hash)?
                            .map(|v| ExtendedTransaction::from_value(v.as_ref()).inner))
                    })
                    .filter_map(|v| v.transpose())
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(PayloadResponse::from_block_with_transactions(
                    block,
                    transactions,
                ))
            })
            .transpose()
    }

    fn by_id(
        &self,
        db: &Self::Storage,
        id: PayloadId,
    ) -> Result<Option<PayloadResponse>, Self::Err> {
        db.get_pinned_cf(&cf(db), id.to_key())?
            .map(|hash| B256::new(hash.as_ref().try_into().unwrap()))
            .map(|hash| self.by_hash(db, hash))
            .unwrap_or(Ok(None))
    }
}

pub(crate) fn cf(db: &RocksDb) -> impl AsColumnFamilyRef + use<'_> {
    db.cf_handle(COLUMN_FAMILY)
        .expect("Column family should exist")
}
