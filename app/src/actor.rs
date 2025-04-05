use {
    crate::{
        Application, Dependencies, DependenciesThreadSafe,
        input::{
            Command, ExecutionOutcome, Payload, Query, StateMessage, WithExecutionOutcome,
            WithPayloadAttributes,
        },
    },
    alloy::{
        consensus::{Receipt, Transaction},
        eips::{
            BlockId,
            BlockNumberOrTag::{self, *},
            eip2718::Encodable2718,
        },
        primitives::{Bloom, TxKind, keccak256},
        rlp::{Decodable, Encodable},
        rpc::types::FeeHistory,
    },
    move_core_types::effects::ChangeSet,
    moved_blockchain::{
        block::{
            BaseGasFee, Block, BlockHash, BlockQueries, BlockRepository, ExtendedBlock, Header,
        },
        payload::{InMemoryPayloadQueries, PayloadId, PayloadQueries},
        receipt::{ExtendedReceipt, ReceiptQueries, ReceiptRepository},
        state::{InMemoryStateQueries, StateQueries},
        transaction::{ExtendedTransaction, TransactionQueries, TransactionRepository},
    },
    moved_evm_ext::{HeaderForExecution, state::StorageTrieRepository},
    moved_execution::{
        CanonicalExecutionInput, CreateL1GasFee, CreateL2GasFee, DepositExecutionInput, L1GasFee,
        L1GasFeeInput, L2GasFeeInput, LogsBloom, execute_transaction,
        simulate::{call_transaction, simulate_transaction},
        transaction::{ExtendedTxEnvelope, NormalizedExtendedTxEnvelope},
    },
    moved_genesis::config::GenesisConfig,
    moved_shared::{
        error::Error::{InvalidTransaction, InvariantViolation, User},
        primitives::{B256, ToEthAddress, ToMoveAddress, ToSaturatedU64, U64, U256},
    },
    moved_state::State,
    op_alloy::consensus::OpTxEnvelope,
    std::collections::HashMap,
    tokio::{sync::mpsc::Receiver, task::JoinHandle},
};

/// A function invoked on a completion of new transaction execution batch.
pub type OnTxBatch<S> = dyn Fn(&mut S) + Send + Sync;

/// A function invoked on an execution of a new transaction.
pub type OnTx<S> = dyn Fn(&mut S, ChangeSet) + Send + Sync;

/// A function invoked on an execution of a new payload.
pub type OnPayload<S> = dyn Fn(&mut S, PayloadId, B256) + Send + Sync;

pub struct StateActor<D: Dependencies> {
    genesis_config: GenesisConfig,
    rx: Receiver<StateMessage>,
    head: B256,
    height: u64,
    mem_pool: HashMap<B256, (ExtendedTxEnvelope, L1GasFeeInput)>,
    app: Application<D>,
}

impl<D: DependenciesThreadSafe> StateActor<D> {
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

impl<D: Dependencies> StateActor<D> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rx: Receiver<StateMessage>,
        head: B256,
        height: u64,
        genesis_config: GenesisConfig,
        app: Application<D>,
    ) -> Self {
        Self {
            genesis_config,
            rx,
            head,
            height,
            mem_pool: HashMap::new(),
            app,
        }
    }

    pub fn resolve_height(&self, height: BlockNumberOrTag) -> u64 {
        match height {
            Number(height) => height,
            Finalized | Pending | Latest | Safe => self.height,
            Earliest => 0,
        }
    }

    pub fn height_from_block_id(&self, id: BlockId) -> Option<u64> {
        Some(match id {
            BlockId::Number(height) => self.resolve_height(height),
            BlockId::Hash(h) => {
                self.app
                    .block_queries
                    .by_hash(&self.app.storage, h.block_hash, false)
                    .ok()??
                    .0
                    .header
                    .number
            }
        })
    }

    pub fn handle_query(&self, msg: Query) {
        match msg {
            Query::ChainId { response_channel } => response_channel.send(self.genesis_config.chain_id).ok(),
            Query::BalanceByHeight {
                address,
                response_channel,
                height,
            } => response_channel
                .send(self.app.state_queries.balance_at(self.app.state.db(), &self.app.evm_storage, address.to_move_address(), self.resolve_height(height)))
                .ok(),
            Query::NonceByHeight {
                address,
                response_channel,
                height,
            } => response_channel
                .send(self.app.state_queries.nonce_at(self.app.state.db(), &self.app.evm_storage, address.to_move_address(), self.resolve_height(height)))
                .ok(),
            Query::BlockByHash {
                hash,
                response_channel,
                include_transactions,
            } => response_channel
                .send(self.app.block_queries.by_hash(&self.app.storage, hash, include_transactions).unwrap())
                .ok(),
            Query::BlockByHeight {
                height,
                response_channel,
                include_transactions,
            } => response_channel
                .send(self.app.block_queries.by_height(&self.app.storage, self.resolve_height(height), include_transactions).unwrap())
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
                let outcome = simulate_transaction(transaction, self.app.state.resolver(), &self.app.evm_storage, &self.genesis_config, &self.app.base_token, block_height);
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
                let outcome = call_transaction(transaction, self.app.state.resolver(), &self.app.evm_storage, &self.genesis_config, &self.app.base_token);
                response_channel.send(outcome).ok()
            }
            Query::TransactionReceipt { tx_hash, response_channel } => {
                response_channel.send(self.app.receipt_queries.by_transaction_hash(&self.app.receipt_memory, tx_hash).unwrap()).ok()
            }
            Query::TransactionByHash { tx_hash, response_channel } => response_channel
                .send(self.app.transaction_queries.by_hash(&self.app.storage, tx_hash).ok().flatten())
                .ok(),
            Query::GetProof { address, storage_slots, height, response_channel } => {
                response_channel.send(
                    self.height_from_block_id(height).and_then(|height| {
                        self.app.state_queries.proof_at(
                            self.app.state.db(),
                            &self.app.evm_storage,
                            address.to_move_address(),
                            &storage_slots,
                            height,
                        )
                    })
                ).ok()
            }
            Query::GetPayload {
                id: payload_id,
                response_channel,
            } => response_channel
                .send(self.app.payload_queries.by_id(&self.app.storage, payload_id).ok().flatten())
                .ok(),
            Query::GetPayloadByBlockHash {
                block_hash,
                response_channel,
            } => response_channel
                .send(self.app.payload_queries.by_hash(&self.app.storage, block_hash).ok().flatten())
                .ok(),
        };
    }

    pub fn handle_command(&mut self, msg: Command) {
        match msg {
            Command::UpdateHead { block_hash } => {
                self.head = block_hash;
            }
            Command::StartBlockBuild {
                payload_attributes,
                payload_id: id,
            } => {
                let block = self.create_block(payload_attributes);
                self.app
                    .block_repository
                    .add(&mut self.app.storage, block.clone())
                    .unwrap();
                let block_number = block.block.header.number;
                let block_hash = block.hash;
                let base_fee = block.block.header.base_fee_per_gas;
                self.app
                    .transaction_repository
                    .extend(
                        &mut self.app.storage,
                        block
                            .block
                            .transactions
                            .clone()
                            .into_iter()
                            .enumerate()
                            .map(|(transaction_index, inner)| {
                                ExtendedTransaction::new(
                                    inner.effective_gas_price(base_fee),
                                    inner,
                                    block_number,
                                    block_hash,
                                    transaction_index as u64,
                                )
                            }),
                    )
                    .unwrap();
                self.height = self.height.max(block_number);
                (self.app.on_payload)(&mut self.app, id, block_hash);
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
                self.app
                    .block_repository
                    .add(&mut self.app.storage, block)
                    .unwrap();
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
            .filter(|(tx_hash, _)|
                // Do not include transactions we have already processed before
                !self.app.receipt_repository.contains(&self.app.receipt_memory, *tx_hash).unwrap())
            .collect::<Vec<_>>();
        let parent = self
            .app
            .block_repository
            .by_hash(&self.app.storage, self.head)
            .unwrap()
            .expect("Parent block should exist");
        let base_fee = self.app.gas_fee.base_fee_per_gas(
            parent.block.header.gas_limit,
            parent.block.header.gas_used,
            U256::from(parent.block.header.base_fee_per_gas.unwrap_or_default()),
        );

        let header_for_execution = HeaderForExecution {
            number: self.height + 1,
            timestamp: payload_attributes.timestamp.as_limbs()[0],
            prev_randao: payload_attributes.prev_randao,
        };
        let op_transactions: Vec<_> = transactions
            .iter()
            .map(|(_, (tx, _))| OpTxEnvelope::from(tx.clone()))
            .collect();
        let (execution_outcome, receipts) = self.execute_transactions(
            transactions
                .into_iter()
                .map(|(tx_hash, (tx, bytes))| (tx_hash, tx, bytes)),
            base_fee,
            &header_for_execution,
        );

        let transactions_root =
            alloy_trie::root::ordered_trie_root_with_encoder(&op_transactions, |tx, buf| {
                tx.encode_2718(buf)
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

        let hash = self.app.block_hash.block_hash(&header);

        self.app
            .receipt_repository
            .extend(
                &mut self.app.receipt_memory,
                receipts
                    .into_iter()
                    .map(|receipt| receipt.with_block_hash(hash)),
            )
            .unwrap();

        Block::new(header, op_transactions)
            .with_hash(hash)
            .with_value(total_tip)
    }

    fn execute_transactions(
        &mut self,
        transactions: impl Iterator<Item = (B256, ExtendedTxEnvelope, L1GasFeeInput)>,
        base_fee: U256,
        block_header: &HeaderForExecution,
    ) -> (ExecutionOutcome, Vec<ExtendedReceipt>) {
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
            .map(|tx| self.app.l1_fee.for_deposit(tx.data.as_ref()));
        let l2_fee = self.app.l2_fee.with_gas_fee_multiplier(U256::from(1));

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
            let input = match &normalized_tx {
                NormalizedExtendedTxEnvelope::Canonical(tx) => CanonicalExecutionInput {
                    tx,
                    tx_hash: &tx_hash,
                    state: self.app.state.resolver(),
                    storage_trie: &self.app.evm_storage,
                    genesis_config: &self.genesis_config,
                    l1_cost: l1_fee
                        .as_ref()
                        .map(|v| v.l1_fee(l1_cost_input.clone()).to_saturated_u64())
                        .unwrap_or(0),
                    l2_fee: l2_fee.clone(),
                    l2_input: l2_gas_input,
                    base_token: &self.app.base_token,
                    block_header: block_header.clone(),
                }
                .into(),
                NormalizedExtendedTxEnvelope::DepositedTx(tx) => DepositExecutionInput {
                    tx,
                    tx_hash: &tx_hash,
                    state: self.app.state.resolver(),
                    storage_trie: &self.app.evm_storage,
                    genesis_config: &self.genesis_config,
                    block_header: block_header.clone(),
                }
                .into(),
            };
            let outcome = match execute_transaction(input) {
                Ok(outcome) => outcome,
                Err(User(e)) => unreachable!("User errors are handled in execution {e:?}"),
                Err(InvalidTransaction(_)) => continue,
                Err(InvariantViolation(e)) => panic!("ERROR: execution error {e:?}"),
            };

            let l1_block_info = l1_fee.as_ref().and_then(|x| x.l1_block_info(l1_cost_input));

            self.app.on_tx(outcome.changes.move_vm.clone());

            self.app
                .state
                .apply(outcome.changes.move_vm)
                .unwrap_or_else(|e| {
                    panic!("ERROR: state update failed for transaction {tx:?}\n{e:?}")
                });
            self.app
                .evm_storage
                .apply(outcome.changes.evm)
                .unwrap_or_else(|e| {
                    panic!("ERROR: EVM storage update failed for transaction {tx:?}\n{e:?}")
                });

            cumulative_gas_used = cumulative_gas_used.saturating_add(outcome.gas_used as u128);

            let bloom = outcome.logs.iter().logs_bloom();
            logs_bloom.accrue_bloom(&bloom);

            let tx_log_offset = log_offset;
            log_offset += outcome.logs.len() as u64;
            let receipt = Receipt {
                status: outcome.vm_outcome.is_ok().into(),
                cumulative_gas_used: if cumulative_gas_used < u64::MAX as u128 {
                    cumulative_gas_used as u64
                } else {
                    u64::MAX
                },
                logs: outcome.logs,
            };

            let receipt = tx.wrap_receipt(receipt, bloom);

            total_tip = total_tip.saturating_add(
                U256::from(outcome.gas_used).saturating_mul(normalized_tx.tip_per_gas(base_fee)),
            );

            let (to, from) = match &normalized_tx {
                NormalizedExtendedTxEnvelope::Canonical(tx) => {
                    let to = match tx.to {
                        TxKind::Call(to) => Some(to),
                        TxKind::Create => None,
                    };
                    (to, tx.signer)
                }
                NormalizedExtendedTxEnvelope::DepositedTx(tx) => (Some(tx.to), tx.from),
            };

            receipts.push(ExtendedReceipt {
                transaction_hash: tx_hash,
                to,
                from,
                receipt,
                l1_block_info,
                gas_used: outcome.gas_used,
                l2_gas_price: outcome.l2_price,
                transaction_index: tx_index,
                contract_address: outcome
                    .deployment
                    .map(|(address, _)| address.to_eth_address()),
                logs_offset: tx_log_offset,
                block_hash: Default::default(),
                block_number: block_header.number,
                block_timestamp: block_header.timestamp,
            });

            tx_index += 1;
        }

        (self.app.on_tx_batch)(&mut self.app);

        // Compute the receipts root by RLP-encoding each receipt to be a leaf of
        // a merkle trie.
        let receipts_root =
            alloy_trie::root::ordered_trie_root_with_encoder(&receipts, |rx, buf| {
                rx.receipt.encode(buf)
            });
        let logs_bloom = logs_bloom.into();

        let outcome = ExecutionOutcome {
            state_root: self.app.state.state_root(),
            gas_used: U64::from(cumulative_gas_used),
            receipts_root,
            logs_bloom,
            total_tip,
        };
        (outcome, receipts)
    }

    pub fn on_tx_batch_noop() -> &'static OnTxBatch<Application<D>> {
        &|_| {}
    }

    pub fn on_tx_noop() -> &'static OnTx<Application<D>> {
        &|_, _| {}
    }

    pub fn on_payload_noop() -> &'static OnPayload<Application<D>> {
        &|_, _, _| {}
    }
}

impl<D: Dependencies<StateQueries = InMemoryStateQueries>> StateActor<D> {
    pub fn on_tx_in_memory() -> &'static OnTx<Application<D>> {
        &|_state, _changes| ()
    }

    pub fn on_tx_batch_in_memory() -> &'static OnTxBatch<Application<D>> {
        &|state| {
            state
                .state_queries
                .push_state_root(state.state.state_root())
        }
    }
}

impl<D: Dependencies<PayloadQueries = InMemoryPayloadQueries>> StateActor<D> {
    pub fn on_payload_in_memory() -> &'static OnPayload<Application<D>> {
        &|state, payload_id, block_hash| {
            state.payload_queries.add_block_hash(payload_id, block_hash)
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

    let hash = moved_blockchain::block::MovedBlockHash.block_hash(&header);
    assert_eq!(
        hash,
        B256::new(hex!(
            "c9f7a6ef5311bf49b8322a92f3d75bd5c505ee613323fb58c7166c3511a62bcf"
        ))
    );
}

#[cfg(test)]
#[allow(clippy::type_complexity)]
mod tests {
    use move_vm_runtime::AsUnsyncCodeStorage;

    use {
        super::*,
        alloy::{
            consensus::{SignableTransaction, TxEip1559, TxEnvelope},
            hex,
            network::TxSignerSync,
            primitives::{TxKind, address},
        },
        move_core_types::{account_address::AccountAddress, effects::ChangeSet},
        move_vm_runtime::module_traversal::{TraversalContext, TraversalStorage},
        move_vm_types::gas::UnmeteredGasMeter,
        moved_blockchain::{
            block::{Eip1559GasFee, InMemoryBlockQueries, InMemoryBlockRepository, MovedBlockHash},
            in_memory::SharedMemory,
            payload::InMemoryPayloadQueries,
            receipt::{InMemoryReceiptQueries, InMemoryReceiptRepository, ReceiptMemory},
            state::MockStateQueries,
            transaction::{InMemoryTransactionQueries, InMemoryTransactionRepository},
        },
        moved_execution::{MovedBaseTokenAccounts, create_vm_session, session_id::SessionId},
        moved_genesis::{
            CreateMoveVm, MovedVm,
            config::{CHAIN_ID, GenesisConfig},
        },
        moved_shared::primitives::Address,
        moved_state::{InMemoryState, ResolverBasedModuleBytesStorage},
        std::convert::Infallible,
        test_case::test_case,
        tokio::sync::{
            mpsc::{self, Sender},
            oneshot,
        },
    };

    /// The address corresponding to this private key is 0x8fd379246834eac74B8419FfdA202CF8051F7A03
    pub const PRIVATE_KEY: [u8; 32] = [0xaa; 32];

    pub const EVM_ADDRESS: Address = address!("8fd379246834eac74b8419ffda202cf8051f7a03");

    use {
        crate::TestDependencies, alloy::signers::local::PrivateKeySigner,
        moved_blockchain::state::BlockHeight, moved_evm_ext::state::InMemoryStorageTrieRepository,
    };

    #[derive(Debug)]
    pub struct Signer {
        pub inner: PrivateKeySigner,
        pub nonce: u64,
    }

    impl Signer {
        pub fn new(key_bytes: &[u8; 32]) -> Self {
            Self {
                inner: PrivateKeySigner::from_bytes(&key_bytes.into()).unwrap(),
                nonce: 0,
            }
        }
    }

    fn create_state_actor_with_given_queries<SQ: StateQueries + Send + Sync + 'static>(
        height: u64,
        state_queries: SQ,
    ) -> (
        StateActor<
            impl DependenciesThreadSafe<
                SharedStorage = SharedMemory,
                ReceiptStorage = ReceiptMemory,
                BlockQueries = impl BlockQueries<Err = Infallible>,
                PayloadQueries = impl PayloadQueries<Err = Infallible>,
                StateQueries = SQ,
            >,
        >,
        Sender<StateMessage>,
    ) {
        let genesis_config = GenesisConfig::default();
        let (state_channel, rx) = mpsc::channel(10);

        let head_hash = B256::new(hex!(
            "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
        ));
        let genesis_block = Block::default().with_hash(head_hash).with_value(U256::ZERO);

        let mut memory = SharedMemory::new();
        let mut repository = InMemoryBlockRepository::new();
        repository.add(&mut memory, genesis_block).unwrap();

        let mut state = InMemoryState::new();
        let mut evm_storage = InMemoryStorageTrieRepository::new();
        let (changes, tables, evm_storage_changes) = moved_genesis_image::load();
        moved_genesis::apply(
            changes,
            tables,
            evm_storage_changes,
            &genesis_config,
            &mut state,
            &mut evm_storage,
        );

        let state: StateActor<TestDependencies<SQ>> = StateActor::new(
            rx,
            head_hash,
            height,
            genesis_config,
            Application {
                base_token: MovedBaseTokenAccounts::new(AccountAddress::ONE),
                block_hash: MovedBlockHash,
                block_queries: InMemoryBlockQueries,
                block_repository: repository,
                on_payload: StateActor::on_payload_noop(),
                on_tx: StateActor::on_tx_noop(),
                on_tx_batch: StateActor::on_tx_batch_noop(),
                payload_queries: InMemoryPayloadQueries::new(),
                receipt_queries: InMemoryReceiptQueries::new(),
                receipt_repository: InMemoryReceiptRepository::new(),
                receipt_memory: ReceiptMemory::new(),
                storage: memory,
                state,
                state_queries,
                evm_storage,
                transaction_queries: InMemoryTransactionQueries::new(),
                transaction_repository: InMemoryTransactionRepository::new(),
                gas_fee: Eip1559GasFee::default(),
                l1_fee: U256::ZERO,
                l2_fee: U256::ZERO,
            },
        );
        (state, state_channel)
    }

    fn mint_eth(
        state: &impl State,
        evm_storage: &impl StorageTrieRepository,
        addr: AccountAddress,
        amount: U256,
    ) -> ChangeSet {
        let moved_vm = MovedVm::default();
        let module_bytes_storage = ResolverBasedModuleBytesStorage::new(state.resolver());
        let code_storage = module_bytes_storage.as_unsync_code_storage(&moved_vm);
        let vm = moved_vm.create_move_vm().unwrap();
        let mut session = create_vm_session(
            &vm,
            state.resolver(),
            SessionId::default(),
            evm_storage,
            &(),
        );
        let traversal_storage = TraversalStorage::new();
        let mut traversal_context = TraversalContext::new(&traversal_storage);
        let mut gas_meter = UnmeteredGasMeter;

        moved_execution::mint_eth(
            &addr,
            amount,
            &mut session,
            &mut traversal_context,
            &mut gas_meter,
            &code_storage,
        )
        .unwrap();

        session.finish(&code_storage).unwrap()
    }

    fn create_state_actor_with_fake_queries(
        addr: AccountAddress,
        initial_balance: U256,
    ) -> (
        StateActor<
            impl Dependencies<
                SharedStorage = SharedMemory,
                ReceiptStorage = ReceiptMemory,
                BlockQueries = impl BlockQueries<Err = Infallible>,
                PayloadQueries = impl PayloadQueries<Err = Infallible>,
            >,
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

        let mut memory = SharedMemory::new();
        let mut repository = InMemoryBlockRepository::new();
        repository.add(&mut memory, genesis_block).unwrap();

        let evm_storage = InMemoryStorageTrieRepository::new();
        let mut state = InMemoryState::new();
        let (genesis_changes, table_changes, evm_storage_changes) = moved_genesis_image::load();
        state
            .apply_with_tables(genesis_changes.clone(), table_changes)
            .unwrap();
        evm_storage.apply(evm_storage_changes).unwrap();
        let changes_addition = mint_eth(&state, &evm_storage, addr, initial_balance);
        state.apply(changes_addition.clone()).unwrap();

        let state_queries = InMemoryStateQueries::from_genesis(state.state_root());

        let state: StateActor<TestDependencies> = StateActor::new(
            rx,
            head_hash,
            height,
            genesis_config,
            Application {
                base_token: MovedBaseTokenAccounts::new(AccountAddress::ONE),
                block_hash: MovedBlockHash,
                block_queries: InMemoryBlockQueries,
                block_repository: repository,
                on_payload: StateActor::on_payload_in_memory(),
                on_tx: StateActor::on_tx_in_memory(),
                on_tx_batch: StateActor::on_tx_batch_in_memory(),
                payload_queries: InMemoryPayloadQueries::new(),
                receipt_queries: InMemoryReceiptQueries::new(),
                receipt_repository: InMemoryReceiptRepository::new(),
                receipt_memory: ReceiptMemory::new(),
                storage: memory,
                state,
                state_queries,
                evm_storage,
                transaction_queries: InMemoryTransactionQueries::new(),
                transaction_repository: InMemoryTransactionRepository::new(),
                gas_fee: Eip1559GasFee::default(),
                l1_fee: U256::ZERO,
                l2_fee: U256::ZERO,
            },
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
        let address = Address::new(hex!("11223344556677889900ffeeaabbccddee111111"));
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
        let address = Address::new(hex!("44223344556677889900ffeeaabbccddee111111"));
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

    fn create_transaction(nonce: u64) -> TxEnvelope {
        let to = Address::new(hex!("44223344556677889900ffeeaabbccddee111111"));
        let amount = U256::from(4);
        let signer = Signer::new(&PRIVATE_KEY);
        let mut tx = TxEip1559 {
            chain_id: CHAIN_ID,
            nonce: signer.nonce + nonce,
            gas_limit: u64::MAX,
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            to: TxKind::Call(to),
            value: amount,
            access_list: Default::default(),
            input: Default::default(),
        };
        let signature = signer.inner.sign_transaction_sync(&mut tx).unwrap();

        TxEnvelope::Eip1559(tx.into_signed(signature))
    }

    #[test]
    fn test_fetched_balances_are_updated_after_transfer_of_funds() {
        let to = Address::new(hex!("44223344556677889900ffeeaabbccddee111111"));
        let initial_balance = U256::from(5);
        let amount = U256::from(4);
        let (mut state_actor, _) =
            create_state_actor_with_fake_queries(EVM_ADDRESS.to_move_address(), initial_balance);

        let tx = create_transaction(0);

        state_actor.handle_command(Command::AddTransaction { tx: tx.clone() });
        state_actor.handle_command(Command::StartBlockBuild {
            payload_attributes: Default::default(),
            payload_id: U64::from(0x03421ee50df45cacu64),
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
        let to = Address::new(hex!("44223344556677889900ffeeaabbccddee111111"));
        let initial_balance = U256::from(5);
        let (mut state_actor, _) =
            create_state_actor_with_fake_queries(EVM_ADDRESS.to_move_address(), initial_balance);

        let tx = create_transaction(0);

        state_actor.handle_command(Command::AddTransaction { tx });
        state_actor.handle_command(Command::StartBlockBuild {
            payload_attributes: Default::default(),
            payload_id: U64::from(0x03421ee50df45cacu64),
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

    #[test]
    fn test_one_payload_can_be_fetched_repeatedly() {
        let initial_balance = U256::from(5);
        let (mut state_actor, _) =
            create_state_actor_with_fake_queries(EVM_ADDRESS.to_move_address(), initial_balance);

        let tx = create_transaction(0);

        state_actor.handle_command(Command::AddTransaction { tx });

        let payload_id = U64::from(0x03421ee50df45cacu64);

        state_actor.handle_command(Command::StartBlockBuild {
            payload_attributes: Default::default(),
            payload_id,
        });

        let (tx, rx) = oneshot::channel();

        state_actor.handle_query(Query::GetPayload {
            id: payload_id,
            response_channel: tx,
        });

        let expected_payload = rx.blocking_recv().unwrap();

        let (tx, rx) = oneshot::channel();

        state_actor.handle_query(Query::GetPayload {
            id: payload_id,
            response_channel: tx,
        });

        let actual_payload = rx.blocking_recv().unwrap();

        assert_eq!(expected_payload, actual_payload);
    }

    #[test]
    fn test_older_payload_can_be_fetched_again_successfully() {
        let initial_balance = U256::from(15);
        let (mut state_actor, _) =
            create_state_actor_with_fake_queries(EVM_ADDRESS.to_move_address(), initial_balance);

        let tx = create_transaction(0);

        state_actor.handle_command(Command::AddTransaction { tx });

        let payload_id = U64::from(0x03421ee50df45cacu64);

        state_actor.handle_command(Command::StartBlockBuild {
            payload_attributes: Default::default(),
            payload_id,
        });

        let (tx, rx) = oneshot::channel();

        state_actor.handle_query(Query::GetPayload {
            id: payload_id,
            response_channel: tx,
        });

        let expected_payload = rx.blocking_recv().unwrap();

        let tx = create_transaction(1);

        state_actor.handle_command(Command::AddTransaction { tx });

        let payload_2_id = U64::from(0x03421ee50df45dadu64);

        state_actor.handle_command(Command::StartBlockBuild {
            payload_attributes: Payload {
                timestamp: U64::from(1u64),
                ..Default::default()
            },
            payload_id: payload_2_id,
        });

        let (tx, _rx) = oneshot::channel();

        state_actor.handle_query(Query::GetPayload {
            id: payload_2_id,
            response_channel: tx,
        });

        let (tx, rx) = oneshot::channel();

        state_actor.handle_query(Query::GetPayload {
            id: payload_id,
            response_channel: tx,
        });

        let actual_payload = rx.blocking_recv().unwrap();

        assert_eq!(expected_payload, actual_payload);
    }
}
