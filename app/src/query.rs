use {
    crate::{Application, ApplicationReader, Dependencies, block_hash::StorageBasedProvider},
    alloy::{
        eips::{
            BlockId,
            BlockNumberOrTag::{self, Earliest, Finalized, Latest, Number, Pending, Safe},
        },
        rpc::types::{FeeHistory, TransactionRequest},
    },
    moved_blockchain::{
        block::{BlockQueries, BlockResponse},
        payload::{PayloadId, PayloadQueries, PayloadResponse},
        receipt::{ReceiptQueries, TransactionReceipt},
        state::{ProofResponse, StateQueries},
        transaction::{TransactionQueries, TransactionResponse},
    },
    moved_execution::simulate::{call_transaction, simulate_transaction},
    moved_shared::{
        error::Result,
        primitives::{Address, B256, ToMoveAddress, U256},
    },
    moved_state::State,
};

impl<D: Dependencies> ApplicationReader<D> {
    pub fn chain_id(&self) -> u64 {
        self.genesis_config.chain_id
    }

    pub fn balance_by_height(&self, address: Address, height: BlockNumberOrTag) -> Option<U256> {
        self.state_queries.balance_at(
            self.state.db(),
            &self.evm_storage,
            address.to_move_address(),
            self.resolve_height(height)?,
        )
    }

    pub fn nonce_by_height(&self, address: Address, height: BlockNumberOrTag) -> Option<u64> {
        self.state_queries.nonce_at(
            self.state.db(),
            &self.evm_storage,
            address.to_move_address(),
            self.resolve_height(height)?,
        )
    }

    pub fn block_by_hash(&self, hash: B256, include_transactions: bool) -> Option<BlockResponse> {
        self.block_queries
            .by_hash(&self.storage, hash, include_transactions)
            .unwrap()
    }

    pub fn block_by_height(
        &self,
        height: BlockNumberOrTag,
        include_transactions: bool,
    ) -> Option<BlockResponse> {
        self.block_queries
            .by_height(
                &self.storage,
                self.resolve_height(height)?,
                include_transactions,
            )
            .unwrap()
    }

    pub fn block_number(&self) -> u64 {
        self.block_queries.latest(&self.storage).unwrap().unwrap()
    }

    pub fn fee_history(
        &self,
        _block_count: u64,
        _block_number: BlockNumberOrTag,
        _reward_percentiles: Option<Vec<f64>>,
    ) -> FeeHistory {
        // TODO: Respond with a real fee history
        FeeHistory::default()
    }

    pub fn estimate_gas(
        &self,
        transaction: TransactionRequest,
        block_number: BlockNumberOrTag,
    ) -> Result<u64> {
        // TODO: Support gas estimation from arbitrary blocks
        let block_height = match block_number {
            Number(height) => height,
            Finalized | Pending | Latest | Safe => self
                .block_queries
                .latest(&self.storage)
                .unwrap()
                .expect("Blocks should be non-empty"),
            Earliest => 0,
        };
        let block_hash_lookup = StorageBasedProvider::new(&self.storage, &self.block_queries);
        let outcome = simulate_transaction(
            transaction,
            self.state.resolver(),
            &self.evm_storage,
            &self.genesis_config,
            &self.base_token,
            block_height,
            &block_hash_lookup,
        );

        outcome.map(|outcome| {
            // Add 33% extra gas as a buffer.
            outcome.gas_used + (outcome.gas_used / 3)
        })
    }

    pub fn call(
        &self,
        transaction: TransactionRequest,
        _block_number: BlockNumberOrTag,
    ) -> Result<Vec<u8>> {
        // TODO: Support transaction call from arbitrary blocks
        let block_hash_lookup = StorageBasedProvider::new(&self.storage, &self.block_queries);
        call_transaction(
            transaction,
            self.state.resolver(),
            &self.evm_storage,
            &self.genesis_config,
            &self.base_token,
            &block_hash_lookup,
        )
    }

    pub fn transaction_receipt(&self, tx_hash: B256) -> Option<TransactionReceipt> {
        self.receipt_queries
            .by_transaction_hash(&self.receipt_memory, tx_hash)
            .unwrap()
    }

    pub fn transaction_by_hash(&self, tx_hash: B256) -> Option<TransactionResponse> {
        self.transaction_queries
            .by_hash(&self.storage, tx_hash)
            .ok()
            .flatten()
    }

    pub fn proof(
        &self,
        address: Address,
        storage_slots: Vec<U256>,
        height: BlockId,
    ) -> Option<ProofResponse> {
        self.height_from_block_id(height).and_then(|height| {
            self.state_queries.proof_at(
                self.state.db(),
                &self.evm_storage,
                address.to_move_address(),
                &storage_slots,
                height,
            )
        })
    }

    pub fn payload(&self, id: PayloadId) -> Option<PayloadResponse> {
        self.payload_queries.by_id(&self.storage, id).ok().flatten()
    }

    pub fn payload_by_block_hash(&self, block_hash: B256) -> Option<PayloadResponse> {
        self.payload_queries
            .by_hash(&self.storage, block_hash)
            .ok()
            .flatten()
    }

    fn resolve_height(&self, height: BlockNumberOrTag) -> Option<u64> {
        Some(match height {
            Number(height) => height,
            Finalized | Pending | Latest | Safe => {
                self.block_queries.latest(&self.storage).ok()??
            }
            Earliest => 0,
        })
    }

    fn height_from_block_id(&self, id: BlockId) -> Option<u64> {
        Some(match id {
            BlockId::Number(height) => self.resolve_height(height)?,
            BlockId::Hash(h) => {
                self.block_queries
                    .by_hash(&self.storage, h.block_hash, false)
                    .ok()??
                    .0
                    .header
                    .number
            }
        })
    }
}
