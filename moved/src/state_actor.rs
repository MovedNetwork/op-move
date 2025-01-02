pub use {
    payload::{NewPayloadId, NewPayloadIdInput, StatePayloadId},
    queries::{
        Balance, BlockHeight, InMemoryStateQueries, Nonce, StateMemory, StateQueries, Version,
    },
};

use {
    crate::{
        block::{
            BaseGasFee, Block, BlockHash, BlockQueries, BlockRepository, ExtendedBlock, Header,
            HeaderForExecution,
        },
        genesis::config::GenesisConfig,
        move_execution::{
            execute_transaction,
            simulate::{call_transaction, simulate_transaction},
            BaseTokenAccounts, CreateL1GasFee, CreateL2GasFee, L1GasFee, L1GasFeeInput,
            L2GasFeeInput, LogsBloom,
        },
        primitives::{self, ToEthAddress, ToMoveAddress, ToSaturatedU64, B256, U256, U64},
        storage::State,
        types::{
            state::{
                Command, ExecutionOutcome, Payload, PayloadId, PayloadResponse, Query,
                StateMessage, ToPayloadIdInput, TransactionReceipt, TransactionWithReceipt,
                WithExecutionOutcome, WithPayloadAttributes,
            },
            transactions::{ExtendedTxEnvelope, NormalizedExtendedTxEnvelope},
        },
        Error::{InvalidTransaction, InvariantViolation, User},
    },
    alloy::{
        consensus::Receipt,
        eips::{
            eip2718::Encodable2718,
            BlockNumberOrTag::{self, *},
        },
        primitives::{keccak256, Bloom},
        rlp::{Decodable, Encodable},
        rpc::types::{FeeHistory, TransactionReceipt as AlloyTxReceipt},
    },
    move_binary_format::errors::PartialVMError,
    move_core_types::effects::ChangeSet,
    revm::primitives::TxKind,
    std::collections::HashMap,
    tokio::{sync::mpsc::Receiver, task::JoinHandle},
};

mod payload;
mod queries;

#[cfg(any(feature = "test-doubles", test))]
pub type InMemStateActor = StateActor<
    crate::storage::InMemoryState,
    u64,
    crate::block::MovedBlockHash,
    crate::block::InMemoryBlockRepository,
    crate::block::Eip1559GasFee,
    U256,
    U256,
    crate::move_execution::MovedBaseTokenAccounts,
    crate::block::InMemoryBlockQueries,
    crate::block::BlockMemory,
    InMemoryStateQueries,
>;

/// A function invoked on a completion of new transaction execution batch.
type OnTxBatch<S> =
    Box<dyn Fn() -> Box<dyn Fn(&mut S) + Send + Sync + 'static> + Send + Sync + 'static>;

/// A function invoked on an execution of a new transaction.
type OnTx<S> =
    Box<dyn Fn() -> Box<dyn Fn(&mut S, ChangeSet) + Send + Sync + 'static> + Send + Sync + 'static>;

pub struct StateActor<
    S: State,
    P: NewPayloadId,
    H: BlockHash,
    R: BlockRepository<Storage = M>,
    G: BaseGasFee,
    L1G: CreateL1GasFee,
    L2G: CreateL2GasFee,
    B: BaseTokenAccounts,
    Q: BlockQueries<Storage = M>,
    M,
    SQ,
> {
    genesis_config: GenesisConfig,
    rx: Receiver<StateMessage>,
    head: B256,
    height: u64,
    payload_id: P,
    block_hash: H,
    gas_fee: G,
    execution_payloads: HashMap<B256, PayloadResponse>,
    pending_payload: Option<(PayloadId, PayloadResponse)>,
    mem_pool: HashMap<B256, (ExtendedTxEnvelope, L1GasFeeInput)>,
    state: S,
    block_repository: R,
    block_queries: Q,
    l1_fee: L1G,
    l2_fee: L2G,
    base_token: B,
    block_memory: M,
    state_queries: SQ,
    // tx_hash -> (tx_with_receipt, block_hash)
    tx_receipts: HashMap<B256, (TransactionWithReceipt, B256)>,
    on_tx_batch: OnTxBatch<Self>,
    on_tx: OnTx<Self>,
}

impl<
        S: State<Err = PartialVMError> + Send + Sync + 'static,
        P: NewPayloadId + Send + Sync + 'static,
        H: BlockHash + Send + Sync + 'static,
        R: BlockRepository<Storage = M> + Send + Sync + 'static,
        G: BaseGasFee + Send + Sync + 'static,
        L1G: CreateL1GasFee + Send + Sync + 'static,
        L2G: CreateL2GasFee + Send + Sync + 'static,
        B: BaseTokenAccounts + Send + Sync + 'static,
        Q: BlockQueries<Storage = M> + Send + Sync + 'static,
        M: Send + Sync + 'static,
        SQ: StateQueries + Send + Sync + 'static,
    > StateActor<S, P, H, R, G, L1G, L2G, B, Q, M, SQ>
{
    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(msg) = self.rx.recv().await {
                match msg {
                    StateMessage::Command(msg) => self.handle_command(msg),
                    StateMessage::Query(msg) => self.handle_query(msg),
                };
            }
        })
    }
}

impl<
        S: State<Err = PartialVMError>,
        P: NewPayloadId,
        H: BlockHash,
        R: BlockRepository<Storage = M>,
        G: BaseGasFee,
        L1G: CreateL1GasFee,
        L2G: CreateL2GasFee,
        B: BaseTokenAccounts,
        Q: BlockQueries<Storage = M>,
        M,
        SQ: StateQueries,
    > StateActor<S, P, H, R, G, L1G, L2G, B, Q, M, SQ>
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rx: Receiver<StateMessage>,
        state: S,
        head: B256,
        height: u64,
        genesis_config: GenesisConfig,
        payload_id: P,
        block_hash: H,
        block_repository: R,
        base_fee_per_gas: G,
        l1_fee: L1G,
        l2_fee: L2G,
        base_token: B,
        block_queries: Q,
        block_memory: M,
        state_queries: SQ,
        on_tx: OnTx<Self>,
        on_tx_batch: OnTxBatch<Self>,
    ) -> Self {
        Self {
            genesis_config,
            rx,
            head,
            height,
            payload_id,
            execution_payloads: HashMap::new(),
            pending_payload: None,
            mem_pool: HashMap::new(),
            state,
            block_hash,
            block_repository,
            gas_fee: base_fee_per_gas,
            l1_fee,
            l2_fee,
            base_token,
            block_queries,
            block_memory,
            state_queries,
            tx_receipts: HashMap::new(),
            on_tx,
            on_tx_batch,
        }
    }

    pub fn resolve_height(&self, height: BlockNumberOrTag) -> u64 {
        match height {
            Number(height) => height,
            Finalized | Pending | Latest | Safe => self.height,
            Earliest => 0,
        }
    }

    pub fn handle_query(&self, msg: Query) {
        match msg {
            Query::ChainId { response_channel } => response_channel.send(self.genesis_config.chain_id).ok(),
            Query::BalanceByHeight {
                address,
                response_channel,
                height,
            } => response_channel
                .send(self.state_queries.balance_at(self.state.db(), address.to_move_address(), self.resolve_height(height)))
                .ok(),
            Query::NonceByHeight {
                address,
                response_channel,
                height,
            } => response_channel
                .send(self.state_queries.nonce_at(self.state.db(), address.to_move_address(), self.resolve_height(height)))
                .ok(),
            Query::BlockByHash {
                hash,
                response_channel,
                include_transactions,
            } => response_channel
                .send(self.block_queries.by_hash(&self.block_memory, hash, include_transactions))
                .ok(),
            Query::BlockByHeight {
                height,
                response_channel,
                include_transactions,
            } => response_channel
                .send(self.block_queries.by_height(&self.block_memory, self.resolve_height(height), include_transactions))
                .ok(),
            Query::BlockNumber {
                response_channel,
            } => response_channel
                .send(self.height)
                .ok(),
            Query::FeeHistory {
                response_channel,
                ..
                // TODO: Respond with a real fee history
            } => response_channel.send(FeeHistory::default()).ok(),
            Query::EstimateGas {
                transaction,
                block_number,
                response_channel,
            } => {
                // TODO: Support gas estimation from arbitrary blocks
                let block_height = match block_number {
                    Number(height) => height,
                    Finalized | Pending | Latest | Safe => self.height,
                    Earliest => 0,
                };
                // TODO: simulation should account for gas from non-zero L1 fee
                let outcome = simulate_transaction(transaction, self.state.resolver(), &self.genesis_config, &self.base_token, block_height);
                match outcome {
                    Ok(outcome) => response_channel.send(Ok(1000 * outcome.gas_used)).ok(),
                    Err(e) => response_channel.send(Err(e)).ok(),
                }
            }
            Query::Call {
                transaction,
                response_channel,
                ..
            } => {
                // TODO: Support transaction call from arbitrary blocks
                let outcome = call_transaction(transaction, self.state.resolver(), &self.genesis_config, &self.base_token);
                response_channel.send(outcome).ok()
            }
            Query::TransactionReceipt { tx_hash, response_channel } => {
                response_channel.send(self.query_transaction_receipt(tx_hash)).ok()
            }
        };
    }

    pub fn handle_command(&mut self, msg: Command) {
        match msg {
            Command::UpdateHead { block_hash } => {
                self.head = block_hash;
            }
            Command::StartBlockBuild {
                payload_attributes,
                response_channel,
            } => {
                let input = payload_attributes.to_payload_id_input(&self.head);
                let id = self.payload_id.new_payload_id(input);
                response_channel.send(id).ok();
                let block = self.create_block(payload_attributes);
                self.block_repository
                    .add(&mut self.block_memory, block.clone());
                self.height += 1;
                self.pending_payload
                    .replace((id, PayloadResponse::from_block(block)));
            }
            Command::GetPayload {
                id: request_id,
                response_channel,
            } => match self.pending_payload.take() {
                Some((id, payload)) => {
                    if request_id == id {
                        response_channel.send(Some(payload.clone())).ok();
                        self.execution_payloads
                            .insert(payload.execution_payload.block_hash, payload);
                    } else {
                        println!("WARN: unexpected PayloadId: {request_id}");
                        response_channel.send(None).ok();
                        self.pending_payload.replace((id, payload));
                    }
                }
                None => {
                    response_channel.send(None).ok();
                }
            },
            Command::GetPayloadByBlockHash {
                block_hash,
                response_channel,
            } => {
                let response = self.execution_payloads.get(&block_hash).cloned();
                response_channel.send(response).ok();
            }
            Command::AddTransaction { tx } => {
                let tx_hash = tx.tx_hash().0.into();
                let mut encoded = Vec::new();
                tx.encode(&mut encoded);
                let encoded = encoded.as_slice().into();
                self.mem_pool
                    .insert(tx_hash, (ExtendedTxEnvelope::Canonical(tx), encoded));
            }
            Command::GenesisUpdate { block } => {
                self.head = block.hash;
                self.block_repository.add(&mut self.block_memory, block);
            }
        }
    }

    fn create_block(&mut self, payload_attributes: Payload) -> ExtendedBlock {
        // Include transactions from both `payload_attributes` and internal mem-pool
        let transactions = payload_attributes
            .transactions
            .iter()
            .filter_map(|tx_bytes| {
                let mut slice: &[u8] = tx_bytes.as_ref();
                let tx_hash = B256::new(keccak256(slice).0);
                let tx = ExtendedTxEnvelope::decode(&mut slice)
                    .inspect_err(|_| {
                        println!("WARN: Failed to RLP decode transaction in payload_attributes")
                    })
                    .ok()?;

                Some((tx_hash, (tx, L1GasFeeInput::from(slice))))
            })
            .chain(self.mem_pool.drain())
            .collect::<Vec<_>>();
        let parent = self
            .block_repository
            .by_hash(&self.block_memory, self.head)
            .expect("Parent block should exist");
        let base_fee = self.gas_fee.base_fee_per_gas(
            parent.block.header.gas_limit,
            parent.block.header.gas_used,
            U256::from(parent.block.header.base_fee_per_gas.unwrap_or_default()),
        );

        let header_for_execution = HeaderForExecution {
            number: self.height + 1,
            timestamp: payload_attributes.timestamp.as_limbs()[0],
            prev_randao: payload_attributes.prev_randao,
        };
        let (execution_outcome, receipts) = self.execute_transactions(
            transactions
                .into_iter()
                .map(|(tx_hash, (tx, bytes))| (tx_hash, tx, bytes)),
            base_fee,
            &header_for_execution,
        );

        let transactions_root =
            alloy_trie::root::ordered_trie_root_with_encoder(&receipts, |rx, buf| {
                rx.tx.encode_2718(buf)
            });
        // TODO: is this the correct withdrawals root calculation?
        let withdrawals_root = alloy_trie::root::ordered_trie_root(&payload_attributes.withdrawals);
        let total_tip = execution_outcome.total_tip;

        let header = Header {
            parent_hash: self.head,
            number: header_for_execution.number,
            transactions_root,
            withdrawals_root: Some(withdrawals_root),
            base_fee_per_gas: Some(base_fee.saturating_to()),
            blob_gas_used: Some(0),
            excess_blob_gas: Some(0),
            ..Default::default()
        }
        .with_payload_attributes(payload_attributes)
        .with_execution_outcome(execution_outcome);

        let hash = self.block_hash.block_hash(&header);

        let transactions: Vec<_> = receipts
            .into_iter()
            .map(|v| {
                let tx = v.tx.clone();
                self.tx_receipts.insert(v.tx_hash, (v, hash));
                tx
            })
            .collect();

        Block::new(header, transactions)
            .with_hash(hash)
            .with_value(total_tip)
    }

    fn execute_transactions(
        &mut self,
        transactions: impl Iterator<Item = (B256, ExtendedTxEnvelope, L1GasFeeInput)>,
        base_fee: U256,
        block_header: &HeaderForExecution,
    ) -> (ExecutionOutcome, Vec<TransactionWithReceipt>) {
        let on_tx = (self.on_tx)();
        let mut total_tip = U256::ZERO;
        let mut receipts = Vec::new();
        let mut transactions = transactions.peekable();
        let mut cumulative_gas_used = 0u128;
        let mut logs_bloom = Bloom::ZERO;
        let mut tx_index = 0;
        let mut log_offset = 0;

        // https://github.com/ethereum-optimism/specs/blob/9dbc6b0/specs/protocol/deposits.md#kinds-of-deposited-transactions
        let l1_fee = transactions
            .peek()
            .and_then(|(_, v, _)| v.as_deposited())
            .map(|tx| self.l1_fee.for_deposit(tx.data.as_ref()));
        let l2_fee = self.l2_fee.with_gas_fee_multiplier(U256::from(1));

        // TODO: parallel transaction processing?
        for (tx_hash, tx, l1_cost_input) in transactions {
            let Ok(normalized_tx): Result<NormalizedExtendedTxEnvelope, _> = tx.clone().try_into()
            else {
                continue;
            };
            // TODO: implement gas limits etc. for `ExtendedTxEnvelope` so that
            // l2 gas inputs can be constructed at an earlier stage and stored in mempool
            let l2_gas_input = L2GasFeeInput::new(
                normalized_tx.gas_limit(),
                normalized_tx.effective_gas_price(base_fee),
            );
            let outcome = match execute_transaction(
                &normalized_tx,
                &tx_hash,
                self.state.resolver(),
                &self.genesis_config,
                l1_fee
                    .as_ref()
                    .map(|v| v.l1_fee(l1_cost_input.clone()).to_saturated_u64())
                    .unwrap_or(0),
                l2_fee.clone(),
                l2_gas_input,
                &self.base_token,
                block_header.clone(),
            ) {
                Ok(outcome) => outcome,
                Err(User(e)) => unreachable!("User errors are handled in execution {e:?}"),
                Err(InvalidTransaction(_)) => continue,
                Err(InvariantViolation(e)) => panic!("ERROR: execution error {e:?}"),
            };

            let l1_block_info = l1_fee.as_ref().and_then(|x| x.l1_block_info(l1_cost_input));

            on_tx(self, outcome.changes.clone());

            self.state
                .apply(outcome.changes)
                .unwrap_or_else(|_| panic!("ERROR: state update failed for transaction {tx:?}"));

            cumulative_gas_used = cumulative_gas_used.saturating_add(outcome.gas_used as u128);

            let bloom = outcome.logs.iter().logs_bloom();
            logs_bloom.accrue_bloom(&bloom);

            let tx_log_offset = log_offset;
            log_offset += outcome.logs.len() as u64;
            let receipt = Receipt {
                status: outcome.vm_outcome.is_ok().into(),
                cumulative_gas_used,
                logs: outcome.logs,
            };

            let receipt = tx.wrap_receipt(receipt, bloom);

            total_tip = total_tip.saturating_add(
                U256::from(outcome.gas_used).saturating_mul(normalized_tx.tip_per_gas(base_fee)),
            );

            receipts.push(TransactionWithReceipt {
                tx_hash,
                tx: tx.into(),
                normalized_tx,
                receipt,
                l1_block_info,
                gas_used: outcome.gas_used,
                l2_gas_price: outcome.l2_price,
                tx_index,
                contract_address: outcome
                    .deployment
                    .map(|(address, _)| address.to_eth_address()),
                logs_offset: tx_log_offset,
            });

            tx_index += 1;
        }

        let on_tx_batch = (self.on_tx_batch)();
        on_tx_batch(self);

        // Compute the receipts root by RLP-encoding each receipt to be a leaf of
        // a merkle trie.
        let receipts_root =
            alloy_trie::root::ordered_trie_root_with_encoder(&receipts, |rx, buf| {
                rx.receipt.encode(buf)
            });
        let logs_bloom = logs_bloom.into();

        let outcome = ExecutionOutcome {
            state_root: self.state.state_root(),
            gas_used: U64::from(cumulative_gas_used),
            receipts_root,
            logs_bloom,
            total_tip,
        };
        (outcome, receipts)
    }

    fn query_transaction_receipt(&self, tx_hash: B256) -> Option<TransactionReceipt> {
        let (rx, block_hash) = self.tx_receipts.get(&tx_hash)?;
        let block = self
            .block_queries
            .by_hash(&self.block_memory, *block_hash, false)?;
        let contract_address = rx.contract_address;
        let (to, from) = match &rx.normalized_tx {
            NormalizedExtendedTxEnvelope::Canonical(tx) => {
                let to = match tx.to {
                    TxKind::Call(to) => Some(to),
                    TxKind::Create => None,
                };
                (to, tx.signer)
            }
            NormalizedExtendedTxEnvelope::DepositedTx(tx) => (Some(tx.to), tx.from),
        };
        let logs = rx
            .receipt
            .logs()
            .iter()
            .enumerate()
            .map(|(internal_index, log)| alloy::rpc::types::Log {
                inner: log.clone(),
                block_hash: Some(*block_hash),
                block_number: Some(block.0.header.number),
                block_timestamp: Some(block.0.header.timestamp),
                transaction_hash: Some(tx_hash),
                transaction_index: Some(rx.tx_index),
                log_index: Some(rx.logs_offset + (internal_index as u64)),
                removed: false,
            })
            .collect();
        let receipt = primitives::with_rpc_logs(&rx.receipt, logs);
        let result = TransactionReceipt {
            inner: AlloyTxReceipt {
                inner: receipt,
                transaction_hash: tx_hash,
                transaction_index: Some(rx.tx_index),
                block_hash: Some(*block_hash),
                block_number: Some(block.0.header.number),
                gas_used: rx.gas_used as u128,
                // TODO: make all gas prices bounded by u128?
                effective_gas_price: rx.l2_gas_price.saturating_to(),
                // Always None because we do not support eip-4844 transactions
                blob_gas_used: None,
                blob_gas_price: None,
                from,
                to,
                contract_address,
                // EIP-7702 not yet supported
                authorization_list: None,
            },
            l1_block_info: rx.l1_block_info.unwrap_or_default(),
        };
        Some(result)
    }

    pub fn on_tx_batch_noop() -> OnTxBatch<Self> {
        Box::new(|| Box::new(|_| {}))
    }

    pub fn on_tx_noop() -> OnTx<Self> {
        Box::new(|| Box::new(|_, _| {}))
    }
}

impl<
        S: State<Err = PartialVMError>,
        P: NewPayloadId,
        H: BlockHash,
        R: BlockRepository<Storage = M>,
        G: BaseGasFee,
        L1G: CreateL1GasFee,
        L2G: CreateL2GasFee,
        B: BaseTokenAccounts,
        Q: BlockQueries<Storage = M>,
        M,
    > StateActor<S, P, H, R, G, L1G, L2G, B, Q, M, InMemoryStateQueries>
{
    pub fn on_tx_in_memory() -> OnTx<Self> {
        Box::new(|| Box::new(|state, changes| state.state_queries.add(changes)))
    }

    pub fn on_tx_batch_in_memory() -> OnTxBatch<Self> {
        Box::new(|| Box::new(|state| state.state_queries.tag()))
    }
}

#[cfg(any(feature = "test-doubles", test))]
pub use test_doubles::*;

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use {
        super::*, crate::primitives::U256, aptos_jellyfish_merkle::TreeReader,
        aptos_types::state_store::state_key::StateKey,
        move_core_types::account_address::AccountAddress,
    };

    pub struct MockStateQueries(pub AccountAddress, pub BlockHeight);

    impl StateQueries for MockStateQueries {
        type Storage = ();

        fn balance_at(
            &self,
            _db: &(impl TreeReader<StateKey> + Sync),
            account: AccountAddress,
            height: BlockHeight,
        ) -> Option<Balance> {
            assert_eq!(account, self.0);
            assert_eq!(height, self.1);

            Some(U256::from(5))
        }

        fn nonce_at(
            &self,
            _db: &(impl TreeReader<StateKey> + Sync),
            account: AccountAddress,
            height: BlockHeight,
        ) -> Option<Nonce> {
            assert_eq!(account, self.0);
            assert_eq!(height, self.1);

            Some(3)
        }
    }
}

#[test]
fn test_compute_transactions_root() {
    use alloy::{hex, primitives::address};

    let tx = op_alloy::consensus::TxDeposit {
        source_hash: B256::new(hex!("d019f0c65ad46edd487015d96b177006cf35364a870d32fb3f517165f61d9d46")),
        from: address!("deaddeaddeaddeaddeaddeaddeaddeaddead0001"),
        to: address!("4200000000000000000000000000000000000015").into(),
        mint: None,
        value: U256::ZERO,
        gas_limit: 0xf4240,
        is_system_transaction: false,
        input: hex!("440a5e2000022950000c5f4f000000000000000000000000674de72100000000000000210000000000000000000000000000000000000000000000000000000000bd330300000000000000000000000000000000000000000000000000000000000000013f93a2bd37b737d88517db273b0797a0ef98a5c145aed05cd5d227321fc156580000000000000000000000008c67a7b8624044f8f672e9ec374dfa596f01afb9").into(),
    };

    let txs: [op_alloy::consensus::OpTxEnvelope; 1] = [tx.into()];
    let transactions_root =
        alloy_trie::root::ordered_trie_root_with_encoder(&txs, |tx, buf| tx.encode_2718(buf));

    assert_eq!(
        transactions_root,
        B256::new(hex!(
            "90e7a8d12f001569a72bfae8ec3b108c72342f9e8aa824658b974b4f4c0cc640"
        ))
    );
}

#[test]
fn test_build_block_hash() {
    use alloy::{hex, primitives::address};

    let payload_attributes = Payload {
        timestamp: U64::from(0x6759e370_u64),
        prev_randao: B256::new(hex!(
            "ade920edae8d7bb10146e7baae162b5d5d8902c5a2a4e9309d0bf197e7fdf9b6"
        )),
        suggested_fee_recipient: address!("4200000000000000000000000000000000000011"),
        withdrawals: Vec::new(),
        parent_beacon_block_root: Default::default(),
        transactions: Vec::new(),
        gas_limit: U64::from(0x1c9c380),
    };

    let execution_outcome = ExecutionOutcome {
        receipts_root: B256::new(hex!(
            "3c55e3bccc48ee3ee637d8fc6936e4825d1489cbebf6057ce8025d63755ebf54"
        )),
        state_root: B256::new(hex!(
            "5affa0c563587bc4668feaea28e997d29961e864be20b0082d123bcb2fbbaf55"
        )),
        logs_bloom: Default::default(),
        gas_used: U64::from(0x272a2),
        total_tip: Default::default(),
    };

    let header = Header {
        parent_hash: B256::new(hex!(
            "966c80cc0cbf7dbf7a2b2579002b95c8756f388c3fbf4a77c4d94d3719880c6e"
        )),
        number: 1,
        transactions_root: B256::new(hex!(
            "c355179c91ebb544d6662d6ad580c45eb3f155e1626b693b3afa4fdca677c450"
        )),
        base_fee_per_gas: Some(0x3b5dc100),
        blob_gas_used: Some(0),
        excess_blob_gas: Some(0),
        withdrawals_root: Some(B256::new(hex!(
            "56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"
        ))),
        ..Default::default()
    }
    .with_payload_attributes(payload_attributes)
    .with_execution_outcome(execution_outcome);

    let hash = crate::block::MovedBlockHash.block_hash(&header);
    assert_eq!(
        hash,
        B256::new(hex!(
            "c9f7a6ef5311bf49b8322a92f3d75bd5c505ee613323fb58c7166c3511a62bcf"
        ))
    );
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            block::{
                BlockMemory, Eip1559GasFee, InMemoryBlockQueries, InMemoryBlockRepository,
                MovedBlockHash,
            },
            genesis::{self, config::CHAIN_ID},
            move_execution::{create_move_vm, create_vm_session, MovedBaseTokenAccounts},
            storage::InMemoryState,
            tests::{signer::Signer, EVM_ADDRESS, PRIVATE_KEY},
            types::session_id::SessionId,
        },
        alloy::{
            consensus::{SignableTransaction, TxEip1559, TxEnvelope},
            hex,
            network::TxSignerSync,
        },
        move_core_types::{account_address::AccountAddress, effects::ChangeSet},
        move_vm_runtime::module_traversal::{TraversalContext, TraversalStorage},
        move_vm_types::gas::UnmeteredGasMeter,
        test_case::test_case,
        tokio::sync::{
            mpsc::{self, Sender},
            oneshot,
        },
    };

    fn create_state_actor_with_given_queries<SQ: StateQueries>(
        height: u64,
        state_queries: SQ,
    ) -> (
        StateActor<
            impl State<Err = PartialVMError>,
            impl NewPayloadId,
            impl BlockHash,
            impl BlockRepository<Storage = BlockMemory>,
            impl BaseGasFee,
            impl CreateL1GasFee,
            impl CreateL2GasFee,
            impl BaseTokenAccounts,
            impl BlockQueries<Storage = BlockMemory>,
            BlockMemory,
            SQ,
        >,
        Sender<StateMessage>,
    ) {
        let genesis_config = GenesisConfig::default();
        let (state_channel, rx) = mpsc::channel(10);

        let head_hash = B256::new(hex!(
            "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
        ));
        let genesis_block = Block::default().with_hash(head_hash).with_value(U256::ZERO);

        let mut block_memory = BlockMemory::new();
        let mut repository = InMemoryBlockRepository::new();
        repository.add(&mut block_memory, genesis_block);

        let mut state = InMemoryState::new();
        genesis::init_and_apply(&genesis_config, &mut state);

        let state = StateActor::new(
            rx,
            state,
            head_hash,
            height,
            genesis_config,
            0x03421ee50df45cacu64,
            MovedBlockHash,
            repository,
            Eip1559GasFee::default(),
            U256::ZERO,
            U256::ZERO,
            MovedBaseTokenAccounts::new(AccountAddress::ONE),
            InMemoryBlockQueries,
            block_memory,
            state_queries,
            StateActor::on_tx_noop(),
            StateActor::on_tx_batch_noop(),
        );
        (state, state_channel)
    }

    fn mint_eth(
        state: &impl State<Err = PartialVMError>,
        addr: AccountAddress,
        amount: U256,
    ) -> ChangeSet {
        let move_vm = create_move_vm().unwrap();
        let mut session = create_vm_session(&move_vm, state.resolver(), SessionId::default());
        let traversal_storage = TraversalStorage::new();
        let mut traversal_context = TraversalContext::new(&traversal_storage);
        let mut gas_meter = UnmeteredGasMeter;

        crate::move_execution::mint_eth(
            &addr,
            amount,
            &mut session,
            &mut traversal_context,
            &mut gas_meter,
        )
        .unwrap();

        session.finish().unwrap()
    }

    fn create_state_actor_with_fake_queries(
        addr: AccountAddress,
        initial_balance: U256,
    ) -> (
        StateActor<
            impl State<Err = PartialVMError>,
            impl NewPayloadId,
            impl BlockHash,
            impl BlockRepository<Storage = BlockMemory>,
            impl BaseGasFee,
            impl CreateL1GasFee,
            impl CreateL2GasFee,
            impl BaseTokenAccounts,
            impl BlockQueries<Storage = BlockMemory>,
            BlockMemory,
            impl StateQueries,
        >,
        Sender<StateMessage>,
    ) {
        let genesis_config = GenesisConfig::default();
        let (state_channel, rx) = mpsc::channel(10);

        let head_hash = B256::new(hex!(
            "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
        ));
        let height = 0;
        let genesis_block = Block::default().with_hash(head_hash).with_value(U256::ZERO);

        let mut block_memory = BlockMemory::new();
        let mut repository = InMemoryBlockRepository::new();
        repository.add(&mut block_memory, genesis_block);

        let mut state = InMemoryState::new();
        let (genesis_changes, table_changes) = genesis::init(&genesis_config, &state);
        state
            .apply_with_tables(genesis_changes.clone(), table_changes)
            .unwrap();
        let changes_addition = mint_eth(&state, addr, initial_balance);
        state.apply(changes_addition.clone()).unwrap();

        let mut state_queries = InMemoryStateQueries::from_genesis(genesis_changes);
        state_queries.add(changes_addition);

        let state = StateActor::new(
            rx,
            state,
            head_hash,
            height,
            genesis_config,
            0x03421ee50df45cacu64,
            MovedBlockHash,
            repository,
            Eip1559GasFee::default(),
            U256::ZERO,
            U256::ZERO,
            MovedBaseTokenAccounts::new(AccountAddress::ONE),
            InMemoryBlockQueries,
            block_memory,
            state_queries,
            StateActor::on_tx_in_memory(),
            StateActor::on_tx_batch_in_memory(),
        );
        (state, state_channel)
    }

    #[test_case(Latest, 4, 4; "Latest")]
    #[test_case(Finalized, 4, 4; "Finalized")]
    #[test_case(Safe, 4, 4; "Safe")]
    #[test_case(Earliest, 4, 0; "Earliest")]
    #[test_case(Pending, 4, 4; "Pending")]
    #[test_case(Number(2), 4, 2; "Number")]
    fn test_nonce_is_fetched_by_height_successfully(
        height: BlockNumberOrTag,
        head_height: BlockHeight,
        expected_height: BlockHeight,
    ) {
        let address = primitives::Address::new(hex!("11223344556677889900ffeeaabbccddee111111"));
        let (state_actor, _) = create_state_actor_with_given_queries(
            head_height,
            MockStateQueries(address.to_move_address(), expected_height),
        );
        let (tx, rx) = oneshot::channel();

        state_actor.handle_query(Query::NonceByHeight {
            height,
            address,
            response_channel: tx,
        });

        let actual_nonce = rx.blocking_recv().unwrap().expect("Block should be found");
        let expected_nonce = 3;

        assert_eq!(actual_nonce, expected_nonce);
    }

    #[test_case(Latest, 2, 2; "Latest")]
    #[test_case(Finalized, 2, 2; "Finalized")]
    #[test_case(Safe, 2, 2; "Safe")]
    #[test_case(Earliest, 2, 0; "Earliest")]
    #[test_case(Pending, 2, 2; "Pending")]
    #[test_case(Number(1), 2, 1; "Number")]
    fn test_balance_is_fetched_by_height_successfully(
        height: BlockNumberOrTag,
        head_height: BlockHeight,
        expected_height: BlockHeight,
    ) {
        let address = primitives::Address::new(hex!("44223344556677889900ffeeaabbccddee111111"));
        let (state_actor, _) = create_state_actor_with_given_queries(
            head_height,
            MockStateQueries(address.to_move_address(), expected_height),
        );
        let (tx, rx) = oneshot::channel();

        state_actor.handle_query(Query::BalanceByHeight {
            height,
            address,
            response_channel: tx,
        });

        let actual_balance = rx.blocking_recv().unwrap().expect("Block should be found");
        let expected_balance = U256::from(5);

        assert_eq!(actual_balance, expected_balance);
    }

    #[test]
    fn test_fetched_balances_are_updated_after_transfer_of_funds() {
        let to = primitives::Address::new(hex!("44223344556677889900ffeeaabbccddee111111"));
        let initial_balance = U256::from(5);
        let amount = U256::from(4);
        let (mut state_actor, _) =
            create_state_actor_with_fake_queries(EVM_ADDRESS.to_move_address(), initial_balance);

        let signer = Signer::new(&PRIVATE_KEY);
        let mut tx = TxEip1559 {
            chain_id: CHAIN_ID,
            nonce: signer.nonce,
            gas_limit: u64::MAX,
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            to: TxKind::Call(to),
            value: amount,
            access_list: Default::default(),
            input: Default::default(),
        };
        let signature = signer.inner.sign_transaction_sync(&mut tx).unwrap();
        let tx = TxEnvelope::Eip1559(tx.into_signed(signature));

        state_actor.handle_command(Command::AddTransaction { tx: tx.clone() });
        state_actor.handle_command(Command::StartBlockBuild {
            payload_attributes: Default::default(),
            response_channel: oneshot::channel().0,
        });

        let (tx, rx) = oneshot::channel();

        state_actor.handle_query(Query::BalanceByHeight {
            height: Latest,
            address: to,
            response_channel: tx,
        });

        let actual_recipient_balance = rx.blocking_recv().unwrap().expect("Block should be found");
        let expected_recipient_balance = amount;

        assert_eq!(actual_recipient_balance, expected_recipient_balance);

        let (tx, rx) = oneshot::channel();

        state_actor.handle_query(Query::BalanceByHeight {
            height: Latest,
            address: EVM_ADDRESS,
            response_channel: tx,
        });

        let actual_sender_balance = rx.blocking_recv().unwrap().expect("Block should be found");
        let expected_sender_balance = initial_balance - amount;

        assert_eq!(actual_sender_balance, expected_sender_balance);
    }

    #[test]
    fn test_fetched_nonces_are_updated_after_executing_transaction() {
        let to = primitives::Address::new(hex!("44223344556677889900ffeeaabbccddee111111"));
        let initial_balance = U256::from(5);
        let amount = U256::from(4);
        let (mut state_actor, _) =
            create_state_actor_with_fake_queries(EVM_ADDRESS.to_move_address(), initial_balance);

        let signer = Signer::new(&PRIVATE_KEY);
        let mut tx = TxEip1559 {
            chain_id: CHAIN_ID,
            nonce: signer.nonce,
            gas_limit: u64::MAX,
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            to: TxKind::Call(to),
            value: amount,
            access_list: Default::default(),
            input: Default::default(),
        };
        let signature = signer.inner.sign_transaction_sync(&mut tx).unwrap();
        let tx = TxEnvelope::Eip1559(tx.into_signed(signature));

        state_actor.handle_command(Command::AddTransaction { tx });

        let (tx, _rx) = oneshot::channel();

        state_actor.handle_command(Command::StartBlockBuild {
            payload_attributes: Default::default(),
            response_channel: tx,
        });

        let (tx, rx) = oneshot::channel();

        state_actor.handle_query(Query::NonceByHeight {
            height: Latest,
            address: to,
            response_channel: tx,
        });

        let actual_recipient_balance = rx.blocking_recv().unwrap().expect("Block should be found");
        let expected_recipient_balance = 0;

        assert_eq!(actual_recipient_balance, expected_recipient_balance);

        let (tx, rx) = oneshot::channel();

        state_actor.handle_query(Query::NonceByHeight {
            height: Latest,
            address: EVM_ADDRESS,
            response_channel: tx,
        });

        let actual_sender_balance = rx.blocking_recv().unwrap().expect("Block should be found");
        let expected_sender_balance = 1;

        assert_eq!(actual_sender_balance, expected_sender_balance);
    }
}
