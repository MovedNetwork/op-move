use {
    crate::dependency::shared::*,
    std::{sync::Arc, time::Duration},
    umi_app::{Application, CommandActor},
    umi_blockchain::state::EthTrieStateQueries,
    umi_genesis::config::GenesisConfig,
    umi_state::{EthTrieState, State},
};

pub type Dependency = RocksDbDependencies;

pub fn dependencies() -> crate::dependency::Dependency {
    RocksDbDependencies
}

pub struct RocksDbDependencies;

impl RocksDbDependencies {
    /// Creates a set of dependencies appropriate for usage in reader.
    pub fn reader(&self) -> Self {
        RocksDbDependencies
    }
}

impl umi_app::Dependencies for RocksDbDependencies {
    type BlockQueries = umi_storage_rocksdb::block::RocksDbBlockQueries<'static>;
    type BlockRepository = umi_storage_rocksdb::block::RocksDbBlockRepository<'static>;
    type OnPayload = umi_app::OnPayload<Application<Self>>;
    type OnTx = umi_app::OnTx<Application<Self>>;
    type OnTxBatch = umi_app::OnTxBatch<Application<Self>>;
    type PayloadQueries = umi_storage_rocksdb::payload::RocksDbPayloadQueries<'static>;
    type ReceiptQueries = umi_storage_rocksdb::receipt::RocksDbReceiptQueries<'static>;
    type ReceiptRepository = umi_storage_rocksdb::receipt::RocksDbReceiptRepository<'static>;
    type ReceiptStorage = &'static umi_storage_rocksdb::RocksDb;
    type SharedStorage = &'static umi_storage_rocksdb::RocksDb;
    type ReceiptStorageReader = &'static umi_storage_rocksdb::RocksDb;
    type SharedStorageReader = &'static umi_storage_rocksdb::RocksDb;
    type State = EthTrieState<umi_storage_rocksdb::RocksEthTrieDb<'static>>;
    type StateQueries = EthTrieStateQueries<
        umi_storage_rocksdb::RocksDbStateRootIndex<'static>,
        umi_storage_rocksdb::RocksEthTrieDb<'static>,
    >;
    type StorageTrieRepository = umi_storage_rocksdb::evm::RocksDbStorageTrieRepository;
    type TransactionQueries = umi_storage_rocksdb::transaction::RocksDbTransactionQueries<'static>;
    type TransactionRepository =
        umi_storage_rocksdb::transaction::RocksDbTransactionRepository<'static>;

    fn block_queries() -> Self::BlockQueries {
        umi_storage_rocksdb::block::RocksDbBlockQueries::new()
    }

    fn block_repository() -> Self::BlockRepository {
        umi_storage_rocksdb::block::RocksDbBlockRepository::new()
    }

    fn on_payload() -> &'static Self::OnPayload {
        &|state, id, hash| state.payload_queries.add_block_hash(id, hash).unwrap()
    }

    fn on_tx() -> &'static Self::OnTx {
        CommandActor::on_tx_noop()
    }

    fn on_tx_batch() -> &'static Self::OnTxBatch {
        &|state| {
            state
                .state_queries
                .push_state_root(state.state.state_root())
                .unwrap()
        }
    }

    fn payload_queries() -> Self::PayloadQueries {
        umi_storage_rocksdb::payload::RocksDbPayloadQueries::new(db())
    }

    fn receipt_queries() -> Self::ReceiptQueries {
        umi_storage_rocksdb::receipt::RocksDbReceiptQueries::new()
    }

    fn receipt_repository() -> Self::ReceiptRepository {
        umi_storage_rocksdb::receipt::RocksDbReceiptRepository::new()
    }

    fn receipt_memory(&mut self) -> Self::ReceiptStorage {
        db()
    }

    fn shared_storage(&mut self) -> Self::SharedStorage {
        db()
    }

    fn receipt_memory_reader(&self) -> Self::ReceiptStorageReader {
        db()
    }

    fn shared_storage_reader(&self) -> Self::SharedStorageReader {
        db()
    }

    fn state(&self) -> Self::State {
        let mut tries = 1..60;

        loop {
            match EthTrieState::try_new(TRIE_DB.clone()) {
                Ok(state) => return state,
                Err(error) if tries.next().is_none() => panic!("{error}"),
                Err(error) => {
                    let duration = Duration::from_secs(1);
                    eprintln!("WARN: Failed to create state {error}, retrying in {duration:?}...");
                    std::thread::sleep(duration);
                }
            }
        }
    }

    fn state_queries(&self, genesis_config: &GenesisConfig) -> Self::StateQueries {
        EthTrieStateQueries::new(
            umi_storage_rocksdb::RocksDbStateRootIndex::new(db()),
            TRIE_DB.clone(),
            genesis_config.initial_state_root,
        )
    }

    fn storage_trie_repository(&self) -> Self::StorageTrieRepository {
        umi_storage_rocksdb::evm::RocksDbStorageTrieRepository::new(Database.clone())
    }

    fn transaction_queries() -> Self::TransactionQueries {
        umi_storage_rocksdb::transaction::RocksDbTransactionQueries::new()
    }

    fn transaction_repository() -> Self::TransactionRepository {
        umi_storage_rocksdb::transaction::RocksDbTransactionRepository::new()
    }

    impl_shared!();
}

lazy_static::lazy_static! {
    static ref Database: Arc<umi_storage_rocksdb::RocksDb> = {
        Arc::new(create_db())
    };
    static ref TRIE_DB: Arc<umi_storage_rocksdb::RocksEthTrieDb<'static>> = {
        Arc::new(umi_storage_rocksdb::RocksEthTrieDb::new(db()))
    };
}

fn db() -> &'static umi_storage_rocksdb::RocksDb {
    &Database
}

fn create_db() -> umi_storage_rocksdb::RocksDb {
    let path = "db";

    if std::env::var("PURGE").as_ref().map(String::as_str) == Ok("1") {
        let _ = std::fs::remove_dir_all(path);
    }

    let mut options = umi_storage_rocksdb::rocksdb::Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);

    umi_storage_rocksdb::RocksDb::open_cf(&options, path, umi_storage_rocksdb::COLUMN_FAMILIES)
        .expect("Database should open in db dir")
}
