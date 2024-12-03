pub use payload::{NewPayloadId, NewPayloadIdInput, StatePayloadId};

use {
    crate::{
        block::{
            Block, BlockHash, BlockQueries, BlockRepository, ExtendedBlock, GasFee, Header,
            HeaderForExecution,
        },
        genesis::config::GenesisConfig,
        merkle_tree::MerkleRootExt,
        move_execution::{
            execute_transaction, quick_get_nonce, BaseTokenAccounts, CreateL1GasFee, L1GasFee,
            L1GasFeeInput, LogsBloom, StateQueries,
        },
        primitives::{self, ToEthAddress, ToMoveAddress, ToSaturatedU64, B256, U256, U64},
        storage::State,
        types::{
            state::{
                Command, ExecutionOutcome, Payload, PayloadId, PayloadResponse, Query,
                StateMessage, ToPayloadIdInput, TransactionReceipt, TransactionWithReceipt,
                WithExecutionOutcome, WithPayloadAttributes,
            },
            transactions::{
                ExtendedTxEnvelope, NormalizedEthTransaction, NormalizedExtendedTxEnvelope,
                TransactionExecutionOutcome,
            },
        },
        Error::{InvalidTransaction, InvariantViolation, User},
    },
    alloy::{
        consensus::Receipt,
        eips::{BlockNumberOrTag, BlockNumberOrTag::*},
        primitives::{keccak256, Bloom},
        rlp::{Decodable, Encodable},
        rpc::types::{FeeHistory, TransactionReceipt as AlloyTxReceipt, TransactionRequest},
    },
    move_binary_format::errors::PartialVMError,
    revm::primitives::TxKind,
    std::{
        collections::HashMap,
        time::{SystemTime, UNIX_EPOCH},
    },
    tokio::{sync::mpsc::Receiver, task::JoinHandle},
};

mod payload;

#[cfg(any(feature = "test-doubles", test))]
pub type InMemStateActor = StateActor<
    crate::storage::InMemoryState,
    u64,
    crate::block::MovedBlockHash,
    crate::block::InMemoryBlockRepository,
    crate::block::Eip1559GasFee,
    U256,
    crate::move_execution::MovedBaseTokenAccounts,
    crate::block::InMemoryBlockQueries,
    crate::block::BlockMemory,
    crate::move_execution::InMemoryStateQueries,
>;

#[derive(Debug)]
pub struct StateActor<
    S: State,
    P: NewPayloadId,
    H: BlockHash,
    R: BlockRepository<Storage = M>,
    G: GasFee,
    L1G: CreateL1GasFee,
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
    base_token: B,
    block_memory: M,
    state_queries: SQ,
    // tx_hash -> (tx_with_receipt, block_hash)
    tx_receipts: HashMap<B256, (TransactionWithReceipt, B256)>,
}

impl<
        S: State<Err = PartialVMError> + Send + Sync + 'static,
        P: NewPayloadId + Send + Sync + 'static,
        H: BlockHash + Send + Sync + 'static,
        R: BlockRepository<Storage = M> + Send + Sync + 'static,
        G: GasFee + Send + Sync + 'static,
        L1G: CreateL1GasFee + Send + Sync + 'static,
        B: BaseTokenAccounts + Send + Sync + 'static,
        Q: BlockQueries<Storage = M> + Send + Sync + 'static,
        M: Send + Sync + 'static,
        SQ: StateQueries + Send + Sync + 'static,
    > StateActor<S, P, H, R, G, L1G, B, Q, M, SQ>
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
        G: GasFee,
        L1G: CreateL1GasFee,
        B: BaseTokenAccounts,
        Q: BlockQueries<Storage = M>,
        M,
        SQ: StateQueries,
    > StateActor<S, P, H, R, G, L1G, B, Q, M, SQ>
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
        base_token: B,
        block_queries: Q,
        block_memory: M,
        state_queries: SQ,
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
            base_token,
            block_queries,
            block_memory,
            state_queries,
            tx_receipts: HashMap::new(),
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
                .send(self.state_queries.balance_at(address.to_move_address(), self.resolve_height(height)))
                .ok(),
            Query::NonceByHeight {
                address,
                response_channel,
                height,
            } => response_channel
                .send(self.state_queries.nonce_at(address.to_move_address(), self.resolve_height(height)))
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
                response_channel,
                ..
            } => {
                // TODO: Support gas estimation from arbitrary blocks
                let outcome = self.simulate_transaction(transaction);
                match outcome {
                    Ok(outcome) => response_channel.send(Ok(outcome.gas_used)).ok(),
                    Err(e) => response_channel.send(Err(e)).ok(),
                }
            }
            Query::Call {
                response_channel,
                ..
                // TODO: Respond with a real transaction call result
            } => response_channel.send(vec![]).ok(),
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
                self.pending_payload.replace((id, block.into()));
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
            parent.block.header.base_fee_per_gas,
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

        let transactions_root = receipts
            .iter()
            .map(|v| alloy::rlp::encode(&v.tx))
            .map(keccak256)
            .merkle_root();
        let total_tip = execution_outcome.total_tip;
        // TODO: Compute `withdrawals_root`
        let header = Header::new(self.head, header_for_execution.number)
            .with_payload_attributes(payload_attributes)
            .with_execution_outcome(execution_outcome)
            .with_transactions_root(transactions_root)
            .with_base_fee_per_gas(base_fee);

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
            .map(|v| self.l1_fee.for_deposit(v.data.as_ref()));

        // TODO: parallel transaction processing?
        for (tx_hash, tx, l1_cost_input) in transactions {
            let Ok(normalized_tx) = tx.clone().try_into() else {
                continue;
            };
            let outcome = match execute_transaction(
                &normalized_tx,
                &tx_hash,
                self.state.resolver(),
                &self.genesis_config,
                l1_fee
                    .as_ref()
                    .map(|v| v.l1_fee(l1_cost_input.clone()).to_saturated_u64())
                    .unwrap_or(0),
                &self.base_token,
                block_header.clone(),
            ) {
                Ok(outcome) => outcome,
                Err(User(_)) => unreachable!("User errors are handled in execution"),
                Err(InvalidTransaction(_)) => continue,
                Err(InvariantViolation(e)) => panic!("ERROR: execution error {e:?}"),
            };

            let l1_block_info = l1_fee.as_ref().and_then(|x| x.l1_block_info(l1_cost_input));

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
                tx,
                normalized_tx,
                receipt,
                l1_block_info,
                gas_used: outcome.gas_used,
                tx_index,
                contract_address: outcome
                    .deployment
                    .map(|(address, _)| address.to_eth_address()),
                logs_offset: tx_log_offset,
            });

            tx_index += 1;
        }

        let receipts_root = receipts
            .iter()
            .map(|v| alloy::rlp::encode(&v.receipt))
            .map(keccak256)
            .merkle_root();
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

    fn simulate_transaction(
        &self,
        request: TransactionRequest,
    ) -> crate::Result<TransactionExecutionOutcome> {
        let mut tx = NormalizedEthTransaction::from(request.clone());
        if request.from.is_some() && request.nonce.is_none() {
            tx.nonce = quick_get_nonce(&tx.signer.to_move_address(), self.state.resolver());
        }
        let tx = NormalizedExtendedTxEnvelope::Canonical(tx);

        let block_header = HeaderForExecution {
            number: self.height + 1,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Should get current time")
                .as_secs(),
            prev_randao: B256::random(),
        };

        execute_transaction(
            &tx,
            &B256::random(),
            self.state.resolver(),
            &self.genesis_config,
            0,
            &self.base_token,
            block_header,
        )
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
                // TODO: charge for gas
                effective_gas_price: 0,
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
            genesis,
            move_execution::{BlockHeight, MovedBaseTokenAccounts},
            storage::InMemoryState,
        },
        alloy::hex,
        move_core_types::account_address::AccountAddress,
        test_case::test_case,
        tokio::sync::{
            mpsc::{self, Sender},
            oneshot,
        },
    };

    struct MockStateQueries(BlockHeight);

    impl StateQueries for MockStateQueries {
        type Storage = ();

        fn balance(&self, _account: AccountAddress) -> crate::move_execution::Balance {
            unimplemented!()
        }

        fn balance_at(
            &self,
            account: AccountAddress,
            height: BlockHeight,
        ) -> crate::move_execution::Balance {
            assert_eq!(height, self.0);
            assert_eq!(
                account,
                primitives::Address::new(hex!("44223344556677889900ffeeaabbccddee111111"))
                    .to_move_address()
            );

            U256::from(5)
        }

        fn nonce(&self, _account: AccountAddress) -> crate::move_execution::Nonce {
            unimplemented!()
        }

        fn nonce_at(
            &self,
            account: AccountAddress,
            height: BlockHeight,
        ) -> crate::move_execution::Nonce {
            assert_eq!(height, self.0);
            assert_eq!(
                account,
                primitives::Address::new(hex!("11223344556677889900ffeeaabbccddee111111"))
                    .to_move_address()
            );

            3
        }
    }

    pub fn create_state_actor<SQ: StateQueries>(
        height: u64,
        state_queries: SQ,
    ) -> (
        StateActor<
            impl State<Err = PartialVMError>,
            impl NewPayloadId,
            impl BlockHash,
            impl BlockRepository<Storage = BlockMemory>,
            impl GasFee,
            impl CreateL1GasFee,
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
            MovedBaseTokenAccounts::new(AccountAddress::ONE),
            InMemoryBlockQueries,
            block_memory,
            state_queries,
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
        let (state_actor, _) = create_state_actor(head_height, MockStateQueries(expected_height));
        let (tx, rx) = oneshot::channel();

        state_actor.handle_query(Query::NonceByHeight {
            height,
            address: primitives::Address::new(hex!("11223344556677889900ffeeaabbccddee111111")),
            response_channel: tx,
        });

        let actual_height = rx.blocking_recv().unwrap();
        let expected_height = 3;

        assert_eq!(actual_height, expected_height);
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
        let (state_actor, _) = create_state_actor(head_height, MockStateQueries(expected_height));
        let (tx, rx) = oneshot::channel();

        state_actor.handle_query(Query::BalanceByHeight {
            height,
            address: primitives::Address::new(hex!("44223344556677889900ffeeaabbccddee111111")),
            response_channel: tx,
        });

        let actual_height = rx.blocking_recv().unwrap();
        let expected_height = U256::from(5);

        assert_eq!(actual_height, expected_height);
    }
}
