use {
    crate::dependency::shared::*,
    moved_app::{Application, ApplicationReader, CommandActor},
    moved_blockchain::{in_memory::shared_memory, receipt::receipt_memory},
    moved_genesis::config::GenesisConfig,
    std::{
        iter,
        sync::{Arc, Mutex},
    },
};

pub type Dependency = InMemoryDependencies;

pub fn create(
    genesis_config: &GenesisConfig,
) -> (
    Application<InMemoryDependencies>,
    ApplicationReader<InMemoryDependencies>,
) {
    (
        Application::new(InMemoryDependencies, genesis_config),
        ApplicationReader::new(InMemoryDependencies, genesis_config),
    )
}

pub struct InMemoryDependencies;

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

    fn receipt_memory() -> Self::ReceiptStorage {
        RECEIPT_MEM
            .lock()
            .unwrap()
            .1
            .take()
            .expect("Should be called once")
    }

    fn shared_storage() -> Self::SharedStorage {
        SHARED_MEM
            .lock()
            .unwrap()
            .1
            .take()
            .expect("Should be called once")
    }

    fn receipt_memory_reader() -> Self::ReceiptStorageReader {
        RECEIPT_MEM
            .lock()
            .unwrap()
            .0
            .pop()
            .expect("Should be called once")
    }

    fn shared_storage_reader() -> Self::SharedStorageReader {
        SHARED_MEM
            .lock()
            .unwrap()
            .0
            .pop()
            .expect("Should be called once")
    }

    fn state() -> Self::State {
        moved_state::InMemoryState::new(TRIE_DB.clone())
    }

    fn state_queries(_genesis_config: &GenesisConfig) -> Self::StateQueries {
        moved_blockchain::state::InMemoryStateQueries::new(
            Self::shared_storage_reader(),
            TRIE_DB.clone(),
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

lazy_static::lazy_static! {
    static ref TRIE_DB: Arc<moved_state::InMemoryTrieDb> = moved_state::InMemoryState::create_db();
    static ref SHARED_MEM: Mutex<(
        Vec<moved_blockchain::in_memory::SharedMemoryReader>,
        Option<moved_blockchain::in_memory::SharedMemory>,
    )> = {
        let (memory_reader, memory) = shared_memory::new();

        Mutex::new((iter::repeat_n(memory_reader, 3).collect(), Some(memory)))
    };
    static ref RECEIPT_MEM: Mutex<(
        Vec<moved_blockchain::receipt::ReceiptMemoryReader>,
        Option<moved_blockchain::receipt::ReceiptMemory>,
    )> = {
        let (memory_reader, memory) = receipt_memory::new();

        Mutex::new((iter::repeat_n(memory_reader, 3).collect(), Some(memory)))
    };
}
