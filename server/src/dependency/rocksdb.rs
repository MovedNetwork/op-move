use {
    crate::dependency::shared::*,
    umi_app::{Application, ApplicationReader, CommandActor},
    umi_blockchain::state::EthTrieStateQueries,
    umi_genesis::config::GenesisConfig,
    umi_state::{EthTrieState, State},
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

impl umi_app::Dependencies for RocksDbDependencies {
    type BlockQueries = umi_storage_rocksdb::block::RocksDbBlockQueries;
    type BlockRepository = umi_storage_rocksdb::block::RocksDbBlockRepository;
    type OnPayload = umi_app::OnPayload<Application<Self>>;
    type OnTx = umi_app::OnTx<Application<Self>>;
    type OnTxBatch = umi_app::OnTxBatch<Application<Self>>;
    type PayloadQueries = umi_storage_rocksdb::payload::RocksDbPayloadQueries;
    type ReceiptQueries = umi_storage_rocksdb::receipt::RocksDbReceiptQueries;
    type ReceiptRepository = umi_storage_rocksdb::receipt::RocksDbReceiptRepository;
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
    type TransactionQueries = umi_storage_rocksdb::transaction::RocksDbTransactionQueries;
    type TransactionRepository = umi_storage_rocksdb::transaction::RocksDbTransactionRepository;

    fn block_queries() -> Self::BlockQueries {
        umi_storage_rocksdb::block::RocksDbBlockQueries
    }

    fn block_repository() -> Self::BlockRepository {
        umi_storage_rocksdb::block::RocksDbBlockRepository
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
        umi_storage_rocksdb::receipt::RocksDbReceiptQueries
    }

    fn receipt_repository() -> Self::ReceiptRepository {
        umi_storage_rocksdb::receipt::RocksDbReceiptRepository
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
        EthTrieStateQueries::new(
            umi_storage_rocksdb::RocksDbStateRootIndex::new(db()),
            TRIE_DB.clone(),
            genesis_config.initial_state_root,
        )
    }

    fn storage_trie_repository() -> Self::StorageTrieRepository {
        umi_storage_rocksdb::evm::RocksDbStorageTrieRepository::new(db())
    }

    fn transaction_queries() -> Self::TransactionQueries {
        umi_storage_rocksdb::transaction::RocksDbTransactionQueries
    }

    fn transaction_repository() -> Self::TransactionRepository {
        umi_storage_rocksdb::transaction::RocksDbTransactionRepository
    }

    impl_shared!();
}

lazy_static::lazy_static! {
    static ref Database: umi_storage_rocksdb::RocksDb = {
        create_db()
    };
    static ref TRIE_DB: std::sync::Arc<umi_storage_rocksdb::RocksEthTrieDb<'static>> = {
        std::sync::Arc::new(
            umi_storage_rocksdb::RocksEthTrieDb::new(db()),
        )
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
