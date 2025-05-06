use {
    crate::dependency::shared::*,
    moved_app::{Application, ApplicationReader, CommandActor},
    moved_genesis::config::GenesisConfig,
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
    type SharedStorage = moved_blockchain::in_memory::SharedMemory;
    type ReceiptStorage = moved_blockchain::receipt::ReceiptMemory;
    type StateQueries = moved_blockchain::state::InMemoryStateQueries;
    type ReceiptRepository = moved_blockchain::receipt::InMemoryReceiptRepository;
    type OnPayload = moved_app::OnPayload<Application<Self>>;
    type OnTx = moved_app::OnTx<Application<Self>>;
    type OnTxBatch = moved_app::OnTxBatch<Application<Self>>;
    type ReceiptQueries = moved_blockchain::receipt::InMemoryReceiptQueries;
    type PayloadQueries = moved_blockchain::payload::InMemoryPayloadQueries;
    type StorageTrieRepository = moved_evm_ext::state::InMemoryStorageTrieRepository;
    type TransactionRepository = moved_blockchain::transaction::InMemoryTransactionRepository;
    type TransactionQueries = moved_blockchain::transaction::InMemoryTransactionQueries;
    type BlockQueries = moved_blockchain::block::InMemoryBlockQueries;
    type BlockRepository = moved_blockchain::block::InMemoryBlockRepository;
    type State = moved_state::InMemoryState;

    fn block_repository() -> Self::BlockRepository {
        moved_blockchain::block::InMemoryBlockRepository::new()
    }

    fn state() -> Self::State {
        moved_state::InMemoryState::new()
    }

    fn on_tx_batch() -> &'static Self::OnTxBatch {
        CommandActor::on_tx_batch_in_memory()
    }

    fn on_tx() -> &'static Self::OnTx {
        CommandActor::on_tx_in_memory()
    }

    fn on_payload() -> &'static Self::OnPayload {
        CommandActor::on_payload_in_memory()
    }

    fn transaction_repository() -> Self::TransactionRepository {
        moved_blockchain::transaction::InMemoryTransactionRepository::new()
    }

    fn transaction_queries() -> Self::TransactionQueries {
        moved_blockchain::transaction::InMemoryTransactionQueries::new()
    }

    fn receipt_repository() -> Self::ReceiptRepository {
        moved_blockchain::receipt::InMemoryReceiptRepository::new()
    }

    fn receipt_queries() -> Self::ReceiptQueries {
        moved_blockchain::receipt::InMemoryReceiptQueries::new()
    }

    fn receipt_memory() -> Self::ReceiptStorage {
        moved_blockchain::receipt::ReceiptMemory::new()
    }

    fn block_queries() -> Self::BlockQueries {
        moved_blockchain::block::InMemoryBlockQueries
    }

    fn payload_queries() -> Self::PayloadQueries {
        moved_blockchain::payload::InMemoryPayloadQueries::new()
    }

    fn storage_trie_repository() -> Self::StorageTrieRepository {
        moved_evm_ext::state::InMemoryStorageTrieRepository::new()
    }

    fn shared_storage() -> Self::SharedStorage {
        moved_blockchain::in_memory::SharedMemory::new()
    }

    fn state_queries(genesis_config: &GenesisConfig) -> Self::StateQueries {
        moved_blockchain::state::InMemoryStateQueries::from_genesis(
            genesis_config.initial_state_root,
        )
    }

    impl_shared!();
}
