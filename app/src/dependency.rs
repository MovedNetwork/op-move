#[cfg(any(feature = "test-doubles", test))]
pub use test_doubles::TestDependencies;

use {
    move_core_types::effects::ChangeSet, moved_blockchain::payload::PayloadId,
    moved_execution::L1GasFeeInput, moved_genesis::config::GenesisConfig,
    moved_shared::primitives::B256, op_alloy::consensus::OpTxEnvelope, std::collections::HashMap,
};

pub struct ApplicationReader<D: Dependencies> {
    pub genesis_config: GenesisConfig,
    pub base_token: D::BaseTokenAccounts,
    pub block_queries: D::BlockQueries,
    pub payload_queries: D::PayloadQueries,
    pub receipt_queries: D::ReceiptQueries,
    pub receipt_memory: D::ReceiptStorageReader,
    pub storage: D::SharedStorageReader,
    pub state_queries: D::StateQueries,
    pub evm_storage: D::StorageTrieRepository,
    pub transaction_queries: D::TransactionQueries,
}

unsafe impl<D: Dependencies> Sync for ApplicationReader<D> {}

impl<D: Dependencies> Clone for ApplicationReader<D> {
    fn clone(&self) -> Self {
        Self {
            genesis_config: self.genesis_config.clone(),
            base_token: self.base_token.clone(),
            block_queries: self.block_queries.clone(),
            payload_queries: self.payload_queries.clone(),
            receipt_queries: self.receipt_queries.clone(),
            receipt_memory: self.receipt_memory.clone(),
            storage: self.storage.clone(),
            state_queries: self.state_queries.clone(),
            evm_storage: self.evm_storage.clone(),
            transaction_queries: self.transaction_queries.clone(),
        }
    }
}

impl<D: Dependencies> ApplicationReader<D> {
    pub fn new(_: D, genesis_config: &GenesisConfig) -> Self {
        Self {
            genesis_config: genesis_config.clone(),
            base_token: D::base_token_accounts(genesis_config),
            block_queries: D::block_queries(),
            payload_queries: D::payload_queries(),
            receipt_queries: D::receipt_queries(),
            receipt_memory: D::receipt_memory_reader(),
            storage: D::shared_storage_reader(),
            state_queries: D::state_queries(genesis_config),
            evm_storage: D::storage_trie_repository(),
            transaction_queries: D::transaction_queries(),
        }
    }
}

pub struct Application<D: Dependencies> {
    pub genesis_config: GenesisConfig,
    pub mem_pool: HashMap<B256, (OpTxEnvelope, L1GasFeeInput)>,
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
    pub receipt_memory_reader: D::ReceiptStorageReader,
    pub storage_reader: D::SharedStorageReader,
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
            receipt_memory_reader: D::receipt_memory_reader(),
            storage_reader: D::shared_storage_reader(),
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
        BaseTokenAccounts: Send + 'static,
        BlockHash: Send + 'static,
        BlockQueries: Send + 'static,
        BlockRepository: Send + 'static,
        OnPayload: Send + Sync + 'static,
        OnTx: Send + Sync + 'static,
        OnTxBatch: Send + Sync + 'static,
        PayloadQueries: Send + 'static,
        ReceiptQueries: Send + 'static,
        ReceiptRepository: Send + 'static,
        ReceiptStorage: Send + 'static,
        SharedStorage: Send + 'static,
        ReceiptStorageReader: Send + 'static,
        SharedStorageReader: Send + 'static,
        State: Send + 'static,
        StateQueries: Send + 'static,
        StorageTrieRepository: Send + 'static,
        TransactionQueries: Send + 'static,
        TransactionRepository: Send + 'static,
        BaseGasFee: Send + 'static,
        CreateL1GasFee: Send + 'static,
        CreateL2GasFee: Send + 'static,
    > + Send
    + 'static
{
}

impl<
    T: Dependencies<
            BaseTokenAccounts: Send + 'static,
            BlockHash: Send + 'static,
            BlockQueries: Send + 'static,
            BlockRepository: Send + 'static,
            OnPayload: Send + Sync + 'static,
            OnTx: Send + Sync + 'static,
            OnTxBatch: Send + Sync + 'static,
            PayloadQueries: Send + 'static,
            ReceiptQueries: Send + 'static,
            ReceiptRepository: Send + 'static,
            ReceiptStorage: Send + 'static,
            SharedStorage: Send + 'static,
            ReceiptStorageReader: Send + 'static,
            SharedStorageReader: Send + 'static,
            State: Send + 'static,
            StateQueries: Send + 'static,
            StorageTrieRepository: Send + 'static,
            TransactionQueries: Send + 'static,
            TransactionRepository: Send + 'static,
            BaseGasFee: Send + 'static,
            CreateL1GasFee: Send + 'static,
            CreateL2GasFee: Send + 'static,
        > + Send
        + 'static,
> DependenciesThreadSafe for T
{
}

pub trait Dependencies {
    type BaseTokenAccounts: moved_execution::BaseTokenAccounts + Clone;
    type BlockHash: moved_blockchain::block::BlockHash;
    type BlockQueries: moved_blockchain::block::BlockQueries<Storage = Self::SharedStorageReader>
        + Clone;
    type BlockRepository: moved_blockchain::block::BlockRepository<Storage = Self::SharedStorage>;

    /// A function invoked on an execution of a new payload.
    type OnPayload: Fn(&mut Application<Self>, PayloadId, B256) + 'static + ?Sized;

    /// A function invoked on an execution of a new transaction.
    type OnTx: Fn(&mut Application<Self>, ChangeSet) + 'static + ?Sized;

    /// A function invoked on a completion of new transaction execution batch.
    type OnTxBatch: Fn(&mut Application<Self>) + 'static + ?Sized;

    type PayloadQueries: moved_blockchain::payload::PayloadQueries<Storage = Self::SharedStorageReader>
        + Clone;
    type ReceiptQueries: moved_blockchain::receipt::ReceiptQueries<Storage = Self::ReceiptStorageReader>
        + Clone;
    type ReceiptRepository: moved_blockchain::receipt::ReceiptRepository<Storage = Self::ReceiptStorage>;
    type ReceiptStorage;
    type SharedStorage;
    type ReceiptStorageReader: Clone;
    type SharedStorageReader: Clone;
    type State: moved_state::State;
    type StateQueries: moved_blockchain::state::StateQueries + Clone;
    type StorageTrieRepository: moved_evm_ext::state::StorageTrieRepository + Clone;
    type TransactionQueries: moved_blockchain::transaction::TransactionQueries<Storage = Self::SharedStorageReader>
        + Clone;
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

    fn receipt_memory_reader() -> Self::ReceiptStorageReader;

    fn shared_storage_reader() -> Self::SharedStorageReader;

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
        RMR = moved_blockchain::receipt::ReceiptMemoryReader,
        BMR = moved_blockchain::in_memory::SharedMemoryReader,
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
        RMR,
        BMR,
        ST,
        TQ,
        TR,
        BF,
        F1,
        F2,
    );

    impl<
        SQ: StateQueries + Clone + Send + 'static,
        S: State + Send + 'static,
        BT: moved_execution::BaseTokenAccounts + Clone + Send + 'static,
        BH: moved_blockchain::block::BlockHash + Send + 'static,
        BQ: moved_blockchain::block::BlockQueries<Storage = BMR> + Clone + Send + 'static,
        BR: moved_blockchain::block::BlockRepository<Storage = B> + Send + 'static,
        PQ: moved_blockchain::payload::PayloadQueries<Storage = BMR> + Clone + Send + 'static,
        RQ: moved_blockchain::receipt::ReceiptQueries<Storage = RMR> + Clone + Send + 'static,
        RR: moved_blockchain::receipt::ReceiptRepository<Storage = R> + Send + 'static,
        R: Send + 'static,
        B: Send + 'static,
        RMR: Clone + Send + 'static,
        BMR: Clone + Send + 'static,
        ST: moved_evm_ext::state::StorageTrieRepository + Clone + Send + 'static,
        TQ: moved_blockchain::transaction::TransactionQueries<Storage = BMR> + Clone + Send + 'static,
        TR: moved_blockchain::transaction::TransactionRepository<Storage = B> + Send + 'static,
        BF: moved_blockchain::block::BaseGasFee + Send + 'static,
        F1: moved_execution::CreateL1GasFee + Send + 'static,
        F2: moved_execution::CreateL2GasFee + Send + 'static,
    > Dependencies
        for TestDependencies<
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
            RMR,
            BMR,
            ST,
            TQ,
            TR,
            BF,
            F1,
            F2,
        >
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
        type ReceiptStorageReader = RMR;
        type SharedStorageReader = BMR;
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

        fn receipt_memory_reader() -> Self::ReceiptStorageReader {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn shared_storage_reader() -> Self::SharedStorageReader {
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
