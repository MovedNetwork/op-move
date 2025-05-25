use {
    alloy::primitives::{Address, B256},
    move_core_types::account_address::AccountAddress,
    moved_execution::L1GasFeeInput,
    moved_shared::{
        error::{Error, InvalidTransactionCause},
        primitives::ToMoveAddress,
    },
    op_alloy::consensus::OpTxEnvelope,
    std::collections::{BTreeMap, HashMap},
};

type Nonce = u64;

#[derive(Debug, Clone)]
pub struct PendingTransaction {
    pub inner: OpTxEnvelope,
    // Nonce taken out of the envelope for easier ordering
    pub tx_nonce: Nonce,
    pub tx_hash: B256,
    pub l1_gas_fee_input: L1GasFeeInput,
}

impl PendingTransaction {
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
}

// TODO: add address -> account nonce hashmap into the mempool for faster lookups.
// That would require figuring out how Aptos increments those so that
// our copy doesn't get out of sync. Another good piece of functionality
// is invalidation of txs with expired nonces
#[derive(Debug, Clone, Default)]
pub struct Mempool {
    // A hashmap for quicker access to each account, backed by an ordered map
    // so that transaction nonces sequencing is preserved.
    txs: HashMap<AccountAddress, BTreeMap<Nonce, PendingTransaction>>,
}

impl Mempool {
    /// Insert a [`MempoolTransaction`] into [`Mempool`]. As the key for the underlying
    /// map is derivable from the transaction itself, it doesn't need to be supplied.
    pub fn insert(&mut self, value: PendingTransaction) -> Option<PendingTransaction> {
        let address = match self.get_tx_signer(&value) {
            Ok(addr) => addr.to_move_address(),
            // TODO: propagate the error?
            Err(_) => return None,
        };

        let account_txs = self.txs.entry(address).or_default();

        account_txs.insert(value.tx_nonce, value)
    }

    /// Recovers the signer of the transaction to use as a mempool key.
    fn get_tx_signer(&self, tx: &PendingTransaction) -> Result<Address, Error> {
        match &tx.inner {
            OpTxEnvelope::Legacy(signed) => signed
                .recover_signer()
                .map_err(|_| InvalidTransactionCause::InvalidSigner.into()),
            OpTxEnvelope::Eip2930(signed) => signed
                .recover_signer()
                .map_err(|_| InvalidTransactionCause::InvalidSigner.into()),
            OpTxEnvelope::Eip1559(signed) => signed
                .recover_signer()
                .map_err(|_| InvalidTransactionCause::InvalidSigner.into()),
            // EVM account abstraction not planned to be supported
            OpTxEnvelope::Eip7702(_) => Err(Error::InvalidTransaction(
                InvalidTransactionCause::UnsupportedType,
            )),
            // External API only allows canonical transactions in
            OpTxEnvelope::Deposit(_) => Err(Error::InvariantViolation(
                moved_shared::error::InvariantViolation::MempoolTransaction,
            )),
        }
    }

    /// Drains all transactions from the [`Mempool`], returning them in a sensible order
    /// for block inclusion (ordered by account, then by nonce).
    pub fn drain(&mut self) -> impl Iterator<Item = PendingTransaction> {
        let txs = std::mem::take(&mut self.txs);

        txs.into_iter()
            .flat_map(|(_, account_txs)| account_txs.into_values())
    }
}

#[cfg(test)]
mod tests {
    use {
        alloy::{
            consensus::{SignableTransaction, TxEip1559},
            eips::Encodable2718,
            network::TxSignerSync,
            primitives::{TxKind, ruint::aliases::U256},
            signers::local::PrivateKeySigner,
        },
        op_alloy::consensus::TxDeposit,
    };

    use super::*;

    fn create_test_tx(signer: &PrivateKeySigner, nonce: u64, to: Address) -> PendingTransaction {
        let mut tx = TxEip1559 {
            chain_id: 1,
            nonce,
            gas_limit: 21000,
            max_fee_per_gas: 1000000000,
            max_priority_fee_per_gas: 1000000000,
            to: TxKind::Call(to),
            value: U256::from(100),
            access_list: Default::default(),
            input: Default::default(),
        };

        let signature = signer.sign_transaction_sync(&mut tx).unwrap();
        let envelope = OpTxEnvelope::Eip1559(tx.into_signed(signature));
        let tx_hash = envelope.tx_hash();

        PendingTransaction::new(envelope, nonce, tx_hash, L1GasFeeInput::default())
    }

    #[test]
    fn test_insert_multiple_accounts() {
        let mut mempool = Mempool::default();
        let signer1 = PrivateKeySigner::random();
        let signer2 = PrivateKeySigner::random();
        let to = Address::random();

        let tx1 = create_test_tx(&signer1, 0, to);
        let tx2 = create_test_tx(&signer2, 0, to);

        mempool.insert(tx1);
        mempool.insert(tx2);

        let addr1 = signer1.address().to_move_address();
        let addr2 = signer2.address().to_move_address();

        assert_eq!(mempool.txs.len(), 2);
        assert!(mempool.txs.contains_key(&addr1));
        assert!(mempool.txs.contains_key(&addr2));
        assert_eq!(mempool.txs[&addr1].len(), 1);
        assert_eq!(mempool.txs[&addr2].len(), 1);
    }

    #[test]
    fn test_insert_replace_same_nonce() {
        let mut mempool = Mempool::default();
        let signer = PrivateKeySigner::random();
        let to = Address::random();

        let tx1 = create_test_tx(&signer, 0, to);
        let tx2 = create_test_tx(&signer, 0, to); // Same nonce, different tx

        mempool.insert(tx1.clone());
        let replaced = mempool.insert(tx2.clone());

        assert!(replaced.is_some());
        assert_eq!(replaced.unwrap().tx_hash, tx1.tx_hash);

        let addr = signer.address().to_move_address();
        assert_eq!(mempool.txs[&addr].len(), 1);
        assert_eq!(mempool.txs[&addr][&0].tx_hash, tx2.tx_hash);
    }

    #[test]
    fn test_deposit_transaction_rejected() {
        let mut mempool = Mempool::default();

        let deposit_tx = TxDeposit {
            from: Address::random(),
            to: TxKind::Call(Address::random()),
            value: U256::from(100),
            gas_limit: 21000,
            source_hash: B256::random(),
            mint: None,
            is_system_transaction: false,
            input: Default::default(),
        }
        .seal();

        let envelope = OpTxEnvelope::Deposit(deposit_tx);
        let tx_hash = envelope.tx_hash();

        let pending_tx = PendingTransaction::new(envelope, 0, tx_hash, L1GasFeeInput::default());

        let result = mempool.insert(pending_tx);
        assert!(result.is_none());
        assert_eq!(mempool.txs.len(), 0); // Should not be added
    }
}
