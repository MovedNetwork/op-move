use {
    crate::{
        Application, Dependencies, ExecutionOutcome, Payload,
        block_hash::StorageBasedProvider,
        input::{WithExecutionOutcome, WithPayloadAttributes},
    },
    alloy::{
        consensus::{Receipt, Transaction, TxEnvelope},
        eips::eip2718::Encodable2718,
        primitives::{Bloom, keccak256},
        rlp::{Decodable, Encodable},
    },
    moved_blockchain::{
        block::{BaseGasFee, Block, BlockHash, BlockRepository, ExtendedBlock, Header},
        payload::PayloadId,
        receipt::{ExtendedReceipt, ReceiptRepository},
        transaction::{ExtendedTransaction, TransactionRepository},
    },
    moved_evm_ext::{HeaderForExecution, state::StorageTrieRepository},
    moved_execution::{
        CanonicalExecutionInput, CreateL1GasFee, CreateL2GasFee, DepositExecutionInput, L1GasFee,
        L1GasFeeInput, L2GasFeeInput, LogsBloom, execute_transaction,
        transaction::{NormalizedExtendedTxEnvelope, WrapReceipt},
    },
    moved_shared::{
        error::Error::{InvalidTransaction, InvariantViolation, User},
        primitives::{B256, ToEthAddress, U64, U256},
    },
    moved_state::State,
    op_alloy::consensus::OpTxEnvelope,
};

impl<D: Dependencies> Application<D> {
    pub fn start_block_build(&mut self, attributes: Payload, id: PayloadId) {
        // Include transactions from both `payload_attributes` and internal mem-pool
        let transactions_with_metadata = attributes
            .transactions
            .iter()
            .filter_map(|tx_bytes| {
                let mut slice: &[u8] = tx_bytes.as_ref();
                let tx_hash = B256::new(keccak256(slice).0);
                let tx = OpTxEnvelope::decode(&mut slice)
                    .inspect_err(|_| {
                        println!("WARN: Failed to RLP decode transaction in payload_attributes")
                    })
                    .ok()?;

                Some((tx_hash, (tx, L1GasFeeInput::from(slice))))
            })
            .chain(self.mem_pool.drain())
            .filter(|(tx_hash, _)|
                // Do not include transactions we have already processed before
                !self.receipt_repository.contains(&self.receipt_memory, *tx_hash).unwrap())
            .collect::<Vec<_>>();
        let parent = self
            .block_repository
            .latest(&self.storage)
            .unwrap()
            .expect("Parent block should exist");
        let base_fee = self.gas_fee.base_fee_per_gas(
            parent.block.header.gas_limit,
            parent.block.header.gas_used,
            U256::from(parent.block.header.base_fee_per_gas.unwrap_or_default()),
        );

        let header_for_execution = HeaderForExecution {
            number: parent.block.header.number + 1,
            timestamp: attributes.timestamp.as_limbs()[0],
            prev_randao: attributes.prev_randao,
        };
        let transactions: Vec<_> = transactions_with_metadata
            .iter()
            .map(|(_, (tx, _))| tx.clone())
            .collect();
        let (execution_outcome, receipts) = self.execute_transactions(
            transactions_with_metadata
                .into_iter()
                .map(|(tx_hash, (tx, bytes))| (tx_hash, tx, bytes)),
            base_fee,
            &header_for_execution,
        );

        let transactions_root = alloy_trie::root::ordered_trie_root(&transactions);
        // TODO: is this the correct withdrawals root calculation?
        let withdrawals_root = alloy_trie::root::ordered_trie_root(&attributes.withdrawals);
        let total_tip = execution_outcome.total_tip;

        let header = Header {
            parent_hash: parent.hash,
            number: header_for_execution.number,
            transactions_root,
            withdrawals_root: Some(withdrawals_root),
            base_fee_per_gas: Some(base_fee.saturating_to()),
            blob_gas_used: Some(0),
            excess_blob_gas: Some(0),
            ..Default::default()
        }
        .with_payload_attributes(attributes)
        .with_execution_outcome(execution_outcome);

        let block_hash = self.block_hash.block_hash(&header);

        let block = Block::new(header, transactions.iter().map(|v| v.trie_hash()).collect())
            .with_hash(block_hash)
            .with_value(total_tip);

        let block_number = block.block.header.number;
        let base_fee = block.block.header.base_fee_per_gas;

        self.receipt_repository
            .extend(
                &mut self.receipt_memory,
                receipts
                    .into_iter()
                    .map(|receipt| receipt.with_block_hash(block_hash)),
            )
            .unwrap();

        self.transaction_repository
            .extend(
                &mut self.storage,
                transactions
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

        self.block_repository.add(&mut self.storage, block).unwrap();

        (self.on_payload)(self, id, block_hash);
    }

    pub fn add_transaction(&mut self, tx: TxEnvelope) {
        let tx_hash = tx.tx_hash().0.into();
        let mut encoded = Vec::new();
        tx.encode(&mut encoded);
        let encoded = encoded.as_slice().into();
        self.mem_pool.insert(
            tx_hash,
            (
                OpTxEnvelope::try_from_eth_envelope(tx)
                    .unwrap_or_else(|_| unreachable!("EIP-4844 not supported")),
                encoded,
            ),
        );
    }

    pub fn genesis_update(&mut self, block: ExtendedBlock) {
        self.block_repository.add(&mut self.storage, block).unwrap();
    }

    fn execute_transactions(
        &mut self,
        transactions: impl Iterator<Item = (B256, OpTxEnvelope, L1GasFeeInput)>,
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
            .and_then(|(_, v, _)| v.as_deposit())
            .map(|tx| self.l1_fee.for_deposit(tx.input.as_ref()));
        let l2_fee = self.l2_fee.with_default_gas_fee_multiplier();

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
            let block_hash_lookup =
                StorageBasedProvider::new(&self.storage_reader, &self.block_queries);
            let input = match &normalized_tx {
                NormalizedExtendedTxEnvelope::Canonical(tx) => CanonicalExecutionInput {
                    tx,
                    tx_hash: &tx_hash,
                    state: self.state.resolver(),
                    storage_trie: &self.evm_storage,
                    genesis_config: &self.genesis_config,
                    l1_cost: l1_fee
                        .as_ref()
                        .map(|v| v.l1_fee(l1_cost_input.clone()))
                        .unwrap_or(U256::ZERO),
                    l2_fee: l2_fee.clone(),
                    l2_input: l2_gas_input,
                    base_token: &self.base_token,
                    block_header: block_header.clone(),
                    block_hash_lookup: &block_hash_lookup,
                }
                .into(),
                NormalizedExtendedTxEnvelope::DepositedTx(tx) => DepositExecutionInput {
                    tx,
                    tx_hash: &tx_hash,
                    state: self.state.resolver(),
                    storage_trie: &self.evm_storage,
                    genesis_config: &self.genesis_config,
                    block_header: block_header.clone(),
                    block_hash_lookup: &block_hash_lookup,
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

            self.on_tx(outcome.changes.move_vm.clone());

            self.state
                .apply(outcome.changes.move_vm)
                .unwrap_or_else(|e| {
                    panic!("ERROR: state update failed for transaction {tx:?}\n{e:?}")
                });
            self.evm_storage
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
                NormalizedExtendedTxEnvelope::Canonical(tx) => (tx.to.to(), tx.signer),
                NormalizedExtendedTxEnvelope::DepositedTx(tx) => (tx.to.to(), tx.from),
            };

            receipts.push(ExtendedReceipt {
                transaction_hash: tx_hash,
                to: to.copied(),
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

        (self.on_tx_batch)(self);

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
}
