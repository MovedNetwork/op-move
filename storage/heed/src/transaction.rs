use {
    crate::{
        all::HeedDb,
        generic::{EncodableB256, SerdeJson},
    },
    heed::RoTxn,
    moved_blockchain::transaction::{
        ExtendedTransaction, TransactionQueries, TransactionRepository, TransactionResponse,
    },
    moved_shared::primitives::B256,
};

pub type Key = EncodableB256;
pub type Value = EncodableTransaction;
pub type Db = heed::Database<Key, Value>;
pub type EncodableTransaction = SerdeJson<ExtendedTransaction>;

pub const DB: &str = "transaction";

#[derive(Debug)]
pub struct HeedTransactionRepository;

impl TransactionRepository for HeedTransactionRepository {
    type Err = heed::Error;
    type Storage = &'static heed::Env;

    fn extend(
        &mut self,
        env: &mut Self::Storage,
        transactions: impl IntoIterator<Item = ExtendedTransaction>,
    ) -> Result<(), Self::Err> {
        let mut db_transaction = env.write_txn()?;

        let db = env.transaction_database(&db_transaction)?;

        transactions.into_iter().try_for_each(|transaction| {
            db.put(&mut db_transaction, &transaction.hash(), &transaction)
        })?;

        db_transaction.commit()
    }
}

#[derive(Debug)]
pub struct HeedTransactionQueries;

impl TransactionQueries for HeedTransactionQueries {
    type Err = heed::Error;
    type Storage = &'static heed::Env;

    fn by_hash(
        &self,
        env: &Self::Storage,
        hash: B256,
    ) -> Result<Option<TransactionResponse>, Self::Err> {
        let transaction = env.read_txn()?;

        let db = env.transaction_database(&transaction)?;

        let response = db.get(&transaction, &hash)?.map(TransactionResponse::from);

        transaction.commit()?;

        Ok(response)
    }
}

pub trait HeedTransactionExt {
    fn transaction_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>>;
}

impl HeedTransactionExt for heed::Env {
    fn transaction_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>> {
        let db: Db = self
            .open_database(rtxn, Some(DB))?
            .expect("Transaction database should exist");

        Ok(HeedDb(db))
    }
}
