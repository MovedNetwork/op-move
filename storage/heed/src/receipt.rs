use {
    crate::{
        all::HeedDb,
        generic::{EncodableB256, SerdeJson},
    },
    heed::RoTxn,
    std::marker::PhantomData,
    umi_blockchain::receipt::{
        ExtendedReceipt, ReceiptQueries, ReceiptRepository, TransactionReceipt,
    },
    umi_shared::primitives::B256,
};

pub type Key = EncodableB256;
pub type Value = EncodableReceipt;
pub type Db = heed::Database<Key, Value>;
pub type EncodableReceipt = SerdeJson<ExtendedReceipt>;

pub const DB: &str = "receipt";

#[derive(Debug)]
pub struct HeedReceiptRepository<'db>(PhantomData<&'db ()>);

impl Default for HeedReceiptRepository<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl HeedReceiptRepository<'_> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl ReceiptRepository for HeedReceiptRepository<'_> {
    type Err = heed::Error;
    type Storage = heed::Env;

    fn contains(&self, env: &Self::Storage, transaction_hash: B256) -> Result<bool, Self::Err> {
        let transaction = env.read_txn()?;

        let db = env.receipt_database(&transaction)?.lazily_decode_data();

        let response = db.get(&transaction, &transaction_hash).map(|v| v.is_some());

        transaction.commit()?;

        response
    }

    fn extend(
        &self,
        env: &mut Self::Storage,
        receipts: impl IntoIterator<Item = ExtendedReceipt>,
    ) -> Result<(), Self::Err> {
        let mut transaction = env.write_txn()?;

        let db = env.receipt_database(&transaction)?;

        receipts.into_iter().try_for_each(|receipt| {
            db.put(&mut transaction, &receipt.transaction_hash, &receipt)
        })?;

        transaction.commit()
    }
}

#[derive(Debug, Clone)]
pub struct HeedReceiptQueries<'db>(PhantomData<&'db ()>);

impl Default for HeedReceiptQueries<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl HeedReceiptQueries<'_> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl ReceiptQueries for HeedReceiptQueries<'_> {
    type Err = heed::Error;
    type Storage = heed::Env;

    fn by_transaction_hash(
        &self,
        env: &Self::Storage,
        transaction_hash: B256,
    ) -> Result<Option<TransactionReceipt>, Self::Err> {
        let transaction = env.read_txn()?;

        let db = env.receipt_database(&transaction)?;

        let response = db.get(&transaction, &transaction_hash);

        transaction.commit()?;

        Ok(response?.map(TransactionReceipt::from))
    }
}

pub trait HeedReceiptExt {
    fn receipt_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>>;
}

impl HeedReceiptExt for heed::Env {
    fn receipt_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>> {
        let db: Db = self
            .open_database(rtxn, Some(DB))?
            .expect("Receipt database should exist");

        Ok(HeedDb(db))
    }
}
