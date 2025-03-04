use {
    crate::{
        all::HeedDb,
        generic::{EncodableB256, SerdeJson},
    },
    heed::RoTxn,
    moved_blockchain::receipt::{
        ExtendedReceipt, ReceiptQueries, ReceiptRepository, TransactionReceipt,
    },
    moved_shared::primitives::B256,
};

pub type Key = EncodableB256;
pub type Value = EncodableReceipt;
pub type Db = heed::Database<Key, Value>;
pub type EncodableReceipt = SerdeJson<ExtendedReceipt>;

pub const DB: &str = "receipt";

#[derive(Debug)]
pub struct HeedReceiptRepository;

impl ReceiptRepository for HeedReceiptRepository {
    type Err = heed::Error;
    type Storage = &'static heed::Env;

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

#[derive(Debug)]
pub struct HeedReceiptQueries;

impl ReceiptQueries for HeedReceiptQueries {
    type Err = heed::Error;
    type Storage = &'static heed::Env;

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
