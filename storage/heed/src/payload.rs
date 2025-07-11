use {
    crate::{
        all::HeedDb,
        block::HeedBlockExt,
        generic::{EncodableB256, EncodableU64},
        transaction::HeedTransactionExt,
    },
    heed::RoTxn,
    umi_blockchain::payload::{
        InProgressPayloads, MaybePayloadResponse, PayloadId, PayloadQueries, PayloadResponse,
    },
    umi_shared::primitives::{B256, ToU64},
};

pub type Key = EncodableU64;
pub type Value = EncodableB256;
pub type Db = heed::Database<Key, Value>;

pub const DB: &str = "payload";

#[derive(Debug, Clone)]
pub struct HeedPayloadQueries {
    env: heed::Env,
    in_progress: InProgressPayloads,
}

impl HeedPayloadQueries {
    pub const fn new(env: heed::Env, in_progress: InProgressPayloads) -> Self {
        Self { env, in_progress }
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
    type Storage = heed::Env;

    fn by_hash(
        &self,
        env: &Self::Storage,
        hash: B256,
    ) -> Result<Option<PayloadResponse>, Self::Err> {
        let transaction = env.read_txn()?;

        let db = env.block_database(&transaction)?;

        let response = db.get(&transaction, &hash).and_then(|v| {
            v.map(|block| {
                let db = env.transaction_database(&transaction)?;

                let transactions = block
                    .transaction_hashes()
                    .filter_map(|hash| db.get(&transaction, &hash).transpose())
                    .map(|v| v.map(|v| v.inner))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(PayloadResponse::from_block_with_transactions(
                    block,
                    transactions,
                ))
            })
            .transpose()
        });

        transaction.commit()?;

        response
    }

    fn by_id(&self, env: &Self::Storage, id: PayloadId) -> Result<MaybePayloadResponse, Self::Err> {
        if let Some(delayed) = self.in_progress.get_delayed(&id) {
            return Ok(MaybePayloadResponse::Delayed(delayed));
        }

        let transaction = env.read_txn()?;

        let db = env.payload_database(&transaction)?;

        let Some(hash) = db.get(&transaction, &id.to_u64())? else {
            return Ok(MaybePayloadResponse::Unknown);
        };
        transaction.commit()?;
        let response = self
            .by_hash(env, hash)?
            .map_or(MaybePayloadResponse::Unknown, MaybePayloadResponse::Some);
        Ok(response)
    }

    fn get_in_progress(&self) -> InProgressPayloads {
        self.in_progress.clone()
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
