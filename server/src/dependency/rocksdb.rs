use {
    crate::dependency::shared::*,
    std::sync::Arc,
    umi_app::{Application, CommandActor, HybridBlockHashCache},
    umi_blockchain::state::EthTrieStateQueries,
    umi_genesis::config::GenesisConfig,
    umi_shared::error::Error,
    umi_state::{EthTrieState, State},
    umi_storage_rocksdb::{block, RocksEthTrieDb},
};

pub type Dependency = RocksDbDependencies;
pub type ReaderDependency = RocksDbReaderDependencies;

pub fn dependencies(args: umi_server_args::Database) -> Dependency {
    let db = Arc::new(create_db(args));
    RocksDbDependencies {
        db: db.clone(),
        in_progress_payloads: Default::default(),
        block_hash_lookup: HybridBlockHashCache::new(db, block::RocksDbBlockQueries::new()),
    }
}

pub struct RocksDbDependencies {
    db: Arc<umi_storage_rocksdb::RocksDb>,
    in_progress_payloads: umi_blockchain::payload::InProgressPayloads,
    block_hash_lookup:
        HybridBlockHashCache<Arc<umi_storage_rocksdb::RocksDb>, block::RocksDbBlockQueries>,
}

pub struct RocksDbReaderDependencies {
    db: Arc<umi_storage_rocksdb::RocksDb>,
    in_progress_payloads: umi_blockchain::payload::InProgressPayloads,
    block_hash_lookup:
        HybridBlockHashCache<Arc<umi_storage_rocksdb::RocksDb>, block::RocksDbBlockQueries>,
}

impl RocksDbDependencies {
    /// Creates a set of dependencies appropriate for usage in reader.
    pub fn reader(&self) -> ReaderDependency {
        RocksDbReaderDependencies {
            db: self.db.clone(),
            in_progress_payloads: self.in_progress_payloads.clone(),
            block_hash_lookup: self.block_hash_lookup.clone(),
        }
    }
}

impl<'db> umi_app::Dependencies<'db> for RocksDbDependencies {
    type BlockHashLookup = HybridBlockHashCache<Self::SharedStorageReader, Self::BlockQueries>;
    type BlockHashWriter = HybridBlockHashCache<Self::SharedStorageReader, Self::BlockQueries>;
    type BlockQueries = umi_storage_rocksdb::block::RocksDbBlockQueries;
    type BlockRepository = umi_storage_rocksdb::block::RocksDbBlockRepository<'db>;
    type OnPayload = umi_app::OnPayload<Application<'db, Self>>;
    type OnTx = umi_app::OnTx<Application<'db, Self>>;
    type OnTxBatch = umi_app::OnTxBatch<Application<'db, Self>>;
    type PayloadQueries = umi_storage_rocksdb::payload::RocksDbPayloadQueries;
    type ReceiptQueries = umi_storage_rocksdb::receipt::RocksDbReceiptQueries<'db>;
    type ReceiptRepository = umi_storage_rocksdb::receipt::RocksDbReceiptRepository<'db>;
    type ReceiptStorage = Arc<umi_storage_rocksdb::RocksDb>;
    type SharedStorage = Arc<umi_storage_rocksdb::RocksDb>;
    type ReceiptStorageReader = Arc<umi_storage_rocksdb::RocksDb>;
    type SharedStorageReader = Arc<umi_storage_rocksdb::RocksDb>;
    type State = EthTrieState<RocksEthTrieDb>;
    type StateQueries =
        EthTrieStateQueries<umi_storage_rocksdb::RocksDbStateRootIndex, RocksEthTrieDb>;
    type StorageTrieRepository = umi_storage_rocksdb::evm::RocksDbStorageTrieRepository;
    type TransactionQueries = umi_storage_rocksdb::transaction::RocksDbTransactionQueries<'db>;
    type TransactionRepository =
        umi_storage_rocksdb::transaction::RocksDbTransactionRepository<'db>;

    fn block_hash_lookup(&self) -> Self::BlockHashLookup {
        self.block_hash_lookup.clone()
    }

    fn block_hash_writer(&self) -> Self::BlockHashWriter {
        self.block_hash_lookup.clone()
    }

    fn block_queries() -> Self::BlockQueries {
        umi_storage_rocksdb::block::RocksDbBlockQueries::new()
    }

    fn block_repository() -> Self::BlockRepository {
        umi_storage_rocksdb::block::RocksDbBlockRepository::new()
    }

    fn on_payload() -> &'db Self::OnPayload {
        &|state, id, hash| {
            state.payload_queries.add_block_hash(id, hash).map_err(|e| {
                tracing::error!("on_payload callback failed: {e:?}");
                Error::DatabaseState
            })
        }
    }

    fn on_tx() -> &'db Self::OnTx {
        CommandActor::on_tx_noop()
    }

    fn on_tx_batch() -> &'db Self::OnTxBatch {
        &|state| {
            state
                .state_queries
                .push_state_root(state.state.state_root())
                .map_err(|e| {
                    tracing::error!("on_tx_batch callback failed: {e:?}");
                    Error::DatabaseState
                })
        }
    }

    fn payload_queries(&self) -> Self::PayloadQueries {
        umi_storage_rocksdb::payload::RocksDbPayloadQueries::new(
            self.db.clone(),
            self.in_progress_payloads.clone(),
        )
    }

    fn receipt_queries() -> Self::ReceiptQueries {
        umi_storage_rocksdb::receipt::RocksDbReceiptQueries::new()
    }

    fn receipt_repository() -> Self::ReceiptRepository {
        umi_storage_rocksdb::receipt::RocksDbReceiptRepository::new()
    }

    fn receipt_memory(&mut self) -> Self::ReceiptStorage {
        self.db.clone()
    }

    fn shared_storage(&mut self) -> Self::SharedStorage {
        self.db.clone()
    }

    fn receipt_memory_reader(&self) -> Self::ReceiptStorageReader {
        self.db.clone()
    }

    fn shared_storage_reader(&self) -> Self::SharedStorageReader {
        self.db.clone()
    }

    fn state(&self) -> Self::State {
        fallible::retry(|| EthTrieState::try_new(Arc::new(RocksEthTrieDb::new(self.db.clone()))))
    }

    fn state_queries(&self, genesis_config: &GenesisConfig) -> Self::StateQueries {
        EthTrieStateQueries::new(
            umi_storage_rocksdb::RocksDbStateRootIndex::new(self.db.clone()),
            Arc::new(RocksEthTrieDb::new(self.db.clone())),
            genesis_config.initial_state_root,
        )
    }

    fn storage_trie_repository(&self) -> Self::StorageTrieRepository {
        umi_storage_rocksdb::evm::RocksDbStorageTrieRepository::new(self.db.clone())
    }

    fn transaction_queries() -> Self::TransactionQueries {
        umi_storage_rocksdb::transaction::RocksDbTransactionQueries::new()
    }

    fn transaction_repository() -> Self::TransactionRepository {
        umi_storage_rocksdb::transaction::RocksDbTransactionRepository::new()
    }

    impl_shared!();
}

impl<'db> umi_app::Dependencies<'db> for RocksDbReaderDependencies {
    type BlockHashLookup = HybridBlockHashCache<Self::SharedStorageReader, Self::BlockQueries>;
    type BlockHashWriter = HybridBlockHashCache<Self::SharedStorageReader, Self::BlockQueries>;
    type BlockQueries = umi_storage_rocksdb::block::RocksDbBlockQueries;
    type BlockRepository = umi_storage_rocksdb::block::RocksDbBlockRepository<'db>;
    type OnPayload = umi_app::OnPayload<Application<'db, Self>>;
    type OnTx = umi_app::OnTx<Application<'db, Self>>;
    type OnTxBatch = umi_app::OnTxBatch<Application<'db, Self>>;
    type PayloadQueries = umi_storage_rocksdb::payload::RocksDbPayloadQueries;
    type ReceiptQueries = umi_storage_rocksdb::receipt::RocksDbReceiptQueries<'db>;
    type ReceiptRepository = umi_storage_rocksdb::receipt::RocksDbReceiptRepository<'db>;
    type ReceiptStorage = Arc<umi_storage_rocksdb::RocksDb>;
    type SharedStorage = Arc<umi_storage_rocksdb::RocksDb>;
    type ReceiptStorageReader = Arc<umi_storage_rocksdb::RocksDb>;
    type SharedStorageReader = Arc<umi_storage_rocksdb::RocksDb>;
    type State = EthTrieState<RocksEthTrieDb>;
    type StateQueries =
        EthTrieStateQueries<umi_storage_rocksdb::RocksDbStateRootIndex, RocksEthTrieDb>;
    type StorageTrieRepository = umi_storage_rocksdb::evm::RocksDbStorageTrieRepository;
    type TransactionQueries = umi_storage_rocksdb::transaction::RocksDbTransactionQueries<'db>;
    type TransactionRepository =
        umi_storage_rocksdb::transaction::RocksDbTransactionRepository<'db>;

    fn block_hash_lookup(&self) -> Self::BlockHashLookup {
        self.block_hash_lookup.clone()
    }

    fn block_hash_writer(&self) -> Self::BlockHashWriter {
        self.block_hash_lookup.clone()
    }

    fn block_queries() -> Self::BlockQueries {
        umi_storage_rocksdb::block::RocksDbBlockQueries::new()
    }

    fn block_repository() -> Self::BlockRepository {
        umi_storage_rocksdb::block::RocksDbBlockRepository::new()
    }

    fn on_payload() -> &'db Self::OnPayload {
        &|state, id, hash| {
            state.payload_queries.add_block_hash(id, hash).map_err(|e| {
                tracing::error!("on_payload callback failed: {e:?}");
                Error::DatabaseState
            })
        }
    }

    fn on_tx() -> &'db Self::OnTx {
        CommandActor::on_tx_noop()
    }

    fn on_tx_batch() -> &'db Self::OnTxBatch {
        &|state| {
            state
                .state_queries
                .push_state_root(state.state.state_root())
                .map_err(|e| {
                    tracing::error!("on_tx_batch callback failed: {e:?}");
                    Error::DatabaseState
                })
        }
    }

    fn payload_queries(&self) -> Self::PayloadQueries {
        umi_storage_rocksdb::payload::RocksDbPayloadQueries::new(
            self.db.clone(),
            self.in_progress_payloads.clone(),
        )
    }

    fn receipt_queries() -> Self::ReceiptQueries {
        umi_storage_rocksdb::receipt::RocksDbReceiptQueries::new()
    }

    fn receipt_repository() -> Self::ReceiptRepository {
        umi_storage_rocksdb::receipt::RocksDbReceiptRepository::new()
    }

    fn receipt_memory(&mut self) -> Self::ReceiptStorage {
        self.db.clone()
    }

    fn shared_storage(&mut self) -> Self::SharedStorage {
        self.db.clone()
    }

    fn receipt_memory_reader(&self) -> Self::ReceiptStorageReader {
        self.db.clone()
    }

    fn shared_storage_reader(&self) -> Self::SharedStorageReader {
        self.db.clone()
    }

    fn state(&self) -> Self::State {
        fallible::retry(|| EthTrieState::try_new(Arc::new(RocksEthTrieDb::new(self.db.clone()))))
    }

    fn state_queries(&self, genesis_config: &GenesisConfig) -> Self::StateQueries {
        EthTrieStateQueries::new(
            umi_storage_rocksdb::RocksDbStateRootIndex::new(self.db.clone()),
            Arc::new(RocksEthTrieDb::new(self.db.clone())),
            genesis_config.initial_state_root,
        )
    }

    fn storage_trie_repository(&self) -> Self::StorageTrieRepository {
        umi_storage_rocksdb::evm::RocksDbStorageTrieRepository::new(self.db.clone())
    }

    fn transaction_queries() -> Self::TransactionQueries {
        umi_storage_rocksdb::transaction::RocksDbTransactionQueries::new()
    }

    fn transaction_repository() -> Self::TransactionRepository {
        umi_storage_rocksdb::transaction::RocksDbTransactionRepository::new()
    }

    impl_shared!();
}

fn create_db(args: umi_server_args::Database) -> umi_storage_rocksdb::RocksDb {
    if args.purge {
        let _ = std::fs::remove_dir_all(&args.dir);
    }

    let mut options = umi_storage_rocksdb::rocksdb::Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);

    umi_storage_rocksdb::RocksDb::open_cf(&options, &args.dir, umi_storage_rocksdb::COLUMN_FAMILIES)
        .expect("Database should open in db dir")
}
