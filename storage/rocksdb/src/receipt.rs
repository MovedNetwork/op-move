use {
    crate::generic::{FromValue, ToValue},
    rocksdb::{AsColumnFamilyRef, DB as RocksDb, WriteBatchWithTransaction},
    std::{marker::PhantomData, sync::Arc},
    umi_blockchain::receipt::{
        ExtendedReceipt, ReceiptQueries, ReceiptRepository, TransactionReceipt,
    },
    umi_shared::primitives::B256,
};

pub const COLUMN_FAMILY: &str = "receipt";

#[derive(Debug)]
pub struct RocksDbReceiptRepository<'db>(PhantomData<&'db ()>);

impl Default for RocksDbReceiptRepository<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl RocksDbReceiptRepository<'_> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl ReceiptRepository for RocksDbReceiptRepository<'_> {
    type Err = rocksdb::Error;
    type Storage = Arc<RocksDb>;

    fn contains(&self, db: &Self::Storage, transaction_hash: B256) -> Result<bool, Self::Err> {
        let cf = cf(db);
        db.get_cf(&cf, transaction_hash)
            .map(|v: Option<Vec<u8>>| v.is_some())
    }

    fn extend(
        &self,
        db: &mut Self::Storage,
        receipts: impl IntoIterator<Item = ExtendedReceipt>,
    ) -> Result<(), Self::Err> {
        let cf = cf(db);

        db.write(receipts.into_iter().fold(
            WriteBatchWithTransaction::<false>::default(),
            |mut batch, receipt| {
                batch.put_cf(&cf, receipt.transaction_hash, receipt.to_value());
                batch
            },
        ))
    }
}

#[derive(Debug, Clone)]
pub struct RocksDbReceiptQueries<'db>(PhantomData<&'db ()>);

impl Default for RocksDbReceiptQueries<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl RocksDbReceiptQueries<'_> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl ReceiptQueries for RocksDbReceiptQueries<'_> {
    type Err = rocksdb::Error;
    type Storage = Arc<RocksDb>;

    fn by_transaction_hash(
        &self,
        db: &Self::Storage,
        transaction_hash: B256,
    ) -> Result<Option<TransactionReceipt>, Self::Err> {
        let cf = cf(db);

        Ok(db
            .get_pinned_cf(&cf, transaction_hash)?
            .map(|v| ExtendedReceipt::from_value(v.as_ref()).into()))
    }
}

fn cf(db: &RocksDb) -> impl AsColumnFamilyRef + use<'_> {
    db.cf_handle(COLUMN_FAMILY)
        .expect("Column family should exist")
}
