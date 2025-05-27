use {
    crate::dependency::shared::*,
    moved_app::{Application, ApplicationReader, CommandActor},
    moved_genesis::config::GenesisConfig,
    moved_state::{EthTrieState, State},
};

pub type Dependency = RocksDbDependencies;

pub fn create(
    genesis_config: &GenesisConfig,
) -> (
    Application<RocksDbDependencies>,
    ApplicationReader<RocksDbDependencies>,
) {
    (
        Application::new(RocksDbDependencies, genesis_config),
        ApplicationReader::new(RocksDbDependencies, genesis_config),
    )
}

pub struct RocksDbDependencies;

impl moved_app::Dependencies for RocksDbDependencies {
    type BlockQueries = moved_storage_rocksdb::block::RocksDbBlockQueries;
    type BlockRepository = moved_storage_rocksdb::block::RocksDbBlockRepository;
    type OnPayload = moved_app::OnPayload<Application<Self>>;
    type OnTx = moved_app::OnTx<Application<Self>>;
    type OnTxBatch = moved_app::OnTxBatch<Application<Self>>;
    type PayloadQueries = moved_storage_rocksdb::payload::RocksDbPayloadQueries;
    type ReceiptQueries = moved_storage_rocksdb::receipt::RocksDbReceiptQueries;
    type ReceiptRepository = moved_storage_rocksdb::receipt::RocksDbReceiptRepository;
    type ReceiptStorage = &'static moved_storage_rocksdb::RocksDb;
    type SharedStorage = &'static moved_storage_rocksdb::RocksDb;
    type ReceiptStorageReader = &'static moved_storage_rocksdb::RocksDb;
    type SharedStorageReader = &'static moved_storage_rocksdb::RocksDb;
    type State = EthTrieState<moved_storage_rocksdb::RocksEthTrieDb<'static>>;
    type StateQueries = moved_storage_rocksdb::RocksDbStateQueries<'static>;
    type StorageTrieRepository = moved_storage_rocksdb::evm::RocksDbStorageTrieRepository;
    type TransactionQueries = moved_storage_rocksdb::transaction::RocksDbTransactionQueries;
    type TransactionRepository = moved_storage_rocksdb::transaction::RocksDbTransactionRepository;

    fn block_queries() -> Self::BlockQueries {
        moved_storage_rocksdb::block::RocksDbBlockQueries
    }

    fn block_repository() -> Self::BlockRepository {
        moved_storage_rocksdb::block::RocksDbBlockRepository
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
        moved_storage_rocksdb::payload::RocksDbPayloadQueries::new(db())
    }

    fn receipt_queries() -> Self::ReceiptQueries {
        moved_storage_rocksdb::receipt::RocksDbReceiptQueries
    }

    fn receipt_repository() -> Self::ReceiptRepository {
        moved_storage_rocksdb::receipt::RocksDbReceiptRepository
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
        EthTrieState::try_new(TRIE_DB.clone()).unwrap()
    }

    fn state_queries(&self, genesis_config: &GenesisConfig) -> Self::StateQueries {
        moved_storage_rocksdb::RocksDbStateQueries::new(
            db(),
            TRIE_DB.clone(),
            genesis_config.initial_state_root,
        )
    }

    fn storage_trie_repository() -> Self::StorageTrieRepository {
        moved_storage_rocksdb::evm::RocksDbStorageTrieRepository::new(db())
    }

    fn transaction_queries() -> Self::TransactionQueries {
        moved_storage_rocksdb::transaction::RocksDbTransactionQueries
    }

    fn transaction_repository() -> Self::TransactionRepository {
        moved_storage_rocksdb::transaction::RocksDbTransactionRepository
    }

    impl_shared!();
}

lazy_static::lazy_static! {
    static ref Database: moved_storage_rocksdb::RocksDb = {
        create_db()
    };
    static ref TRIE_DB: std::sync::Arc<moved_storage_rocksdb::RocksEthTrieDb<'static>> = {
        std::sync::Arc::new(
            moved_storage_rocksdb::RocksEthTrieDb::new(db()),
        )
    };
}

fn db() -> &'static moved_storage_rocksdb::RocksDb {
    &Database
}

fn create_db() -> moved_storage_rocksdb::RocksDb {
    let path = "db";

    if std::env::var("PURGE").as_ref().map(String::as_str) == Ok("1") {
        let _ = std::fs::remove_dir_all(path);
    }

    let mut options = moved_storage_rocksdb::rocksdb::Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);

    moved_storage_rocksdb::RocksDb::open_cf(&options, path, moved_storage_rocksdb::COLUMN_FAMILIES)
        .expect("Database should open in db dir")
}
