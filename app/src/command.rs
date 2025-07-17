use {
    crate::{
        Application, Dependencies, ExecutionOutcome, PayloadForExecution,
        actor::UnrecoverableAppFailure,
        input::{WithExecutionOutcome, WithPayloadAttributes},
    },
    alloy::{consensus::Receipt, primitives::Bloom, rlp::Encodable},
    umi_blockchain::{
        block::{BaseGasFee, Block, BlockHash, BlockRepository, ExtendedBlock, Header},
        payload::{PayloadId, PayloadQueries},
        receipt::{ExtendedReceipt, ReceiptRepository},
        transaction::{ExtendedTransaction, TransactionRepository},
    },
    umi_evm_ext::{
        HeaderForExecution,
        state::{BlockHashWriter, StorageTrieRepository},
    },
    umi_execution::{
        CanonicalExecutionInput, CreateL1GasFee, CreateL2GasFee, DepositExecutionInput, L1GasFee,
        L2GasFeeInput, LogsBloom, execute_transaction,
        transaction::{NormalizedEthTransaction, NormalizedExtendedTxEnvelope, WrapReceipt},
    },
    umi_shared::{
        error::Error::{DatabaseState, InvalidTransaction, InvariantViolation, User},
        primitives::{ToEthAddress, U64, U256},
    },
    umi_state::State,
};

impl<'app, D: Dependencies<'app>> Application<'app, D> {
    #[tracing::instrument(level = "debug", skip(self, attributes))]
    pub(crate) fn start_block_build(
        &mut self,
        attributes: PayloadForExecution,
        id: PayloadId,
    ) -> Result<(), UnrecoverableAppFailure> {
        let payload_exists = self
            .payload_queries
            .by_id(&self.storage_reader, id)
            .map_err(|e| {
                tracing::error!(
                    "Failure during `start_block_build`. Payload queries failed: {e:?}"
                );
                UnrecoverableAppFailure
            })?
            .is_some();
        if payload_exists {
            return Ok(());
        }
        let in_progress_payloads = self.payload_queries.get_in_progress();
        if in_progress_payloads.start_id(id).is_err() {
            return Ok(());
        }

        // Include transactions from both `payload_attributes` and internal mem-pool
        let transactions = attributes
            .transactions
            .iter()
            .cloned()
            .chain(self.mem_pool.drain().map(Into::into))
            .filter_map(|tx|
                // Do not include transactions we have already processed before
                match self.receipt_repository.contains(&self.receipt_memory, tx.tx_hash()) {
                    Ok(false) => Some(Ok(tx)),
                    Ok(true) => None,
                    Err(e) => Some(Err(e)),
                }
            )
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                tracing::error!("Failure during `start_block_build`. Receipt queries failed: {e:?}");
                UnrecoverableAppFailure
            })?;
        let parent = self
            .block_repository
            .latest(&self.storage)
            .map_err(|e| {
                tracing::error!("Failure during `start_block_build`. Failed to get latest block from block repository: {e:?}");
                UnrecoverableAppFailure
            })?
            .expect("Block repository is non-empty (must always at least contain genesis)");
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
        let (execution_outcome, receipts) = self.execute_transactions(
            transactions.clone().into_iter(),
            base_fee,
            &header_for_execution,
        )?;

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
            .into_extended_with_hash(block_hash)
            .with_value(total_tip)
            .with_payload_id(id);

        let block_number = block.block.header.number;

        let extended_transactions = transactions
            .iter()
            .cloned()
            .enumerate()
            .map(|(transaction_index, inner)| {
                ExtendedTransaction::new(
                    inner.effective_gas_price(base_fee),
                    inner,
                    block_number,
                    block_hash,
                    transaction_index as u64,
                )
            })
            .collect::<Vec<_>>();

        let size = block.byte_length(extended_transactions.clone());
        let block = block.with_size(size);

        self.receipt_repository
            .extend(
                &mut self.receipt_memory,
                receipts
                    .into_iter()
                    .map(|receipt| receipt.with_block_hash(block_hash)),
            )
            .map_err(|e| {
                tracing::error!(
                    "Failure during `start_block_build`. Failed to write receipts: {e:?}"
                );
                UnrecoverableAppFailure
            })?;

        self.transaction_repository
            .extend(&mut self.storage, extended_transactions)
            .map_err(|e| {
                tracing::error!(
                    "Failure during `start_block_build`. Failed to write transactions: {e:?}"
                );
                UnrecoverableAppFailure
            })?;

        self.block_hash_writer.push(block_number, block_hash);
        self.block_repository
            .add(&mut self.storage, block.clone())
            .map_err(|e| {
                tracing::error!(
                    "Failure during `start_block_build`. Failed to write produced block: {e:?}"
                );
                UnrecoverableAppFailure
            })?;

        (self.on_payload)(self, id, block_hash).map_err(|e| {
            tracing::error!(
                "Failure during `start_block_build`. `on_payload` callback failed: {e:?}"
            );
            UnrecoverableAppFailure
        })?;
        in_progress_payloads.finish_id(block, transactions.into_iter().map(Into::into));
        Ok(())
    }

    pub fn add_transaction(&mut self, tx: NormalizedEthTransaction) {
        self.mem_pool.insert(tx);
    }

    pub fn genesis_update(
        &mut self,
        block: ExtendedBlock,
    ) -> Result<(), <D::BlockRepository as BlockRepository>::Err> {
        self.block_hash_writer.push(0, block.hash);
        self.block_repository.add(&mut self.storage, block)
    }

    fn execute_transactions(
        &mut self,
        transactions: impl Iterator<Item = NormalizedExtendedTxEnvelope>,
        base_fee: U256,
        block_header: &HeaderForExecution,
    ) -> Result<(ExecutionOutcome, Vec<ExtendedReceipt>), UnrecoverableAppFailure> {
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
            .and_then(|tx| tx.as_deposit())
            .map(|tx| self.l1_fee.for_deposit(tx.input.as_ref()));
        let l2_fee = self.l2_fee.with_default_gas_fee_multiplier();

        // TODO: parallel transaction processing?
        for normalized_tx in transactions {
            let l2_gas_input = L2GasFeeInput::new(
                normalized_tx.gas_limit(),
                normalized_tx.effective_gas_price(base_fee),
            );
            let tx_hash = normalized_tx.tx_hash();
            let input = match &normalized_tx {
                NormalizedExtendedTxEnvelope::Canonical(tx) => CanonicalExecutionInput {
                    tx,
                    tx_hash: &tx.tx_hash,
                    state: self.state.resolver(),
                    storage_trie: &self.evm_storage,
                    genesis_config: &self.genesis_config,
                    l1_cost: l1_fee
                        .as_ref()
                        .map(|v| v.l1_fee(normalized_tx.l1_gas_fee_input()))
                        .unwrap_or(U256::ZERO),
                    l2_fee: l2_fee.clone(),
                    l2_input: l2_gas_input,
                    base_token: &self.base_token,
                    block_header: block_header.clone(),
                    block_hash_lookup: &self.block_hash_lookup,
                    block_hash_writer: &self.block_hash_writer,
                }
                .into(),
                NormalizedExtendedTxEnvelope::DepositedTx(tx) => DepositExecutionInput {
                    tx,
                    tx_hash: &tx_hash,
                    state: self.state.resolver(),
                    storage_trie: &self.evm_storage,
                    genesis_config: &self.genesis_config,
                    block_header: block_header.clone(),
                    block_hash_lookup: &self.block_hash_lookup,
                }
                .into(),
            };
            let outcome = match execute_transaction(input, &mut self.resolver_cache) {
                Ok(outcome) => outcome,
                e @ (Err(InvalidTransaction(_)) | Err(DatabaseState)) => {
                    tracing::warn!("Filtered invalid transaction. hash={tx_hash:?} reason={e:?}");
                    continue;
                }
                Err(User(e)) => unreachable!("User errors are handled in execution {e:?}"),
                Err(InvariantViolation(e)) => panic!("ERROR: execution error {e:?}"),
            };

            let l1_block_info = l1_fee
                .as_ref()
                .and_then(|x| x.l1_block_info(normalized_tx.l1_gas_fee_input()));

            self.on_tx(outcome.changes.move_vm.clone().accounts)
                .map_err(|e| {
                    tracing::error!(
                        "Failure during `start_block_build`. `on_tx` callback failed: {e:?}"
                    );
                    UnrecoverableAppFailure
                })?;

            self.state.apply(outcome.changes.move_vm).map_err(|e| {
                tracing::error!("State update failed for transaction {normalized_tx:?}\n{e:?}");
                UnrecoverableAppFailure
            })?;
            self.evm_storage.apply(outcome.changes.evm).map_err(|e| {
                tracing::error!(
                    "EVM storage update failed for transaction {normalized_tx:?}\n{e:?}"
                );
                UnrecoverableAppFailure
            })?;

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

            let receipt = normalized_tx.wrap_receipt(receipt, bloom);

            total_tip = total_tip.saturating_add(
                U256::from(outcome.gas_used).saturating_mul(normalized_tx.tip_per_gas(base_fee)),
            );

            let (to, from) = match &normalized_tx {
                NormalizedExtendedTxEnvelope::Canonical(tx) => (tx.to.to(), tx.signer),
                NormalizedExtendedTxEnvelope::DepositedTx(tx) => (tx.to.to(), tx.from),
            };

            receipts.push(ExtendedReceipt {
                transaction_hash: normalized_tx.tx_hash(),
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

        (self.on_tx_batch)(self).map_err(|e| {
            tracing::error!(
                "Failure during `start_block_build`. `on_tx_batch` callback failed: {e:?}"
            );
            UnrecoverableAppFailure
        })?;

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
        Ok((outcome, receipts))
    }
}
