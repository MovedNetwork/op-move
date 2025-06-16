use {
    crate::{Application, Dependencies},
    std::sync::Arc,
    umi_blockchain::{block::Eip1559GasFee, state::EthTrieStateQueries},
    umi_evm_ext::state::InMemoryDb,
    umi_execution::U256,
    umi_genesis::config::GenesisConfig,
    umi_shared::primitives::B256,
    umi_state::InMemoryState,
};

/// A set of non-operational dependencies that can be used to satisfy a parameter list.
pub struct Uninitialized;

impl Dependencies for Uninitialized {
    type BaseTokenAccounts = ();
    type BlockHash = B256;
    type BlockQueries = ();
    type BlockHashLookup = ();
    type BlockHashWriter = ();
    type BlockRepository = ();
    type OnPayload = crate::OnPayload<Application<Self>>;
    type OnTx = crate::OnTx<Application<Self>>;
    type OnTxBatch = crate::OnTxBatch<Application<Self>>;
    type PayloadQueries = ();
    type ReceiptQueries = ();
    type ReceiptRepository = ();
    type ReceiptStorage = ();
    type SharedStorage = ();
    type ReceiptStorageReader = ();
    type SharedStorageReader = ();
    type State = InMemoryState;
    type StateQueries = EthTrieStateQueries<Vec<B256>, InMemoryDb>;
    type StorageTrieRepository = ();
    type TransactionQueries = ();
    type TransactionRepository = ();
    type BaseGasFee = Eip1559GasFee;
    type CreateL1GasFee = U256;
    type CreateL2GasFee = U256;

    fn base_token_accounts(_genesis_config: &GenesisConfig) -> Self::BaseTokenAccounts {}

    fn block_hash() -> Self::BlockHash {
        B256::ZERO
    }

    fn block_queries() -> Self::BlockQueries {}

    fn block_hash_lookup(&self) -> Self::BlockHashLookup {}

    fn block_hash_writer(&self) -> Self::BlockHashWriter {}

    fn block_repository() -> Self::BlockRepository {}

    fn on_payload() -> &'static Self::OnPayload {
        &|_, _, _| {}
    }

    fn on_tx() -> &'static Self::OnTx {
        &|_, _| {}
    }

    fn on_tx_batch() -> &'static Self::OnTxBatch {
        &|_| {}
    }

    fn payload_queries() -> Self::PayloadQueries {}

    fn receipt_queries() -> Self::ReceiptQueries {}

    fn receipt_repository() -> Self::ReceiptRepository {}

    fn receipt_memory(&mut self) -> Self::ReceiptStorage {}

    fn shared_storage(&mut self) -> Self::SharedStorage {}

    fn receipt_memory_reader(&self) -> Self::ReceiptStorageReader {}

    fn shared_storage_reader(&self) -> Self::SharedStorageReader {}

    fn state(&self) -> Self::State {
        InMemoryState::default()
    }

    fn state_queries(&self, genesis_config: &GenesisConfig) -> Self::StateQueries {
        EthTrieStateQueries::new(
            vec![genesis_config.initial_state_root],
            Arc::new(InMemoryDb::empty()),
            genesis_config.initial_state_root,
        )
    }

    fn storage_trie_repository(&self) -> Self::StorageTrieRepository {}

    fn transaction_queries() -> Self::TransactionQueries {}

    fn transaction_repository() -> Self::TransactionRepository {}

    fn base_gas_fee() -> Self::BaseGasFee {
        Eip1559GasFee::default()
    }

    fn create_l1_gas_fee() -> Self::CreateL1GasFee {
        U256::ZERO
    }

    fn create_l2_gas_fee() -> Self::CreateL2GasFee {
        U256::ZERO
    }
}
