use {
    crate::dependency::shared::*,
    std::sync::Arc,
    umi_app::{Application, CommandActor},
    umi_genesis::config::GenesisConfig,
};

pub type Dependency = InMemoryDependencies;

pub fn dependencies() -> Dependency {
    InMemoryDependencies::new()
}

pub struct InMemoryDependencies {
    memory_reader: umi_blockchain::in_memory::SharedMemoryReader,
    memory: Option<umi_blockchain::in_memory::SharedMemory>,
    receipt_memory_reader: umi_blockchain::receipt::ReceiptMemoryReader,
    receipt_memory: Option<umi_blockchain::receipt::ReceiptMemory>,
    trie_db: Arc<umi_state::InMemoryTrieDb>,
    evm_storage_tries: umi_evm_ext::state::InMemoryStorageTrieRepository,
}

impl InMemoryDependencies {
    pub fn new() -> Self {
        let (memory_reader, memory) = umi_blockchain::in_memory::shared_memory::new();
        let (receipt_memory_reader, receipt_memory) =
            umi_blockchain::receipt::receipt_memory::new();

        Self {
            memory_reader,
            memory: Some(memory),
            receipt_memory_reader,
            receipt_memory: Some(receipt_memory),
            trie_db: Arc::new(umi_state::InMemoryTrieDb::empty()),
            evm_storage_tries: umi_evm_ext::state::InMemoryStorageTrieRepository::new(),
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
        }
    }
}

impl Default for InMemoryDependencies {
    fn default() -> Self {
        Self::new()
    }
}

impl umi_app::Dependencies for InMemoryDependencies {
    type BlockQueries = umi_blockchain::block::InMemoryBlockQueries;
    type BlockRepository = umi_blockchain::block::InMemoryBlockRepository;
    type OnPayload = umi_app::OnPayload<Application<Self>>;
    type OnTx = umi_app::OnTx<Application<Self>>;
    type OnTxBatch = umi_app::OnTxBatch<Application<Self>>;
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
        umi_blockchain::payload::InMemoryPayloadQueries::new()
    }

    fn receipt_queries() -> Self::ReceiptQueries {
        umi_blockchain::receipt::InMemoryReceiptQueries::new()
    }

    fn receipt_repository() -> Self::ReceiptRepository {
        umi_blockchain::receipt::InMemoryReceiptRepository::new()
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
