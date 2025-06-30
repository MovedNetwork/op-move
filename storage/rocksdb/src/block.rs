use {
    crate::{
        generic::{FromKey, FromValue, ToKey, ToValue},
        transaction,
    },
    rocksdb::{AsColumnFamilyRef, DB as RocksDb, IteratorMode, WriteBatchWithTransaction},
    std::{marker::PhantomData, sync::Arc},
    umi_blockchain::{
        block::{BlockQueries, BlockRepository, BlockResponse, ExtendedBlock},
        transaction::ExtendedTransaction,
    },
    umi_shared::primitives::B256,
};

pub const BLOCK_COLUMN_FAMILY: &str = "block";
pub const HEIGHT_COLUMN_FAMILY: &str = "height";

#[derive(Debug)]
pub struct RocksDbBlockRepository<'db>(PhantomData<&'db ()>);

impl Default for RocksDbBlockRepository<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl RocksDbBlockRepository<'_> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl BlockRepository for RocksDbBlockRepository<'_> {
    type Err = rocksdb::Error;
    type Storage = Arc<RocksDb>;

    fn add(&mut self, db: &mut Self::Storage, block: ExtendedBlock) -> Result<(), Self::Err> {
        let mut batch = WriteBatchWithTransaction::<false>::default();

        batch.put_cf(&block_cf(db), block.hash, block.to_value());
        batch.put_cf(
            &height_cf(db),
            block.block.header.number.to_key(),
            block.hash,
        );

        db.write(batch)
    }

    fn by_hash(&self, db: &Self::Storage, hash: B256) -> Result<Option<ExtendedBlock>, Self::Err> {
        Ok(db
            .get_pinned_cf(&block_cf(db), hash)?
            .map(|bytes| ExtendedBlock::from_value(bytes.as_ref())))
    }

    fn latest(&self, db: &Self::Storage) -> Result<Option<ExtendedBlock>, Self::Err> {
        Ok(db
            .iterator_cf(&height_cf(db), IteratorMode::End)
            .next()
            .transpose()?
            .map(|(_, hash)| self.by_hash(db, B256::new(hash.as_ref().try_into().unwrap())))
            .transpose()?
            .flatten())
    }
}

#[derive(Debug, Clone)]
pub struct RocksDbBlockQueries;

impl Default for RocksDbBlockQueries {
    fn default() -> Self {
        Self::new()
    }
}

impl RocksDbBlockQueries {
    pub const fn new() -> Self {
        Self
    }
}

impl BlockQueries for RocksDbBlockQueries {
    type Err = rocksdb::Error;
    type Storage = Arc<RocksDb>;

    fn by_hash(
        &self,
        db: &Self::Storage,
        hash: B256,
        include_transactions: bool,
    ) -> Result<Option<BlockResponse>, Self::Err> {
        let block = db
            .get_pinned_cf(&block_cf(db), hash)?
            .map(|v| ExtendedBlock::from_value(v.as_ref()));

        Ok(Some(match block {
            Some(block) if include_transactions => {
                let cf = transaction::cf(db);
                let keys = block.transaction_hashes().collect::<Vec<B256>>();

                let transactions = db
                    .batched_multi_get_cf(&cf, keys.iter(), false)
                    .into_iter()
                    .filter_map(|v| {
                        v.map(|v| v.map(|v| ExtendedTransaction::from_value(v.as_ref())))
                            .transpose()
                    })
                    .collect::<Result<_, _>>()?;

                BlockResponse::from_block_with_transactions(block, transactions)
            }
            Some(block) => BlockResponse::from_block_with_transaction_hashes(block),
            None => return Ok(None),
        }))
    }

    fn by_height(
        &self,
        db: &Self::Storage,
        height: u64,
        include_transactions: bool,
    ) -> Result<Option<BlockResponse>, Self::Err> {
        db.get_pinned_cf(&height_cf(db), height.to_key())?
            .map(|hash| B256::new(hash.as_ref().try_into().unwrap()))
            .map(|hash| self.by_hash(db, hash, include_transactions))
            .unwrap_or(Ok(None))
    }

    fn latest(&self, db: &Self::Storage) -> Result<Option<u64>, Self::Err> {
        Ok(db
            .iterator_cf(&height_cf(db), IteratorMode::End)
            .next()
            .transpose()?
            .map(|(height, _)| u64::from_key(height.as_ref())))
    }
}

pub(crate) fn block_cf(db: &RocksDb) -> impl AsColumnFamilyRef + use<'_> {
    db.cf_handle(BLOCK_COLUMN_FAMILY)
        .expect("Column family should exist")
}

fn height_cf(db: &RocksDb) -> impl AsColumnFamilyRef + use<'_> {
    db.cf_handle(HEIGHT_COLUMN_FAMILY)
        .expect("Column family should exist")
}
