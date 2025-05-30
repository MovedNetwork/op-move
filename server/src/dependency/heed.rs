use {
    crate::dependency::shared::*,
    umi_app::{Application, ApplicationReader, CommandActor},
    umi_blockchain::state::EthTrieStateQueries,
    umi_genesis::config::GenesisConfig,
    umi_state::{EthTrieState, State},
    umi_storage_heed::{
        block, evm, evm_storage_trie, heed::EnvOpenOptions, payload, receipt, state, transaction,
        trie,
    },
};

pub type Dependency = HeedDependencies;

pub fn create(
    genesis_config: &GenesisConfig,
) -> (
    Application<HeedDependencies>,
    ApplicationReader<HeedDependencies>,
) {
    (
        Application::new(HeedDependencies, genesis_config),
        ApplicationReader::new(HeedDependencies, genesis_config),
    )
}

pub struct HeedDependencies;

impl umi_app::Dependencies for HeedDependencies {
    type BlockQueries = block::HeedBlockQueries;
    type BlockRepository = block::HeedBlockRepository;
    type OnPayload = umi_app::OnPayload<Application<Self>>;
    type OnTx = umi_app::OnTx<Application<Self>>;
    type OnTxBatch = umi_app::OnTxBatch<Application<Self>>;
    type PayloadQueries = payload::HeedPayloadQueries;
    type ReceiptQueries = receipt::HeedReceiptQueries;
    type ReceiptRepository = receipt::HeedReceiptRepository;
    type ReceiptStorage = &'static umi_storage_heed::Env;
    type SharedStorage = &'static umi_storage_heed::Env;
    type ReceiptStorageReader = &'static umi_storage_heed::Env;
    type SharedStorageReader = &'static umi_storage_heed::Env;
    type State = EthTrieState<trie::HeedEthTrieDb<'static>>;
    type StateQueries =
        EthTrieStateQueries<state::HeedStateRootIndex<'static>, trie::HeedEthTrieDb<'static>>;
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
        payload::HeedPayloadQueries::new(db())
    }

    fn receipt_queries() -> Self::ReceiptQueries {
        receipt::HeedReceiptQueries
    }

    fn receipt_repository() -> Self::ReceiptRepository {
        receipt::HeedReceiptRepository
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
            state::HeedStateRootIndex::new(db()),
            TRIE_DB.clone(),
            genesis_config.initial_state_root,
        )
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
    static ref Database: umi_storage_heed::Env = {
        create_db()
    };
    static ref TRIE_DB: std::sync::Arc<trie::HeedEthTrieDb<'static>> = {
        std::sync::Arc::new(trie::HeedEthTrieDb::new(db()))
    };
}

fn db() -> &'static umi_storage_heed::Env {
    &Database
}

fn create_db() -> umi_storage_heed::Env {
    assert_eq!(umi_storage_heed::DATABASES.len(), 11);

    let path = "db";

    if std::env::var("PURGE").as_ref().map(String::as_str) == Ok("1") {
        let _ = std::fs::remove_dir_all(path);
    }
    let _ = std::fs::create_dir(path);

    let env = unsafe {
        EnvOpenOptions::new()
            .max_readers(20)
            .max_dbs(umi_storage_heed::DATABASES.len() as u32)
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
