use {
    crate::dependency::shared::*,
    moved_blockchain::{
        block::{BaseGasFee, BlockHash, BlockRepository, MovedBlockHash},
        payload::NewPayloadId,
    },
    moved_execution::{BaseTokenAccounts, CreateL1GasFee, CreateL2GasFee, MovedBaseTokenAccounts},
    moved_genesis::config::GenesisConfig,
    moved_state::State,
    moved_storage_heed::{
        block, evm, evm_storage_trie, heed::EnvOpenOptions, payload, receipt, state, transaction,
        trie,
    },
};

pub type SharedStorage = &'static moved_storage_heed::Env;
pub type ReceiptStorage = &'static moved_storage_heed::Env;
pub type StateQueries = state::HeedStateQueries<'static>;
pub type ReceiptRepository = receipt::HeedReceiptRepository;
pub type ReceiptQueries = receipt::HeedReceiptQueries;
pub type PayloadQueries = payload::HeedPayloadQueries;
pub type StorageTrieRepository = evm::HeedStorageTrieRepository;
pub type TransactionRepository = transaction::HeedTransactionRepository;
pub type TransactionQueries = transaction::HeedTransactionQueries;
pub type BlockQueries = block::HeedBlockQueries;

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
    block::HeedBlockRepository
}

pub fn state() -> impl State + Send + Sync + 'static {
    state::HeedState::new(std::sync::Arc::new(trie::HeedEthTrieDb::new(db())))
}

pub fn state_query(genesis_config: &GenesisConfig) -> StateQueries {
    state::HeedStateQueries::from_genesis(db(), genesis_config.initial_state_root)
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
    StateActor::on_tx_noop()
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
    transaction::HeedTransactionRepository
}

pub fn transaction_queries() -> TransactionQueries {
    transaction::HeedTransactionQueries
}

pub fn receipt_repository() -> ReceiptRepository {
    receipt::HeedReceiptRepository
}

pub fn receipt_queries() -> ReceiptQueries {
    receipt::HeedReceiptQueries
}

pub fn receipt_memory() -> ReceiptStorage {
    db()
}

pub fn block_queries() -> BlockQueries {
    block::HeedBlockQueries
}

pub fn payload_queries() -> PayloadQueries {
    payload::HeedPayloadQueries::new(db())
}

pub fn storage_trie_repository() -> StorageTrieRepository {
    evm::HeedStorageTrieRepository::new(db())
}

lazy_static::lazy_static! {
    static ref Database: moved_storage_heed::Env = {
        create_db()
    };
}

fn db() -> &'static moved_storage_heed::Env {
    &Database
}

fn create_db() -> moved_storage_heed::Env {
    assert_eq!(moved_storage_heed::DATABASES.len(), 11);

    let path = "db";

    if std::fs::exists(path).unwrap() {
        std::fs::remove_dir_all(path)
            .expect("Removing non-empty database directory should succeed");
    }
    std::fs::create_dir(path).unwrap();

    let env = unsafe {
        EnvOpenOptions::new()
            .max_dbs(moved_storage_heed::DATABASES.len() as u32)
            .map_size(1024 * 1024 * 1024 * 1024) // 1 TiB
            .open(path)
            .expect("Database dir should be accessible")
    };

    {
        let mut transaction = env.write_txn().expect("Transaction should be exclusive");

        let _: block::Db = env
            .create_database(&mut transaction, Some(block::DB))
            .expect("Database should be new");
        let _: block::HeightDb = env
            .create_database(&mut transaction, Some(block::HEIGHT_DB))
            .expect("Database should be new");
        let _: state::Db = env
            .create_database(&mut transaction, Some(state::DB))
            .expect("Database should be new");
        let _: state::HeightDb = env
            .create_database(&mut transaction, Some(state::HEIGHT_DB))
            .expect("Database should be new");
        let _: trie::Db = env
            .create_database(&mut transaction, Some(trie::DB))
            .expect("Database should be new");
        let _: trie::RootDb = env
            .create_database(&mut transaction, Some(trie::ROOT_DB))
            .expect("Database should be new");
        let _: evm_storage_trie::Db = env
            .create_database(&mut transaction, Some(evm_storage_trie::DB))
            .expect("Database should be new");
        let _: evm_storage_trie::RootDb = env
            .create_database(&mut transaction, Some(evm_storage_trie::ROOT_DB))
            .expect("Database should be new");
        let _: transaction::Db = env
            .create_database(&mut transaction, Some(transaction::DB))
            .expect("Database should be new");
        let _: receipt::Db = env
            .create_database(&mut transaction, Some(receipt::DB))
            .expect("Database should be new");
        let _: payload::Db = env
            .create_database(&mut transaction, Some(payload::DB))
            .expect("Database should be new");

        transaction.commit().expect("Transaction should succeed");
    }

    env
}
