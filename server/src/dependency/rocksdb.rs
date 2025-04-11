use {
    crate::dependency::shared::*,
    move_core_types::effects::ChangeSet,
    moved_app::{Application, CommandActor},
    moved_genesis::config::GenesisConfig,
    moved_state::State,
};

pub type Dependency = RocksDbDependencies;

pub fn create(genesis_config: &GenesisConfig) -> Application<RocksDbDependencies> {
    Application::new(RocksDbDependencies, genesis_config)
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
    type State = moved_storage_rocksdb::RocksDbState<'static>;
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

    fn receipt_memory() -> Self::ReceiptStorage {
        db()
    }

    fn shared_storage() -> Self::SharedStorage {
        db()
    }

    fn state() -> Self::State {
        moved_storage_rocksdb::RocksDbState::new(std::sync::Arc::new(
            moved_storage_rocksdb::RocksEthTrieDb::new(db()),
        ))
    }

    fn state_queries(genesis_config: &GenesisConfig) -> Self::StateQueries {
        moved_storage_rocksdb::RocksDbStateQueries::from_genesis(
            db(),
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
}

fn db() -> &'static moved_storage_rocksdb::RocksDb {
    &Database
}

fn create_db() -> moved_storage_rocksdb::RocksDb {
    let path = "db";

    if std::fs::exists(path).unwrap() {
        std::fs::remove_dir_all(path)
            .expect("Removing non-empty database directory should succeed");
    }

    let mut options = moved_storage_rocksdb::rocksdb::Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);

    moved_storage_rocksdb::RocksDb::open_cf(&options, path, moved_storage_rocksdb::COLUMN_FAMILIES)
        .expect("Database should open in db dir")
}
