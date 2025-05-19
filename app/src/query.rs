use {
    crate::{ApplicationReader, Dependencies, block_hash::StorageBasedProvider},
    alloy::{
        consensus::Header,
        eips::{
            BlockId,
            BlockNumberOrTag::{self, Earliest, Finalized, Latest, Number, Pending, Safe},
        },
        rpc::types::{FeeHistory, TransactionRequest},
    },
    moved_blockchain::{
        block::{BaseGasFee, BlockQueries, BlockResponse},
        payload::{PayloadId, PayloadQueries, PayloadResponse},
        receipt::{ReceiptQueries, TransactionReceipt},
        state::{CallResponse, ProofResponse, StateQueries},
        transaction::{TransactionQueries, TransactionResponse},
    },
    moved_shared::{
        error::{Error, UserError},
        primitives::{Address, B256, ToMoveAddress, U256},
    },
};

const MAX_PERCENTILE_COUNT: usize = 100;
const ZERO_HASH: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

#[derive(Debug)]
enum BlockNumberOrHash {
    Number(u64),
    Hash(B256),
}

impl<D: Dependencies> ApplicationReader<D> {
    pub fn chain_id(&self) -> u64 {
        self.genesis_config.chain_id
    }

    pub fn balance_by_height(&self, address: Address, height: BlockNumberOrTag) -> Option<U256> {
        self.state_queries.balance_at(
            &self.evm_storage,
            address.to_move_address(),
            self.resolve_height(height)?,
        )
    }

    pub fn nonce_by_height(&self, address: Address, height: BlockNumberOrTag) -> Option<u64> {
        self.state_queries.nonce_at(
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
        block_count: u64,
        block_number: BlockNumberOrTag,
        reward_percentiles: Option<Vec<f64>>,
    ) -> Result<FeeHistory, Error> {
        if block_count < 1 {
            return Err(Error::User(UserError::InvalidBlockCount));
        }
        // reward percentiles should be within (0..100) range and non-decreasing, up to a maximum
        // of 100 elements
        if let Some(reward) = &reward_percentiles {
            if reward.len() > MAX_PERCENTILE_COUNT {
                return Err(Error::User(UserError::RewardPercentilesTooLong));
            }
            if reward.iter().any(|p| !(0.0..=100.0).contains(p)) {
                return Err(Error::User(UserError::InvalidRewardPercentiles));
            }
            if reward.windows(2).any(|w| w[0] > w[1]) {
                return Err(Error::User(UserError::InvalidRewardPercentiles));
            }
        }

        let last_block = self
            .resolve_height(block_number)
            .ok_or(UserError::InvalidBlockHeight)?;

        let latest_block_num = self.block_number();
        // Genesis block is counted as 0
        let block_count = std::cmp::min(block_count, latest_block_num + 1);
        // As block count was clipped above,
        // saturating sub is technically not needed, but it's still better
        // to err on the safe side
        let oldest_block = (last_block + 1).saturating_sub(block_count);

        // base fees (and blob base fees) array should include the fee of the next block past the
        // end of the range as well
        let mut base_fees = Vec::with_capacity(block_count as usize + 1);
        let mut gas_used_ratio = Vec::with_capacity(block_count as usize);

        let total_reward: Option<Vec<Vec<u128>>>;

        match reward_percentiles {
            None => {
                total_reward = None;
                let mut current_block_num = last_block;
                let mut current_block_id = BlockNumberOrHash::Number(last_block);
                while current_block_num >= oldest_block {
                    let parent_hash = self.collect_fee_history_for_block(
                        &mut base_fees,
                        &mut gas_used_ratio,
                        &mut Vec::new(),
                        current_block_id,
                        |_, _, _| (),
                    );
                    if parent_hash == ZERO_HASH || current_block_num == 0 {
                        break;
                    }
                    current_block_id = BlockNumberOrHash::Hash(parent_hash);
                    current_block_num = current_block_num.saturating_sub(1);
                }
            }
            Some(percentiles) => {
                let mut inner_total_reward = Vec::with_capacity(block_count as usize);
                let mut current_block_num = last_block;
                let mut current_block_id = BlockNumberOrHash::Number(last_block);
                while current_block_num >= oldest_block {
                    let parent_hash = self.collect_fee_history_for_block(
                        &mut base_fees,
                        &mut gas_used_ratio,
                        &mut inner_total_reward,
                        current_block_id,
                        |total_reward, block_gas_used, price_and_cum_gas| {
                            total_reward.push(
                                percentiles
                                    .iter()
                                    .map(|p| {
                                        let threshold =
                                            ((block_gas_used as f64) * p / 100.0).round() as u64;
                                        price_and_cum_gas
                                            .iter()
                                            .find(|(_, cum_gas)| cum_gas >= &threshold)
                                            .or_else(|| price_and_cum_gas.last())
                                            .map(|(p, _)| p)
                                            .copied()
                                            .unwrap_or(0u128)
                                    })
                                    .collect::<Vec<_>>(),
                            )
                        },
                    );
                    if parent_hash == ZERO_HASH || current_block_num == 0 {
                        break;
                    }
                    current_block_id = BlockNumberOrHash::Hash(parent_hash);
                    current_block_num -= 1;
                }
                total_reward = Some(inner_total_reward);
            }
        }

        // EIP-4844 txs not supported yet
        let base_fee_per_blob_gas = vec![0u128; block_count as usize + 1];
        let blob_gas_used_ratio = vec![0f64; block_count as usize];

        Ok(FeeHistory {
            base_fee_per_gas: base_fees,
            gas_used_ratio,
            base_fee_per_blob_gas,
            blob_gas_used_ratio,
            oldest_block,
            reward: total_reward,
        })
    }

    pub fn estimate_gas(
        &self,
        transaction: TransactionRequest,
        block_number: BlockNumberOrTag,
    ) -> Result<u64> {
        let height = self.resolve_height(block_number).unwrap();
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
            &self.state_queries.resolver_at(height),
            &self.evm_storage,
            self.resolve_height(block_number)
                .ok_or(UserError::InvalidBlockHeight)?,
            transaction,
            &self.genesis_config,
            &self.base_token,
            &block_hash_lookup,
        )
    }

    pub fn call(
        &self,
        transaction: TransactionRequest,
        block_number: BlockNumberOrTag,
    ) -> Result<Vec<u8>> {
        let height = self.resolve_height(block_number).unwrap();
        let block_hash_lookup = StorageBasedProvider::new(&self.storage, &self.block_queries);
        call_transaction(
            transaction,
            &self.state_queries.resolver_at(height),
            &self.evm_storage,
            self.resolve_height(block_number)
                .ok_or(UserError::InvalidBlockHeight)?,
            transaction,
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

    // TODO: return a `Result` like geth does
    fn resolve_height(&self, height: BlockNumberOrTag) -> Option<u64> {
        self.block_queries
            .latest(&self.storage)
            .ok()?
            .and_then(|latest| match height {
                Number(height) if height <= latest => Some(height),
                Finalized | Pending | Latest | Safe => Some(latest),
                Earliest => Some(0),
                _ => None,
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

    fn collect_fee_history_for_block<F>(
        &self,
        base_fees: &mut Vec<u128>,
        gas_used_ratios: &mut Vec<f64>,
        total_reward: &mut Vec<Vec<u128>>,
        block_id: BlockNumberOrHash,
        mut acc_total_reward: F,
    ) -> B256
    where
        F: FnMut(&mut Vec<Vec<u128>>, u64, &[(u128, u64)]),
    {
        let curr_block = match block_id {
            BlockNumberOrHash::Number(height) => self
                .block_by_height(BlockNumberOrTag::Number(height), false)
                .unwrap(),
            BlockNumberOrHash::Hash(hash) => self.block_by_hash(hash, false).unwrap(),
        };
        let Header {
            gas_limit,
            gas_used: block_gas_used,
            base_fee_per_gas,
            parent_hash,
            ..
        } = curr_block.0.header.inner;

        base_fees.push(base_fee_per_gas.unwrap_or_default().into());

        // to account for weird edge cases in devnet/testnet environments, as defaulting
        // to 0.0 instead of NaN makes more sense
        let gas_used_ratio = if gas_limit == 0 {
            0.0
        } else {
            (block_gas_used as f64) / (gas_limit as f64)
        };
        gas_used_ratios.push(gas_used_ratio);

        let mut price_and_gas: Vec<(u128, u64)> = curr_block
            .0
            .transactions
            .into_hashes()
            .hashes()
            .map(|hash| {
                let rx = self
                    .transaction_receipt(hash)
                    .expect("Tx receipt should exist");
                (rx.inner.effective_gas_price, rx.inner.gas_used)
            })
            .collect();
        price_and_gas.sort_by_key(|&(price, _)| price);
        let price_and_cum_gas = price_and_gas
            .iter()
            .scan(0u64, |cum_gas, (price, gas)| {
                *cum_gas = (*cum_gas).saturating_add(*gas);
                Some((*price, *cum_gas))
            })
            .collect::<Vec<_>>();

        // For the last block, instead of querying block repo again, we resort to direct calculation
        // so that we also account for the range ending with the latest block
        if matches!(block_id, BlockNumberOrHash::Number(_)) {
            let next_block_base_fee = self
                .gas_fee
                .base_fee_per_gas(
                    gas_limit,
                    block_gas_used,
                    U256::from(base_fee_per_gas.unwrap_or_default()),
                )
                .saturating_to();
            base_fees.push(next_block_base_fee);
        }

        acc_total_reward(total_reward, block_gas_used, &price_and_cum_gas);
        parent_hash
    }
}
