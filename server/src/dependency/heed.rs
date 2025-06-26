use {
    crate::dependency::shared::*,
    std::sync::Arc,
    umi_app::{Application, CommandActor, SharedBlockHashCache, SharedHybridBlockHashCache},
    umi_blockchain::state::EthTrieStateQueries,
    umi_genesis::config::GenesisConfig,
    umi_state::{EthTrieState, State},
    umi_storage_heed::{
        block, evm, evm_storage_trie, heed::EnvOpenOptions, payload, receipt, state, transaction,
        trie,
    },
};

pub type Dependency = HeedDependencies;
pub type ReaderDependency = HeedReaderDependencies;

pub fn dependencies(args: umi_server_args::Database) -> Dependency {
    HeedDependencies {
        db: create_db(args),
        in_progress_payloads: Default::default(),
    }
}

pub struct HeedDependencies {
    db: umi_storage_heed::Env,
    in_progress_payloads: umi_blockchain::payload::InProgressPayloads,
}

pub struct HeedReaderDependencies {
    db: umi_storage_heed::Env,
    in_progress_payloads: umi_blockchain::payload::InProgressPayloads,
}

impl HeedDependencies {
    /// Creates a set of dependencies appropriate for usage in reader.
    pub fn reader(&self) -> HeedReaderDependencies {
        HeedReaderDependencies {
            db: self.db.clone(),
            in_progress_payloads: self.in_progress_payloads.clone(),
        }
    }
}

impl<'db> umi_app::Dependencies<'db> for HeedDependencies {
    type BlockHashLookup = SharedBlockHashCache;
    type BlockHashWriter = SharedBlockHashCache;
    type BlockQueries = block::HeedBlockQueries<'db>;
    type BlockRepository = block::HeedBlockRepository<'db>;
    type OnPayload = umi_app::OnPayload<Application<'db, Self>>;
    type OnTx = umi_app::OnTx<Application<'db, Self>>;
    type OnTxBatch = umi_app::OnTxBatch<Application<'db, Self>>;
    type PayloadQueries = payload::HeedPayloadQueries;
    type ReceiptQueries = receipt::HeedReceiptQueries<'db>;
    type ReceiptRepository = receipt::HeedReceiptRepository<'db>;
    type ReceiptStorage = umi_storage_heed::Env;
    type SharedStorage = umi_storage_heed::Env;
    type ReceiptStorageReader = umi_storage_heed::Env;
    type SharedStorageReader = umi_storage_heed::Env;
    type State = EthTrieState<trie::HeedEthTrieDb>;
    type StateQueries = EthTrieStateQueries<state::HeedStateRootIndex, trie::HeedEthTrieDb>;
    type StorageTrieRepository = evm::HeedStorageTrieRepository;
    type TransactionQueries = transaction::HeedTransactionQueries<'db>;
    type TransactionRepository = transaction::HeedTransactionRepository<'db>;

    fn block_hash_lookup(&self) -> Self::BlockHashLookup {
        SharedBlockHashCache::new()
    }

    fn block_hash_writer(&self) -> Self::BlockHashWriter {
        SharedBlockHashCache::new()
    }

    fn block_queries() -> Self::BlockQueries {
        block::HeedBlockQueries::new()
    }

    fn block_repository() -> Self::BlockRepository {
        block::HeedBlockRepository::new()
    }

    fn on_payload() -> &'db Self::OnPayload {
        &|state, id, hash| state.payload_queries.add_block_hash(id, hash).unwrap()
    }

    fn on_tx() -> &'db Self::OnTx {
        CommandActor::on_tx_noop()
    }

    fn on_tx_batch() -> &'db Self::OnTxBatch {
        &|state| {
            state
                .state_queries
                .push_state_root(state.state.state_root())
                .unwrap()
        }
    }

    fn payload_queries(&self) -> Self::PayloadQueries {
        payload::HeedPayloadQueries::new(self.db.clone(), self.in_progress_payloads.clone())
    }

    fn receipt_queries() -> Self::ReceiptQueries {
        receipt::HeedReceiptQueries::new()
    }

    fn receipt_repository() -> Self::ReceiptRepository {
        receipt::HeedReceiptRepository::new()
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
        fallible::retry(|| {
            EthTrieState::try_new(Arc::new(trie::HeedEthTrieDb::new(self.db.clone())))
        })
    }

    fn state_queries(&self, genesis_config: &GenesisConfig) -> Self::StateQueries {
        EthTrieStateQueries::new(
            state::HeedStateRootIndex::new(self.db.clone()),
            Arc::new(trie::HeedEthTrieDb::new(self.db.clone())),
            genesis_config.initial_state_root,
        )
    }

    fn storage_trie_repository(&self) -> Self::StorageTrieRepository {
        evm::HeedStorageTrieRepository::new(self.db.clone())
    }

    fn transaction_queries() -> Self::TransactionQueries {
        transaction::HeedTransactionQueries::new()
    }

    fn transaction_repository() -> Self::TransactionRepository {
        transaction::HeedTransactionRepository::new()
    }

    impl_shared!();
}

impl<'db> umi_app::Dependencies<'db> for HeedReaderDependencies {
    type BlockHashLookup =
        SharedHybridBlockHashCache<Self::SharedStorageReader, Self::BlockQueries>;
    type BlockHashWriter =
        SharedHybridBlockHashCache<Self::SharedStorageReader, Self::BlockQueries>;
    type BlockQueries = block::HeedBlockQueries<'db>;
    type BlockRepository = block::HeedBlockRepository<'db>;
    type OnPayload = umi_app::OnPayload<Application<'db, Self>>;
    type OnTx = umi_app::OnTx<Application<'db, Self>>;
    type OnTxBatch = umi_app::OnTxBatch<Application<'db, Self>>;
    type PayloadQueries = payload::HeedPayloadQueries;
    type ReceiptQueries = receipt::HeedReceiptQueries<'db>;
    type ReceiptRepository = receipt::HeedReceiptRepository<'db>;
    type ReceiptStorage = umi_storage_heed::Env;
    type SharedStorage = umi_storage_heed::Env;
    type ReceiptStorageReader = umi_storage_heed::Env;
    type SharedStorageReader = umi_storage_heed::Env;
    type State = EthTrieState<trie::HeedEthTrieDb>;
    type StateQueries = EthTrieStateQueries<state::HeedStateRootIndex, trie::HeedEthTrieDb>;
    type StorageTrieRepository = evm::HeedStorageTrieRepository;
    type TransactionQueries = transaction::HeedTransactionQueries<'db>;
    type TransactionRepository = transaction::HeedTransactionRepository<'db>;

    fn block_hash_lookup(&self) -> Self::BlockHashLookup {
        SharedHybridBlockHashCache::new(self.db.clone(), block::HeedBlockQueries::new())
    }

    fn block_hash_writer(&self) -> Self::BlockHashWriter {
        SharedHybridBlockHashCache::new(self.db.clone(), block::HeedBlockQueries::new())
    }

    fn block_queries() -> Self::BlockQueries {
        block::HeedBlockQueries::new()
    }

    fn block_repository() -> Self::BlockRepository {
        block::HeedBlockRepository::new()
    }

    fn on_payload() -> &'db Self::OnPayload {
        &|state, id, hash| state.payload_queries.add_block_hash(id, hash).unwrap()
    }

    fn on_tx() -> &'db Self::OnTx {
        CommandActor::on_tx_noop()
    }

    fn on_tx_batch() -> &'db Self::OnTxBatch {
        &|state| {
            state
                .state_queries
                .push_state_root(state.state.state_root())
                .unwrap()
        }
    }

    fn payload_queries(&self) -> Self::PayloadQueries {
        payload::HeedPayloadQueries::new(self.db.clone(), self.in_progress_payloads.clone())
    }

    fn receipt_queries() -> Self::ReceiptQueries {
        receipt::HeedReceiptQueries::new()
    }

    fn receipt_repository() -> Self::ReceiptRepository {
        receipt::HeedReceiptRepository::new()
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
        fallible::retry(|| {
            EthTrieState::try_new(Arc::new(trie::HeedEthTrieDb::new(self.db.clone())))
        })
    }

    fn state_queries(&self, genesis_config: &GenesisConfig) -> Self::StateQueries {
        EthTrieStateQueries::new(
            state::HeedStateRootIndex::new(self.db.clone()),
            Arc::new(trie::HeedEthTrieDb::new(self.db.clone())),
            genesis_config.initial_state_root,
        )
    }

    fn storage_trie_repository(&self) -> Self::StorageTrieRepository {
        evm::HeedStorageTrieRepository::new(self.db.clone())
    }

    fn transaction_queries() -> Self::TransactionQueries {
        transaction::HeedTransactionQueries::new()
    }

    fn transaction_repository() -> Self::TransactionRepository {
        transaction::HeedTransactionRepository::new()
    }

    impl_shared!();
}

fn create_db(args: umi_server_args::Database) -> umi_storage_heed::Env {
    assert_eq!(umi_storage_heed::DATABASES.len(), 11);

    if args.purge {
        let _ = std::fs::remove_dir_all(&args.dir);
    }
    let _ = std::fs::create_dir(&args.dir);

    let env = unsafe {
        EnvOpenOptions::new()
            .max_readers(16384)
            .max_dbs(umi_storage_heed::DATABASES.len() as u32)
            .map_size(1024 * 1024 * 1024 * 1024) // 1 TiB
            .open(&args.dir)
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
