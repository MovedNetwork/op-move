use {
    crate::dependency::shared::*,
    moved_app::{Application, ApplicationReader, CommandActor},
    moved_genesis::config::GenesisConfig,
    std::sync::Arc,
};

pub type Dependency = InMemoryDependencies;

pub fn create(
    genesis_config: &GenesisConfig,
) -> (
    Application<InMemoryDependencies>,
    ApplicationReader<InMemoryDependencies>,
) {
    let deps = InMemoryDependencies::new();
    let reader_deps = deps.reader();

    (
        Application::new(deps, genesis_config),
        ApplicationReader::new(reader_deps, genesis_config),
    )
}

pub struct InMemoryDependencies {
    memory_reader: moved_blockchain::in_memory::SharedMemoryReader,
    memory: Option<moved_blockchain::in_memory::SharedMemory>,
    receipt_memory_reader: moved_blockchain::receipt::ReceiptMemoryReader,
    receipt_memory: Option<moved_blockchain::receipt::ReceiptMemory>,
    trie_db: Arc<moved_state::InMemoryTrieDb>,
}

impl InMemoryDependencies {
    pub fn new() -> Self {
        let (memory_reader, memory) = moved_blockchain::in_memory::shared_memory::new();
        let (receipt_memory_reader, receipt_memory) =
            moved_blockchain::receipt::receipt_memory::new();

        Self {
            memory_reader,
            memory: Some(memory),
            receipt_memory_reader,
            receipt_memory: Some(receipt_memory),
            trie_db: Arc::new(moved_state::InMemoryTrieDb::empty()),
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
        }
    }
}

impl Default for InMemoryDependencies {
    fn default() -> Self {
        Self::new()
    }
}

impl moved_app::Dependencies for InMemoryDependencies {
    type BlockQueries = moved_blockchain::block::InMemoryBlockQueries;
    type BlockRepository = moved_blockchain::block::InMemoryBlockRepository;
    type OnPayload = moved_app::OnPayload<Application<Self>>;
    type OnTx = moved_app::OnTx<Application<Self>>;
    type OnTxBatch = moved_app::OnTxBatch<Application<Self>>;
    type PayloadQueries = moved_blockchain::payload::InMemoryPayloadQueries;
    type ReceiptQueries = moved_blockchain::receipt::InMemoryReceiptQueries;
    type ReceiptRepository = moved_blockchain::receipt::InMemoryReceiptRepository;
    type ReceiptStorage = moved_blockchain::receipt::ReceiptMemory;
    type SharedStorage = moved_blockchain::in_memory::SharedMemory;
    type ReceiptStorageReader = moved_blockchain::receipt::ReceiptMemoryReader;
    type SharedStorageReader = moved_blockchain::in_memory::SharedMemoryReader;
    type State = moved_state::InMemoryState;
    type StateQueries = moved_blockchain::state::InMemoryStateQueries;
    type StorageTrieRepository = moved_evm_ext::state::InMemoryStorageTrieRepository;
    type TransactionQueries = moved_blockchain::transaction::InMemoryTransactionQueries;
    type TransactionRepository = moved_blockchain::transaction::InMemoryTransactionRepository;

    fn block_queries() -> Self::BlockQueries {
        moved_blockchain::block::InMemoryBlockQueries
    }

    fn block_repository() -> Self::BlockRepository {
        moved_blockchain::block::InMemoryBlockRepository::new()
    }

    fn on_payload() -> &'static Self::OnPayload {
        CommandActor::on_payload_in_memory()
    }

    fn on_tx() -> &'static Self::OnTx {
        CommandActor::on_tx_in_memory()
    }

    fn on_tx_batch() -> &'static Self::OnTxBatch {
        CommandActor::on_tx_batch_in_memory()
    }

    fn payload_queries() -> Self::PayloadQueries {
        moved_blockchain::payload::InMemoryPayloadQueries::new()
    }

    fn receipt_queries() -> Self::ReceiptQueries {
        moved_blockchain::receipt::InMemoryReceiptQueries::new()
    }

    fn receipt_repository() -> Self::ReceiptRepository {
        moved_blockchain::receipt::InMemoryReceiptRepository::new()
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
        moved_state::InMemoryState::try_new(self.trie_db.clone())
            .expect("State root should exist and be fetched")
    }

    fn state_queries(&self, genesis_config: &GenesisConfig) -> Self::StateQueries {
        moved_blockchain::state::InMemoryStateQueries::new(
            self.shared_storage_reader(),
            self.trie_db.clone(),
            genesis_config.initial_state_root,
        )
    }

    fn storage_trie_repository() -> Self::StorageTrieRepository {
        moved_evm_ext::state::InMemoryStorageTrieRepository::new()
    }

    fn transaction_queries() -> Self::TransactionQueries {
        moved_blockchain::transaction::InMemoryTransactionQueries::new()
    }

    fn transaction_repository() -> Self::TransactionRepository {
        moved_blockchain::transaction::InMemoryTransactionRepository::new()
    }

    impl_shared!();
}
