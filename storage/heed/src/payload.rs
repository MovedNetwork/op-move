use {
    crate::{
        all::HeedDb,
        block::HeedBlockExt,
        generic::{EncodableB256, EncodableU64},
    },
    heed::RoTxn,
    moved_blockchain::payload::{PayloadId, PayloadQueries, PayloadResponse},
    moved_shared::primitives::{ToU64, B256},
};

pub type Key = EncodableU64;
pub type Value = EncodableB256;
pub type Db = heed::Database<Key, Value>;

pub const DB: &str = "payload";

#[derive(Debug)]
pub struct HeedPayloadQueries {
    env: &'static heed::Env,
}

impl HeedPayloadQueries {
    pub fn new(env: &'static heed::Env) -> Self {
        Self { env }
    }

    pub fn add_block_hash(&self, id: PayloadId, block_hash: B256) -> Result<(), heed::Error> {
        let mut transaction = self.env.write_txn()?;

        let db = self.env.payload_database(&transaction)?;

        db.put(&mut transaction, &id.to_u64(), &block_hash)?;

        transaction.commit()
    }
}

impl PayloadQueries for HeedPayloadQueries {
    type Err = heed::Error;
    type Storage = &'static heed::Env;

    fn by_hash(
        &self,
        env: &Self::Storage,
        hash: B256,
    ) -> Result<Option<PayloadResponse>, Self::Err> {
        let transaction = env.read_txn()?;

        let db = env.block_database(&transaction)?;

        let response = db.get(&transaction, &hash);

        transaction.commit()?;

        Ok(response?.map(PayloadResponse::from_block))
    }

    fn by_id(
        &self,
        env: &Self::Storage,
        id: PayloadId,
    ) -> Result<Option<PayloadResponse>, Self::Err> {
        let transaction = env.read_txn()?;

        let db = env.payload_database(&transaction)?;

        db.get(&transaction, &id.to_u64())?
            .map(|hash| {
                transaction.commit()?;
                self.by_hash(env, hash)
            })
            .unwrap_or(Ok(None))
    }
}

pub trait HeedPayloadExt {
    fn payload_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>>;
}

impl HeedPayloadExt for heed::Env {
    fn payload_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>> {
        let db: Db = self
            .open_database(rtxn, Some(DB))?
            .expect("Payload database should exist");

        Ok(HeedDb(db))
    }
}
