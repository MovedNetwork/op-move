use std::collections::HashMap;
#[cfg(any(feature = "test-doubles", test))]
pub use test_doubles::TestDependencies;

use {
    move_core_types::effects::ChangeSet,
    moved_blockchain::payload::PayloadId,
    moved_execution::{L1GasFeeInput, transaction::ExtendedTxEnvelope},
    moved_genesis::config::GenesisConfig,
    moved_shared::primitives::B256,
};

pub struct Application<D: Dependencies> {
    pub genesis_config: GenesisConfig,
    pub head: B256,
    pub height: u64,
    pub mem_pool: HashMap<B256, (ExtendedTxEnvelope, L1GasFeeInput)>,
    pub gas_fee: D::BaseGasFee,
    pub base_token: D::BaseTokenAccounts,
    pub l1_fee: D::CreateL1GasFee,
    pub l2_fee: D::CreateL2GasFee,
    pub block_hash: D::BlockHash,
    pub block_queries: D::BlockQueries,
    pub block_repository: D::BlockRepository,
    pub on_payload: &'static D::OnPayload,
    pub on_tx: &'static D::OnTx,
    pub on_tx_batch: &'static D::OnTxBatch,
    pub payload_queries: D::PayloadQueries,
    pub receipt_queries: D::ReceiptQueries,
    pub receipt_repository: D::ReceiptRepository,
    pub receipt_memory: D::ReceiptStorage,
    pub storage: D::SharedStorage,
    pub state: D::State,
    pub state_queries: D::StateQueries,
    pub evm_storage: D::StorageTrieRepository,
    pub transaction_queries: D::TransactionQueries,
    pub transaction_repository: D::TransactionRepository,
}

impl<D: Dependencies> Application<D> {
    pub fn new(_: D, genesis_config: &GenesisConfig) -> Self {
        Self {
            genesis_config: genesis_config.clone(),
            head: Default::default(),
            height: 0,
            mem_pool: Default::default(),
            gas_fee: D::base_gas_fee(),
            base_token: D::base_token_accounts(genesis_config),
            l1_fee: D::create_l1_gas_fee(),
            l2_fee: D::create_l2_gas_fee(),
            block_hash: D::block_hash(),
            block_queries: D::block_queries(),
            block_repository: D::block_repository(),
            on_payload: D::on_payload(),
            on_tx: D::on_tx(),
            on_tx_batch: D::on_tx_batch(),
            payload_queries: D::payload_queries(),
            receipt_queries: D::receipt_queries(),
            receipt_repository: D::receipt_repository(),
            receipt_memory: D::receipt_memory(),
            storage: D::shared_storage(),
            state: D::state(),
            state_queries: D::state_queries(genesis_config),
            evm_storage: D::storage_trie_repository(),
            transaction_queries: D::transaction_queries(),
            transaction_repository: D::transaction_repository(),
        }
    }

    pub fn on_tx(&mut self, changes: ChangeSet) {
        (self.on_tx)(self, changes)
    }
}

pub trait DependenciesThreadSafe:
    Dependencies<
        BaseTokenAccounts: Send + Sync + 'static,
        BlockHash: Send + Sync + 'static,
        BlockQueries: Send + Sync + 'static,
        BlockRepository: Send + Sync + 'static,
        OnPayload: Send + Sync + 'static,
        OnTx: Send + Sync + 'static,
        OnTxBatch: Send + Sync + 'static,
        PayloadQueries: Send + Sync + 'static,
        ReceiptQueries: Send + Sync + 'static,
        ReceiptRepository: Send + Sync + 'static,
        ReceiptStorage: Send + Sync + 'static,
        SharedStorage: Send + Sync + 'static,
        State: Send + Sync + 'static,
        StateQueries: Send + Sync + 'static,
        StorageTrieRepository: Send + Sync + 'static,
        TransactionQueries: Send + Sync + 'static,
        TransactionRepository: Send + Sync + 'static,
        BaseGasFee: Send + Sync + 'static,
        CreateL1GasFee: Send + Sync + 'static,
        CreateL2GasFee: Send + Sync + 'static,
    > + Send
    + Sync
    + 'static
{
}

impl<
    T: Dependencies<
            BaseTokenAccounts: Send + Sync + 'static,
            BlockHash: Send + Sync + 'static,
            BlockQueries: Send + Sync + 'static,
            BlockRepository: Send + Sync + 'static,
            OnPayload: Send + Sync + 'static,
            OnTx: Send + Sync + 'static,
            OnTxBatch: Send + Sync + 'static,
            PayloadQueries: Send + Sync + 'static,
            ReceiptQueries: Send + Sync + 'static,
            ReceiptRepository: Send + Sync + 'static,
            ReceiptStorage: Send + Sync + 'static,
            SharedStorage: Send + Sync + 'static,
            State: Send + Sync + 'static,
            StateQueries: Send + Sync + 'static,
            StorageTrieRepository: Send + Sync + 'static,
            TransactionQueries: Send + Sync + 'static,
            TransactionRepository: Send + Sync + 'static,
            BaseGasFee: Send + Sync + 'static,
            CreateL1GasFee: Send + Sync + 'static,
            CreateL2GasFee: Send + Sync + 'static,
        > + Send
        + Sync
        + 'static,
> DependenciesThreadSafe for T
{
}

pub trait Dependencies: Sized {
    type BaseTokenAccounts: moved_execution::BaseTokenAccounts;
    type BlockHash: moved_blockchain::block::BlockHash;
    type BlockQueries: moved_blockchain::block::BlockQueries<Storage = Self::SharedStorage>;
    type BlockRepository: moved_blockchain::block::BlockRepository<Storage = Self::SharedStorage>;

    /// A function invoked on an execution of a new payload.
    type OnPayload: Fn(&mut Application<Self>, PayloadId, B256) + 'static + ?Sized;

    /// A function invoked on an execution of a new transaction.
    type OnTx: Fn(&mut Application<Self>, ChangeSet) + 'static + ?Sized;

    /// A function invoked on a completion of new transaction execution batch.
    type OnTxBatch: Fn(&mut Application<Self>) + 'static + ?Sized;

    type PayloadQueries: moved_blockchain::payload::PayloadQueries<Storage = Self::SharedStorage>;
    type ReceiptQueries: moved_blockchain::receipt::ReceiptQueries<Storage = Self::ReceiptStorage>;
    type ReceiptRepository: moved_blockchain::receipt::ReceiptRepository<Storage = Self::ReceiptStorage>;
    type ReceiptStorage;
    type SharedStorage;
    type State: moved_state::State;
    type StateQueries: moved_blockchain::state::StateQueries;
    type StorageTrieRepository: moved_evm_ext::state::StorageTrieRepository;
    type TransactionQueries: moved_blockchain::transaction::TransactionQueries<Storage = Self::SharedStorage>;
    type TransactionRepository: moved_blockchain::transaction::TransactionRepository<Storage = Self::SharedStorage>;
    type BaseGasFee: moved_blockchain::block::BaseGasFee;
    type CreateL1GasFee: moved_execution::CreateL1GasFee;
    type CreateL2GasFee: moved_execution::CreateL2GasFee;

    fn base_token_accounts(genesis_config: &GenesisConfig) -> Self::BaseTokenAccounts;

    fn block_hash() -> Self::BlockHash;

    fn block_queries() -> Self::BlockQueries;

    fn block_repository() -> Self::BlockRepository;

    fn on_payload() -> &'static Self::OnPayload;

    fn on_tx() -> &'static Self::OnTx;

    fn on_tx_batch() -> &'static Self::OnTxBatch;

    fn payload_queries() -> Self::PayloadQueries;

    fn receipt_queries() -> Self::ReceiptQueries;

    fn receipt_repository() -> Self::ReceiptRepository;

    fn receipt_memory() -> Self::ReceiptStorage;

    fn shared_storage() -> Self::SharedStorage;

    fn state() -> Self::State;

    fn state_queries(genesis_config: &GenesisConfig) -> Self::StateQueries;

    fn storage_trie_repository() -> Self::StorageTrieRepository;

    fn transaction_queries() -> Self::TransactionQueries;

    fn transaction_repository() -> Self::TransactionRepository;

    fn base_gas_fee() -> Self::BaseGasFee;

    fn create_l1_gas_fee() -> Self::CreateL1GasFee;

    fn create_l2_gas_fee() -> Self::CreateL2GasFee;
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use {
        crate::{Application, Dependencies},
        moved_blockchain::state::StateQueries,
        moved_genesis::config::GenesisConfig,
        moved_shared::primitives::U256,
        moved_state::State,
    };

    pub struct TestDependencies<
        SQ = moved_blockchain::state::InMemoryStateQueries,
        S = moved_state::InMemoryState,
        BT = moved_execution::MovedBaseTokenAccounts,
        BH = moved_blockchain::block::MovedBlockHash,
        BQ = moved_blockchain::block::InMemoryBlockQueries,
        BR = moved_blockchain::block::InMemoryBlockRepository,
        PQ = moved_blockchain::payload::InMemoryPayloadQueries,
        RQ = moved_blockchain::receipt::InMemoryReceiptQueries,
        RR = moved_blockchain::receipt::InMemoryReceiptRepository,
        R = moved_blockchain::receipt::ReceiptMemory,
        B = moved_blockchain::in_memory::SharedMemory,
        ST = moved_evm_ext::state::InMemoryStorageTrieRepository,
        TQ = moved_blockchain::transaction::InMemoryTransactionQueries,
        TR = moved_blockchain::transaction::InMemoryTransactionRepository,
        BF = moved_blockchain::block::Eip1559GasFee,
        F1 = U256,
        F2 = U256,
    >(
        SQ,
        S,
        BT,
        BH,
        BQ,
        BR,
        PQ,
        RQ,
        RR,
        R,
        B,
        ST,
        TQ,
        TR,
        BF,
        F1,
        F2,
    );

    impl<
        SQ: StateQueries + Send + Sync + 'static,
        S: State + Send + Sync + 'static,
        BT: moved_execution::BaseTokenAccounts + Send + Sync + 'static,
        BH: moved_blockchain::block::BlockHash + Send + Sync + 'static,
        BQ: moved_blockchain::block::BlockQueries<Storage = B> + Send + Sync + 'static,
        BR: moved_blockchain::block::BlockRepository<Storage = B> + Send + Sync + 'static,
        PQ: moved_blockchain::payload::PayloadQueries<Storage = B> + Send + Sync + 'static,
        RQ: moved_blockchain::receipt::ReceiptQueries<Storage = R> + Send + Sync + 'static,
        RR: moved_blockchain::receipt::ReceiptRepository<Storage = R> + Send + Sync + 'static,
        R: Send + Sync + 'static,
        B: Send + Sync + 'static,
        ST: moved_evm_ext::state::StorageTrieRepository + Send + Sync + 'static,
        TQ: moved_blockchain::transaction::TransactionQueries<Storage = B> + Send + Sync + 'static,
        TR: moved_blockchain::transaction::TransactionRepository<Storage = B> + Send + Sync + 'static,
        BF: moved_blockchain::block::BaseGasFee + Send + Sync + 'static,
        F1: moved_execution::CreateL1GasFee + Send + Sync + 'static,
        F2: moved_execution::CreateL2GasFee + Send + Sync + 'static,
    > Dependencies
        for TestDependencies<SQ, S, BT, BH, BQ, BR, PQ, RQ, RR, R, B, ST, TQ, TR, BF, F1, F2>
    {
        type BaseTokenAccounts = BT;
        type BlockHash = BH;
        type BlockQueries = BQ;
        type BlockRepository = BR;
        type OnPayload = crate::OnPayload<Application<Self>>;
        type OnTx = crate::OnTx<Application<Self>>;
        type OnTxBatch = crate::OnTxBatch<Application<Self>>;
        type PayloadQueries = PQ;
        type ReceiptQueries = RQ;
        type ReceiptRepository = RR;
        type ReceiptStorage = R;
        type SharedStorage = B;
        type State = S;
        type StateQueries = SQ;
        type StorageTrieRepository = ST;
        type TransactionQueries = TQ;
        type TransactionRepository = TR;
        type BaseGasFee = BF;
        type CreateL1GasFee = F1;
        type CreateL2GasFee = F2;

        fn base_token_accounts(_: &GenesisConfig) -> Self::BaseTokenAccounts {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn block_hash() -> Self::BlockHash {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn block_queries() -> Self::BlockQueries {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn block_repository() -> Self::BlockRepository {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn on_payload() -> &'static Self::OnPayload {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn on_tx() -> &'static Self::OnTx {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn on_tx_batch() -> &'static Self::OnTxBatch {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn payload_queries() -> Self::PayloadQueries {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn receipt_queries() -> Self::ReceiptQueries {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn receipt_repository() -> Self::ReceiptRepository {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn receipt_memory() -> Self::ReceiptStorage {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn shared_storage() -> Self::SharedStorage {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn state() -> Self::State {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn state_queries(_: &GenesisConfig) -> Self::StateQueries {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn storage_trie_repository() -> Self::StorageTrieRepository {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn transaction_queries() -> Self::TransactionQueries {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn transaction_repository() -> Self::TransactionRepository {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn base_gas_fee() -> Self::BaseGasFee {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn create_l1_gas_fee() -> Self::CreateL1GasFee {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn create_l2_gas_fee() -> Self::CreateL2GasFee {
            unimplemented!("Dependencies are created manually in tests")
        }
    }
}
