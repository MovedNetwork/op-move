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

pub type SharedStorage = moved_blockchain::in_memory::SharedMemory;
pub type ReceiptStorage = moved_blockchain::receipt::ReceiptMemory;
pub type StateQueries = moved_blockchain::state::InMemoryStateQueries;
pub type ReceiptRepository = moved_blockchain::receipt::InMemoryReceiptRepository;
pub type ReceiptQueries = moved_blockchain::receipt::InMemoryReceiptQueries;
pub type PayloadQueries = moved_blockchain::payload::InMemoryPayloadQueries;
pub type StorageTrieRepository = moved_evm_ext::storage::InMemoryStorageTrieRepository;
pub type TransactionRepository = moved_blockchain::transaction::InMemoryTransactionRepository;
pub type TransactionQueries = moved_blockchain::transaction::InMemoryTransactionQueries;
pub type BlockQueries = moved_blockchain::block::InMemoryBlockQueries;

pub fn block_hash() -> impl BlockHash + Send + Sync + 'static {
    MovedBlockHash
}

pub fn base_token(
    genesis_config: &GenesisConfig,
) -> impl BaseTokenAccounts + Send + Sync + 'static {
    MovedBaseTokenAccounts::new(genesis_config.treasury)
}

pub fn memory() -> SharedStorage {
    moved_blockchain::in_memory::SharedMemory::new()
}

pub fn block_repository() -> impl BlockRepository<Storage = SharedStorage> + Send + Sync + 'static {
    moved_blockchain::block::InMemoryBlockRepository::new()
}

pub fn state() -> impl State + Send + Sync + 'static {
    moved_state::InMemoryState::new()
}

pub fn state_query(genesis_config: &GenesisConfig) -> StateQueries {
    moved_blockchain::state::InMemoryStateQueries::from_genesis(genesis_config.initial_state_root)
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
    moved_app::StateActor::on_tx_batch_in_memory()
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
    moved_app::StateActor::on_tx_in_memory()
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
    moved_app::StateActor::on_payload_in_memory()
}

pub fn transaction_repository() -> TransactionRepository {
    moved_blockchain::transaction::InMemoryTransactionRepository::new()
}

pub fn transaction_queries() -> TransactionQueries {
    moved_blockchain::transaction::InMemoryTransactionQueries::new()
}

pub fn receipt_repository() -> ReceiptRepository {
    moved_blockchain::receipt::InMemoryReceiptRepository::new()
}

pub fn receipt_queries() -> ReceiptQueries {
    moved_blockchain::receipt::InMemoryReceiptQueries::new()
}

pub fn receipt_memory() -> ReceiptStorage {
    moved_blockchain::receipt::ReceiptMemory::new()
}

pub fn block_queries() -> BlockQueries {
    moved_blockchain::block::InMemoryBlockQueries
}

pub fn payload_queries() -> PayloadQueries {
    moved_blockchain::payload::InMemoryPayloadQueries::new()
}

pub fn storage_trie_repository() -> StorageTrieRepository {
    moved_evm_ext::storage::InMemoryStorageTrieRepository::new()
}
