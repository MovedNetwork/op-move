#[cfg(any(feature = "test-doubles", test))]
pub use test_doubles::TestDependencies;

use {
    crate::mempool::Mempool, move_core_types::effects::ChangeSet,
    umi_blockchain::payload::PayloadId, umi_execution::resolver_cache::ResolverCache,
    umi_genesis::config::GenesisConfig, umi_shared::primitives::B256,
};

pub struct ApplicationReader<'app, D: Dependencies<'app>> {
    pub genesis_config: GenesisConfig,
    pub base_token: D::BaseTokenAccounts,
    pub block_queries: D::BlockQueries,
    pub block_hash_lookup: D::BlockHashLookup,
    pub payload_queries: D::PayloadQueries,
    pub receipt_queries: D::ReceiptQueries,
    pub receipt_memory: D::ReceiptStorageReader,
    pub storage: D::SharedStorageReader,
    pub state_queries: D::StateQueries,
    pub evm_storage: D::StorageTrieRepository,
    pub transaction_queries: D::TransactionQueries,
}

unsafe impl<'app, D: Dependencies<'app>> Sync for ApplicationReader<'app, D> {}

impl<'app, D: Dependencies<'app>> Clone for ApplicationReader<'app, D> {
    fn clone(&self) -> Self {
        Self {
            genesis_config: self.genesis_config.clone(),
            base_token: self.base_token.clone(),
            block_queries: self.block_queries.clone(),
            block_hash_lookup: self.block_hash_lookup.clone(),
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

impl<'app, D: Dependencies<'app>> ApplicationReader<'app, D> {
    pub fn new(deps: D, genesis_config: &GenesisConfig) -> Self {
        Self {
            genesis_config: genesis_config.clone(),
            base_token: D::base_token_accounts(genesis_config),
            block_hash_lookup: deps.block_hash_lookup(),
            block_queries: D::block_queries(),
            payload_queries: deps.payload_queries(),
            receipt_queries: D::receipt_queries(),
            receipt_memory: deps.receipt_memory_reader(),
            storage: deps.shared_storage_reader(),
            state_queries: deps.state_queries(genesis_config),
            evm_storage: deps.storage_trie_repository(),
            transaction_queries: D::transaction_queries(),
        }
    }
}

pub struct Application<'app, D: Dependencies<'app>> {
    pub genesis_config: GenesisConfig,
    pub mem_pool: Mempool,
    pub block_hash_writer: D::BlockHashWriter,
    pub block_hash_lookup: D::BlockHashLookup,
    pub gas_fee: D::BaseGasFee,
    pub base_token: D::BaseTokenAccounts,
    pub l1_fee: D::CreateL1GasFee,
    pub l2_fee: D::CreateL2GasFee,
    pub block_hash: D::BlockHash,
    pub block_queries: D::BlockQueries,
    pub block_repository: D::BlockRepository,
    pub on_payload: &'app D::OnPayload,
    pub on_tx: &'app D::OnTx,
    pub on_tx_batch: &'app D::OnTxBatch,
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
    // Note: the purpose of having the resolver cache here is to reuse the memory
    // allocated for the cache for all transaction execution instead of continuously
    // allocating and releasing memory.
    pub resolver_cache: ResolverCache,
}

impl<'app, D: Dependencies<'app>> Application<'app, D> {
    pub fn new(mut deps: D, genesis_config: &GenesisConfig) -> Self {
        Self {
            genesis_config: genesis_config.clone(),
            mem_pool: Mempool::default(),
            block_hash_writer: deps.block_hash_writer(),
            block_hash_lookup: deps.block_hash_lookup(),
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
            payload_queries: deps.payload_queries(),
            receipt_queries: D::receipt_queries(),
            receipt_repository: D::receipt_repository(),
            receipt_memory: deps.receipt_memory(),
            storage: deps.shared_storage(),
            receipt_memory_reader: deps.receipt_memory_reader(),
            storage_reader: deps.shared_storage_reader(),
            state: deps.state(),
            state_queries: deps.state_queries(genesis_config),
            evm_storage: deps.storage_trie_repository(),
            transaction_queries: D::transaction_queries(),
            transaction_repository: D::transaction_repository(),
            resolver_cache: ResolverCache::default(),
        }
    }

    pub fn on_tx(&mut self, changes: ChangeSet) {
        (self.on_tx)(self, changes)
    }
}

pub trait DependenciesThreadSafe<'db>:
    Dependencies<
        'db,
        BaseTokenAccounts: Send + 'static,
        BlockHash: Send + 'static,
        BlockHashLookup: Send + 'static,
        BlockHashWriter: Send + 'static,
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
    + 'db
{
}

impl<
    'app,
    T: Dependencies<
            'app,
            BaseTokenAccounts: Send + 'static,
            BlockHash: Send + 'static,
            BlockHashLookup: Send + 'static,
            BlockHashWriter: Send + 'static,
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
        + 'app,
> DependenciesThreadSafe<'app> for T
{
}

pub trait Dependencies<'db> {
    type BaseTokenAccounts: umi_execution::BaseTokenAccounts + Clone;
    type BlockHash: umi_blockchain::block::BlockHash;
    type BlockHashLookup: umi_evm_ext::state::BlockHashLookup + Clone;
    type BlockHashWriter: umi_evm_ext::state::BlockHashWriter + Clone;
    type BlockQueries: umi_blockchain::block::BlockQueries<Storage = Self::SharedStorageReader>
        + Clone;
    type BlockRepository: umi_blockchain::block::BlockRepository<Storage = Self::SharedStorage>;

    /// A function invoked on an execution of a new payload.
    type OnPayload: Fn(&mut Application<'db, Self>, PayloadId, B256) + 'db + ?Sized;

    /// A function invoked on an execution of a new transaction.
    type OnTx: Fn(&mut Application<'db, Self>, ChangeSet) + 'db + ?Sized;

    /// A function invoked on a completion of new transaction execution batch.
    type OnTxBatch: Fn(&mut Application<'db, Self>) + 'db + ?Sized;

    type PayloadQueries: umi_blockchain::payload::PayloadQueries<Storage = Self::SharedStorageReader>
        + Clone;
    type ReceiptQueries: umi_blockchain::receipt::ReceiptQueries<Storage = Self::ReceiptStorageReader>
        + Clone;
    type ReceiptRepository: umi_blockchain::receipt::ReceiptRepository<Storage = Self::ReceiptStorage>;
    type ReceiptStorage;
    type SharedStorage;
    type ReceiptStorageReader: Clone;
    type SharedStorageReader: Clone;
    type State: umi_state::State;
    type StateQueries: umi_blockchain::state::StateQueries + Clone;
    type StorageTrieRepository: umi_evm_ext::state::StorageTrieRepository + Clone;
    type TransactionQueries: umi_blockchain::transaction::TransactionQueries<Storage = Self::SharedStorageReader>
        + Clone;
    type TransactionRepository: umi_blockchain::transaction::TransactionRepository<Storage = Self::SharedStorage>;
    type BaseGasFee: umi_blockchain::block::BaseGasFee;
    type CreateL1GasFee: umi_execution::CreateL1GasFee;
    type CreateL2GasFee: umi_execution::CreateL2GasFee;

    fn base_token_accounts(genesis_config: &GenesisConfig) -> Self::BaseTokenAccounts;

    fn block_hash() -> Self::BlockHash;

    fn block_hash_lookup(&self) -> Self::BlockHashLookup;

    fn block_hash_writer(&self) -> Self::BlockHashWriter;

    fn block_queries() -> Self::BlockQueries;

    fn block_repository() -> Self::BlockRepository;

    fn on_payload() -> &'db Self::OnPayload;

    fn on_tx() -> &'db Self::OnTx;

    fn on_tx_batch() -> &'db Self::OnTxBatch;

    fn payload_queries(&self) -> Self::PayloadQueries;

    fn receipt_queries() -> Self::ReceiptQueries;

    fn receipt_repository() -> Self::ReceiptRepository;

    fn receipt_memory(&mut self) -> Self::ReceiptStorage;

    fn shared_storage(&mut self) -> Self::SharedStorage;

    fn receipt_memory_reader(&self) -> Self::ReceiptStorageReader;

    fn shared_storage_reader(&self) -> Self::SharedStorageReader;

    fn state(&self) -> Self::State;

    fn state_queries(&self, genesis_config: &GenesisConfig) -> Self::StateQueries;

    fn storage_trie_repository(&self) -> Self::StorageTrieRepository;

    fn transaction_queries() -> Self::TransactionQueries;

    fn transaction_repository() -> Self::TransactionRepository;

    fn base_gas_fee() -> Self::BaseGasFee;

    fn create_l1_gas_fee() -> Self::CreateL1GasFee;

    fn create_l2_gas_fee() -> Self::CreateL2GasFee;
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use {
        crate::{Application, Dependencies, SharedBlockHashCache},
        umi_blockchain::state::StateQueries,
        umi_genesis::config::GenesisConfig,
        umi_shared::primitives::U256,
        umi_state::State,
    };

    pub struct TestDependencies<
        SQ = umi_blockchain::state::InMemoryStateQueries,
        S = umi_state::InMemoryState,
        BT = umi_execution::UmiBaseTokenAccounts,
        BH = umi_blockchain::block::UmiBlockHash,
        BHL = SharedBlockHashCache,
        BHW = SharedBlockHashCache,
        BQ = umi_blockchain::block::InMemoryBlockQueries,
        BR = umi_blockchain::block::InMemoryBlockRepository,
        PQ = umi_blockchain::payload::InMemoryPayloadQueries,
        RQ = umi_blockchain::receipt::InMemoryReceiptQueries,
        RR = umi_blockchain::receipt::InMemoryReceiptRepository,
        R = umi_blockchain::receipt::ReceiptMemory,
        B = umi_blockchain::in_memory::SharedMemory,
        RMR = umi_blockchain::receipt::ReceiptMemoryReader,
        BMR = umi_blockchain::in_memory::SharedMemoryReader,
        ST = umi_evm_ext::state::InMemoryStorageTrieRepository,
        TQ = umi_blockchain::transaction::InMemoryTransactionQueries,
        TR = umi_blockchain::transaction::InMemoryTransactionRepository,
        BF = umi_blockchain::block::Eip1559GasFee,
        F1 = U256,
        F2 = U256,
    >(
        SQ,
        S,
        BT,
        BH,
        BHL,
        BHW,
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
        'app,
        SQ: StateQueries + Clone + Send + 'static,
        S: State + Send + 'static,
        BT: umi_execution::BaseTokenAccounts + Clone + Send + 'static,
        BH: umi_blockchain::block::BlockHash + Send + 'static,
        BHL: umi_evm_ext::state::BlockHashLookup + Clone + Send + 'static,
        BHW: umi_evm_ext::state::BlockHashWriter + Clone + Send + 'static,
        BQ: umi_blockchain::block::BlockQueries<Storage = BMR> + Clone + Send + 'static,
        BR: umi_blockchain::block::BlockRepository<Storage = B> + Send + 'static,
        PQ: umi_blockchain::payload::PayloadQueries<Storage = BMR> + Clone + Send + 'static,
        RQ: umi_blockchain::receipt::ReceiptQueries<Storage = RMR> + Clone + Send + 'static,
        RR: umi_blockchain::receipt::ReceiptRepository<Storage = R> + Send + 'static,
        R: Send + 'static,
        B: Send + 'static,
        RMR: Clone + Send + 'static,
        BMR: Clone + Send + 'static,
        ST: umi_evm_ext::state::StorageTrieRepository + Clone + Send + 'static,
        TQ: umi_blockchain::transaction::TransactionQueries<Storage = BMR> + Clone + Send + 'static,
        TR: umi_blockchain::transaction::TransactionRepository<Storage = B> + Send + 'static,
        BF: umi_blockchain::block::BaseGasFee + Send + 'static,
        F1: umi_execution::CreateL1GasFee + Send + 'static,
        F2: umi_execution::CreateL2GasFee + Send + 'static,
    > Dependencies<'app>
        for TestDependencies<
            SQ,
            S,
            BT,
            BH,
            BHL,
            BHW,
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
        type BlockHashLookup = BHL;
        type BlockHashWriter = BHW;
        type BlockQueries = BQ;
        type BlockRepository = BR;
        type OnPayload = crate::OnPayload<Application<'app, Self>>;
        type OnTx = crate::OnTx<Application<'app, Self>>;
        type OnTxBatch = crate::OnTxBatch<Application<'app, Self>>;
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

        fn block_hash_lookup(&self) -> Self::BlockHashLookup {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn block_hash_writer(&self) -> Self::BlockHashWriter {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn block_queries() -> Self::BlockQueries {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn block_repository() -> Self::BlockRepository {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn on_payload() -> &'app Self::OnPayload {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn on_tx() -> &'app Self::OnTx {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn on_tx_batch() -> &'app Self::OnTxBatch {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn payload_queries(&self) -> Self::PayloadQueries {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn receipt_queries() -> Self::ReceiptQueries {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn receipt_repository() -> Self::ReceiptRepository {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn receipt_memory(&mut self) -> Self::ReceiptStorage {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn shared_storage(&mut self) -> Self::SharedStorage {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn receipt_memory_reader(&self) -> Self::ReceiptStorageReader {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn shared_storage_reader(&self) -> Self::SharedStorageReader {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn state(&self) -> Self::State {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn state_queries(&self, _: &GenesisConfig) -> Self::StateQueries {
            unimplemented!("Dependencies are created manually in tests")
        }

        fn storage_trie_repository(&self) -> Self::StorageTrieRepository {
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
