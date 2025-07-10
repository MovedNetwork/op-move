use {
    crate::{
        RocksDb,
        generic::{FromKey, ToKey},
    },
    rocksdb::{AsColumnFamilyRef, WriteBatchWithTransaction},
    std::sync::Arc,
    umi_blockchain::state::HeightToStateRootIndex,
    umi_shared::primitives::B256,
};

pub const COLUMN_FAMILY: &str = "state";
pub const HEIGHT_COLUMN_FAMILY: &str = "state_height";
pub const HEIGHT_KEY: &str = "state_height";

#[derive(Debug, Clone)]
pub struct RocksDbStateRootIndex {
    db: Arc<RocksDb>,
}

impl RocksDbStateRootIndex {
    pub const fn new(db: Arc<RocksDb>) -> Self {
        Self { db }
    }
}

impl HeightToStateRootIndex for RocksDbStateRootIndex {
    type Err = rocksdb::Error;

    fn root_by_height(&self, height: u64) -> Result<Option<B256>, Self::Err> {
        Ok(self
            .db
            .get_pinned_cf(&self.cf(), height.to_key())?
            .map(|v| B256::from_slice(v.as_ref())))
    }

    fn height(&self) -> Result<u64, Self::Err> {
        Ok(self
            .db
            .get_pinned_cf(&self.height_cf(), HEIGHT_KEY)?
            .map(|v| u64::from_key(v.as_ref()))
            .unwrap_or(0))
    }

    fn push_state_root(&self, state_root: B256) -> Result<(), Self::Err> {
        let height = self.height()? + 1;
        let mut batch = WriteBatchWithTransaction::<false>::default();

        batch.put_cf(&self.cf(), height.to_key(), state_root);
        batch.put_cf(&self.height_cf(), HEIGHT_KEY, height.to_key());

        self.db.write(batch)
    }
}

impl RocksDbStateRootIndex {
    fn height_cf(&self) -> impl AsColumnFamilyRef + use<'_> {
        self.db
            .cf_handle(HEIGHT_COLUMN_FAMILY)
            .expect("Column family should exist")
    }

    fn cf(&self) -> impl AsColumnFamilyRef + use<'_> {
        self.db
            .cf_handle(COLUMN_FAMILY)
            .expect("Column family should exist")
    }
}
