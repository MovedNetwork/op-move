use std::collections::{BTreeMap, HashMap};

use {
    alloy::primitives::{Address, B256},
    move_core_types::account_address::AccountAddress,
    moved_execution::L1GasFeeInput,
    moved_shared::{
        error::{Error, InvalidTransactionCause},
        primitives::ToMoveAddress,
    },
    op_alloy::consensus::OpTxEnvelope,
};

type Nonce = u64;

#[derive(Debug, Clone)]
pub struct MempoolTransaction {
    pub inner: OpTxEnvelope,
    // Nonce taken out of the envelope for easier ordering
    pub tx_nonce: Nonce,
    pub tx_hash: B256,
    pub l1_gas_fee_input: L1GasFeeInput,
}

impl MempoolTransaction {
    pub fn new(
        inner: OpTxEnvelope,
        tx_nonce: Nonce,
        tx_hash: B256,
        l1_gas_fee_input: L1GasFeeInput,
    ) -> Self {
        Self {
            inner,
            tx_nonce,
            tx_hash,
            l1_gas_fee_input,
        }
    }
    pub fn signer(&self) -> Result<Address, Error> {
        match &self.inner {
            OpTxEnvelope::Legacy(signed) => signed
                .recover_signer()
                .map_err(|_| InvalidTransactionCause::InvalidSigner.into()),
            OpTxEnvelope::Eip2930(signed) => signed
                .recover_signer()
                .map_err(|_| InvalidTransactionCause::InvalidSigner.into()),
            OpTxEnvelope::Eip1559(signed) => signed
                .recover_signer()
                .map_err(|_| InvalidTransactionCause::InvalidSigner.into()),
            OpTxEnvelope::Eip7702(_) | OpTxEnvelope::Deposit(_) => Err(Error::InvariantViolation(
                moved_shared::error::InvariantViolation::MempoolTransaction,
            )),
        }
    }
}

// TODO: add address -> account nonce hashmap into the mempool for faster lookups.
// That would require figuring out how Aptos increments those so that
// our copy doesn't get out of sync. Another good piece of functionality
// is invalidation of txs with expired nonces
#[derive(Debug, Clone, Default)]
pub struct Mempool {
    // A hashmap for quicker access to each account, backed by an ordered map
    // so that transaction nonces sequencing is preserved.
    txs: HashMap<AccountAddress, BTreeMap<Nonce, MempoolTransaction>>,
}

impl Mempool {
    /// Insert a [`MempoolTransaction`] into [`Mempool`]. As the key for the underlying
    /// map is derivable from the transaction itself, it doesn't need to be supplied.
    pub fn insert(&mut self, value: MempoolTransaction) -> Option<MempoolTransaction> {
        let address = match value.signer() {
            Ok(addr) => addr.to_move_address(),
            // TODO: propagate the error?
            Err(_) => return None,
        };

        let account_txs = self.txs.entry(address).or_default();

        account_txs.insert(value.tx_nonce, value)
    }

    /// Drains all transactions from the [`Mempool`], returning them in a sensible order
    /// for block inclusion (ordered by account, then by nonce).
    pub fn drain(&mut self) -> impl Iterator<Item = MempoolTransaction> {
        let txs = std::mem::take(&mut self.txs);

        // TODO: any possible dependencies between accounts to be taken care of?
        txs.into_iter()
            .flat_map(|(_, account_txs)| account_txs.into_values())
    }
}
