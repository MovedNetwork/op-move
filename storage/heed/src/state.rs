use {
    crate::{
        all::HeedDb,
        generic::{EncodableB256, EncodableU64},
    },
    heed::RoTxn,
    moved_blockchain::state::HeightToStateRootIndex,
    moved_shared::primitives::B256,
};

pub type Key = EncodableU64;
pub type Value = EncodableB256;
pub type Db = heed::Database<Key, Value>;
pub type HeightKey = EncodableU64;
pub type HeightValue = EncodableU64;
pub type HeightDb = heed::Database<HeightKey, HeightValue>;

pub const DB: &str = "state";
pub const HEIGHT_DB: &str = "state_height";
pub const HEIGHT_KEY: u64 = 0;

#[derive(Debug, Clone)]
pub struct HeedStateRootIndex<'db> {
    env: &'db heed::Env,
}

impl<'db> HeedStateRootIndex<'db> {
    pub const fn new(env: &'db heed::Env) -> Self {
        Self { env }
    }
}

impl HeightToStateRootIndex for HeedStateRootIndex<'_> {
    type Err = heed::Error;

    fn height(&self) -> Result<u64, Self::Err> {
        let transaction = self.env.read_txn()?;

        let db = self.env.state_height_database(&transaction)?;

        let height = db.get(&transaction, &HEIGHT_KEY);

        transaction.commit()?;

        Ok(height?.unwrap_or(0))
    }

    fn root_by_height(&self, height: u64) -> Result<Option<B256>, Self::Err> {
        let transaction = self.env.read_txn()?;

        let db = self.env.state_database(&transaction)?;

        let root = db.get(&transaction, &height);

        transaction.commit()?;

        root
    }

    fn push_state_root(&self, state_root: B256) -> Result<(), Self::Err> {
        let height = self.height()? + 1;
        let mut transaction = self.env.write_txn()?;

        let db = self.env.state_database(&transaction)?;

        db.put(&mut transaction, &height, &state_root)?;

        let db = self.env.state_height_database(&transaction)?;

        db.put(&mut transaction, &HEIGHT_KEY, &height)?;

        transaction.commit()
    }
}

pub trait HeedStateExt {
    fn state_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>>;

    fn state_height_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<HeightKey, HeightValue>>;
}

impl HeedStateExt for heed::Env {
    fn state_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>> {
        let db: Db = self
            .open_database(rtxn, Some(DB))?
            .expect("State root database should exist");

        Ok(HeedDb(db))
    }

    fn state_height_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<HeightKey, HeightValue>> {
        let db: HeightDb = self
            .open_database(rtxn, Some(HEIGHT_DB))?
            .expect("State height database should exist");

        Ok(HeedDb(db))
    }
}
