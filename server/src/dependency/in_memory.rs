use {
    crate::dependency::shared::*,
    std::sync::Arc,
    umi_app::{Application, CommandActor, HybridBlockHashCache},
    umi_blockchain::block::InMemoryBlockQueries,
    umi_genesis::config::GenesisConfig,
};

pub type Dependency = InMemoryDependencies;
// TODO: make the same separation as for other backends
pub type ReaderDependency = InMemoryDependencies;

pub fn dependencies(_args: umi_server_args::Database) -> Dependency {
    InMemoryDependencies::new()
}

unsafe impl Sync for InMemoryDependencies {}

pub struct InMemoryDependencies {
    memory_reader: umi_blockchain::in_memory::SharedMemoryReader,
    memory: Option<umi_blockchain::in_memory::SharedMemory>,
    receipt_memory_reader: umi_blockchain::receipt::ReceiptMemoryReader,
    receipt_memory: Option<umi_blockchain::receipt::ReceiptMemory>,
    trie_db: Arc<umi_state::InMemoryTrieDb>,
    evm_storage_tries: umi_evm_ext::state::InMemoryStorageTrieRepository,
    block_hash_cache:
        HybridBlockHashCache<umi_blockchain::in_memory::SharedMemoryReader, InMemoryBlockQueries>,
    in_progress_payloads: umi_blockchain::payload::InProgressPayloads,
}

impl InMemoryDependencies {
    pub fn new() -> Self {
        let (memory_reader, memory) = umi_blockchain::in_memory::shared_memory::new();
        let (receipt_memory_reader, receipt_memory) =
            umi_blockchain::receipt::receipt_memory::new();

        Self {
            memory_reader: memory_reader.clone(),
            memory: Some(memory),
            receipt_memory_reader,
            receipt_memory: Some(receipt_memory),
            trie_db: Arc::new(umi_state::InMemoryTrieDb::empty()),
            evm_storage_tries: umi_evm_ext::state::InMemoryStorageTrieRepository::new(),
            block_hash_cache: HybridBlockHashCache::new(memory_reader, InMemoryBlockQueries),
            in_progress_payloads: Default::default(),
        }
    }

    /// Creates a set of dependencies appropriate for usage in reader.
    ///
    /// All reader handles are connected to write handles in `self`, but there are no write handles.
    pub fn reader(&self) -> Self {
        Self {
            memory_reader: self.memory_reader.clone(),
            memory: None,
            receipt_memory_reader: self.receipt_memory_reader.clone(),
            receipt_memory: None,
            trie_db: self.trie_db.clone(),
            evm_storage_tries: self.evm_storage_tries.clone(),
            block_hash_cache: self.block_hash_cache.clone(),
            in_progress_payloads: self.in_progress_payloads.clone(),
        }
    }
}

impl Default for InMemoryDependencies {
    fn default() -> Self {
        Self::new()
    }
}

impl<'db> umi_app::Dependencies<'db> for InMemoryDependencies {
    type BlockQueries = umi_blockchain::block::InMemoryBlockQueries;
    type BlockRepository = umi_blockchain::block::InMemoryBlockRepository;
    type BlockHashLookup = umi_app::HybridBlockHashCache<
        umi_blockchain::in_memory::SharedMemoryReader,
        InMemoryBlockQueries,
    >;
    type BlockHashWriter = umi_app::HybridBlockHashCache<
        umi_blockchain::in_memory::SharedMemoryReader,
        InMemoryBlockQueries,
    >;
    type OnPayload = umi_app::OnPayload<Application<'db, Self>>;
    type OnTx = umi_app::OnTx<Application<'db, Self>>;
    type OnTxBatch = umi_app::OnTxBatch<Application<'db, Self>>;
    type PayloadQueries = umi_blockchain::payload::InMemoryPayloadQueries;
    type ReceiptQueries = umi_blockchain::receipt::InMemoryReceiptQueries;
    type ReceiptRepository = umi_blockchain::receipt::InMemoryReceiptRepository;
    type ReceiptStorage = umi_blockchain::receipt::ReceiptMemory;
    type SharedStorage = umi_blockchain::in_memory::SharedMemory;
    type ReceiptStorageReader = umi_blockchain::receipt::ReceiptMemoryReader;
    type SharedStorageReader = umi_blockchain::in_memory::SharedMemoryReader;
    type State = umi_state::InMemoryState;
    type StateQueries = umi_blockchain::state::InMemoryStateQueries;
    type StorageTrieRepository = umi_evm_ext::state::InMemoryStorageTrieRepository;
    type TransactionQueries = umi_blockchain::transaction::InMemoryTransactionQueries;
    type TransactionRepository = umi_blockchain::transaction::InMemoryTransactionRepository;

    fn block_queries() -> Self::BlockQueries {
        umi_blockchain::block::InMemoryBlockQueries
    }

    fn block_repository() -> Self::BlockRepository {
        umi_blockchain::block::InMemoryBlockRepository::new()
    }

    fn on_payload() -> &'db Self::OnPayload {
        CommandActor::on_payload_in_memory()
    }

    fn on_tx() -> &'db Self::OnTx {
        CommandActor::on_tx_in_memory()
    }

    fn on_tx_batch() -> &'db Self::OnTxBatch {
        CommandActor::on_tx_batch_in_memory()
    }

    fn payload_queries(&self) -> Self::PayloadQueries {
        umi_blockchain::payload::InMemoryPayloadQueries::new(self.in_progress_payloads.clone())
    }

    fn receipt_queries() -> Self::ReceiptQueries {
        umi_blockchain::receipt::InMemoryReceiptQueries::new()
    }

    fn receipt_repository() -> Self::ReceiptRepository {
        umi_blockchain::receipt::InMemoryReceiptRepository::new()
    }

    fn block_hash_lookup(&self) -> Self::BlockHashLookup {
        self.block_hash_cache.clone()
    }

    fn block_hash_writer(&self) -> Self::BlockHashWriter {
        self.block_hash_cache.clone()
    }

    fn receipt_memory(&mut self) -> Self::ReceiptStorage {
        self.receipt_memory
            .take()
            .expect("Writer cannot be taken more than once")
    }

    fn shared_storage(&mut self) -> Self::SharedStorage {
        self.memory
            .take()
            .expect("Writer cannot be taken more than once")
    }

    fn receipt_memory_reader(&self) -> Self::ReceiptStorageReader {
        self.receipt_memory_reader.clone()
    }

    fn shared_storage_reader(&self) -> Self::SharedStorageReader {
        self.memory_reader.clone()
    }

    fn state(&self) -> Self::State {
        umi_state::InMemoryState::try_new(self.trie_db.clone())
            .expect("State root should exist and be fetched")
    }

    fn state_queries(&self, genesis_config: &GenesisConfig) -> Self::StateQueries {
        umi_blockchain::state::InMemoryStateQueries::new(
            self.shared_storage_reader(),
            self.trie_db.clone(),
            genesis_config.initial_state_root,
        )
    }

    fn storage_trie_repository(&self) -> Self::StorageTrieRepository {
        self.evm_storage_tries.clone()
    }

    fn transaction_queries() -> Self::TransactionQueries {
        umi_blockchain::transaction::InMemoryTransactionQueries::new()
    }

    fn transaction_repository() -> Self::TransactionRepository {
        umi_blockchain::transaction::InMemoryTransactionRepository::new()
    }

    impl_shared!();
}
