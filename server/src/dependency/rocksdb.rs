use {
    crate::dependency::shared::*,
    moved_blockchain::{
        block::{BaseGasFee, BlockHash, BlockRepository, MovedBlockHash},
        payload::NewPayloadId,
    },
    moved_execution::{BaseTokenAccounts, CreateL1GasFee, CreateL2GasFee, MovedBaseTokenAccounts},
    moved_genesis::config::GenesisConfig,
    moved_state::State,
};

pub type SharedStorage = &'static moved_storage_rocksdb::RocksDb;
pub type ReceiptStorage = &'static moved_storage_rocksdb::RocksDb;
pub type StateQueries = moved_storage_rocksdb::RocksDbStateQueries<'static>;
pub type ReceiptRepository = moved_storage_rocksdb::receipt::RocksDbReceiptRepository;
pub type ReceiptQueries = moved_storage_rocksdb::receipt::RocksDbReceiptQueries;
pub type PayloadQueries = moved_storage_rocksdb::payload::RocksDbPayloadQueries;
pub type StorageTrieRepository = moved_storage_rocksdb::evm::RocksDbStorageTrieRepository;
pub type TransactionRepository = moved_storage_rocksdb::transaction::RocksDbTransactionRepository;
pub type TransactionQueries = moved_storage_rocksdb::transaction::RocksDbTransactionQueries;
pub type BlockQueries = moved_storage_rocksdb::block::RocksDbBlockQueries;

pub fn block_hash() -> impl BlockHash + Send + Sync + 'static {
    MovedBlockHash
}

pub fn base_token(
    genesis_config: &GenesisConfig,
) -> impl BaseTokenAccounts + Send + Sync + 'static {
    MovedBaseTokenAccounts::new(genesis_config.treasury)
}

pub fn memory() -> SharedStorage {
    db()
}

pub fn block_repository() -> impl BlockRepository<Storage = SharedStorage> + Send + Sync + 'static {
    moved_storage_rocksdb::block::RocksDbBlockRepository
}

pub fn state() -> impl State + Send + Sync + 'static {
    moved_storage_rocksdb::RocksDbState::new(std::sync::Arc::new(
        moved_storage_rocksdb::RocksEthTrieDb::new(db()),
    ))
}

pub fn state_query(genesis_config: &GenesisConfig) -> StateQueries {
    moved_storage_rocksdb::RocksDbStateQueries::from_genesis(
        db(),
        genesis_config.initial_state_root,
    )
}

pub fn on_tx_batch<
    A: State,
    B: NewPayloadId,
    C: BlockHash,
    D: BlockRepository<Storage = SharedStorage>,
    E: BaseGasFee,
    F: CreateL1GasFee,
    G: CreateL2GasFee,
    H: BaseTokenAccounts,
>() -> OnTxBatch<A, B, C, D, E, F, G, H> {
    Box::new(|| {
        Box::new(|state| {
            state
                .state_queries()
                .push_state_root(state.state().state_root())
                .unwrap()
        })
    })
}

pub fn on_tx<
    A: State,
    B: NewPayloadId,
    C: BlockHash,
    D: BlockRepository<Storage = SharedStorage>,
    E: BaseGasFee,
    F: CreateL1GasFee,
    G: CreateL2GasFee,
    H: BaseTokenAccounts,
>() -> OnTx<A, B, C, D, E, F, G, H> {
    moved_app::StateActor::on_tx_noop()
}

pub fn on_payload<
    A: State,
    B: NewPayloadId,
    C: BlockHash,
    D: BlockRepository<Storage = SharedStorage>,
    E: BaseGasFee,
    F: CreateL1GasFee,
    G: CreateL2GasFee,
    H: BaseTokenAccounts,
>() -> OnPayload<A, B, C, D, E, F, G, H> {
    Box::new(|| {
        Box::new(|state, id, hash| state.payload_queries().add_block_hash(id, hash).unwrap())
    })
}

pub fn transaction_repository() -> TransactionRepository {
    moved_storage_rocksdb::transaction::RocksDbTransactionRepository
}

pub fn transaction_queries() -> TransactionQueries {
    moved_storage_rocksdb::transaction::RocksDbTransactionQueries
}

pub fn receipt_repository() -> ReceiptRepository {
    moved_storage_rocksdb::receipt::RocksDbReceiptRepository
}

pub fn receipt_queries() -> ReceiptQueries {
    moved_storage_rocksdb::receipt::RocksDbReceiptQueries
}

pub fn receipt_memory() -> ReceiptStorage {
    db()
}

pub fn block_queries() -> BlockQueries {
    moved_storage_rocksdb::block::RocksDbBlockQueries
}

pub fn payload_queries() -> PayloadQueries {
    moved_storage_rocksdb::payload::RocksDbPayloadQueries::new(db())
}

pub fn storage_trie_repository() -> StorageTrieRepository {
    moved_storage_rocksdb::evm::RocksDbStorageTrieRepository::new(db())
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
