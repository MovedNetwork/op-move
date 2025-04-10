use {
    crate::dependency::shared::*,
    moved_app::{Application, StateActor},
    moved_genesis::config::GenesisConfig,
    moved_state::State,
    moved_storage_heed::{
        block, evm, evm_storage_trie, heed::EnvOpenOptions, payload, receipt, state, transaction,
        trie,
    },
};

pub type Dependency = HeedDependencies;

pub fn create(genesis_config: &GenesisConfig) -> Application<HeedDependencies> {
    Application::new(HeedDependencies, genesis_config)
}

pub struct HeedDependencies;

impl moved_app::Dependencies for HeedDependencies {
    type BlockQueries = block::HeedBlockQueries;
    type BlockRepository = block::HeedBlockRepository;
    type OnPayload = moved_app::OnPayload<Application<Self>>;
    type OnTx = moved_app::OnTx<Application<Self>>;
    type OnTxBatch = moved_app::OnTxBatch<Application<Self>>;
    type PayloadQueries = payload::HeedPayloadQueries;
    type ReceiptQueries = receipt::HeedReceiptQueries;
    type ReceiptRepository = receipt::HeedReceiptRepository;
    type ReceiptStorage = &'static moved_storage_heed::Env;
    type SharedStorage = &'static moved_storage_heed::Env;
    type State = state::HeedState<'static>;
    type StateQueries = state::HeedStateQueries<'static>;
    type StorageTrieRepository = evm::HeedStorageTrieRepository;
    type TransactionQueries = transaction::HeedTransactionQueries;
    type TransactionRepository = transaction::HeedTransactionRepository;

    fn block_queries() -> Self::BlockQueries {
        block::HeedBlockQueries
    }

    fn block_repository() -> Self::BlockRepository {
        block::HeedBlockRepository
    }

    fn on_payload() -> &'static Self::OnPayload {
        &|state, id, hash| state.payload_queries.add_block_hash(id, hash).unwrap()
    }

    fn on_tx() -> &'static Self::OnTx {
        StateActor::on_tx_noop()
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
        payload::HeedPayloadQueries::new(db())
    }

    fn receipt_queries() -> Self::ReceiptQueries {
        receipt::HeedReceiptQueries
    }

    fn receipt_repository() -> Self::ReceiptRepository {
        receipt::HeedReceiptRepository
    }

    fn receipt_memory() -> Self::ReceiptStorage {
        db()
    }

    fn shared_storage() -> Self::SharedStorage {
        db()
    }

    fn state() -> Self::State {
        state::HeedState::new(std::sync::Arc::new(trie::HeedEthTrieDb::new(db())))
    }

    fn state_queries(genesis_config: &GenesisConfig) -> Self::StateQueries {
        state::HeedStateQueries::from_genesis(db(), genesis_config.initial_state_root)
    }

    fn storage_trie_repository() -> Self::StorageTrieRepository {
        evm::HeedStorageTrieRepository::new(db())
    }

    fn transaction_queries() -> Self::TransactionQueries {
        transaction::HeedTransactionQueries
    }

    fn transaction_repository() -> Self::TransactionRepository {
        transaction::HeedTransactionRepository
    }

    impl_shared!();
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
            .max_readers(20)
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
