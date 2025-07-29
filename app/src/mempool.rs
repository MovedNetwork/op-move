use {
    alloy::primitives::{Address, B256},
    move_core_types::account_address::AccountAddress,
    std::collections::{BTreeMap, HashMap},
    umi_execution::transaction::NormalizedEthTransaction,
    umi_shared::primitives::ToMoveAddress,
};

type Nonce = u64;

// TODO: add address -> account nonce hashmap into the mempool for faster lookups.
// That would require figuring out how Aptos increments those so that
// our copy doesn't get out of sync. Another good piece of functionality
// is invalidation of txs with expired nonces
#[derive(Debug, Clone, Default)]
pub struct Mempool {
    // A hashmap for quicker access to each account, backed by an ordered map
    // so that transaction nonces sequencing is preserved.
    txs: HashMap<AccountAddress, BTreeMap<Nonce, NormalizedEthTransaction>>,
}

impl Mempool {
    /// Insert a [`NormalizedEthTransaction`] into [`Mempool`]. As the key for the underlying
    /// map is derivable from the transaction itself, it doesn't need to be supplied.
    pub fn insert(&mut self, value: NormalizedEthTransaction) -> Option<NormalizedEthTransaction> {
        let address = value.signer.to_move_address();
        let account_txs = self.txs.entry(address).or_default();
        account_txs.insert(value.nonce, value)
    }

    /// Iterate through all transactions from the [`Mempool`] in a sensible order
    /// for block inclusion (ordered by account, then by nonce).
    pub fn iter(&self) -> impl Iterator<Item = &NormalizedEthTransaction> {
        self.txs
            .values()
            .flat_map(|account_txs| account_txs.values())
    }

    pub fn remove_by_hash(
        &mut self,
        tx_hash: B256,
        signer: Address,
    ) -> Option<NormalizedEthTransaction> {
        let signer = signer.to_move_address();
        let account_txs = self.txs.get_mut(&signer)?;

        let nonce = account_txs
            .iter()
            .find(|(_, tx)| tx.tx_hash == tx_hash)
            .map(|(nonce, _)| *nonce)?;

        let removed_tx = account_txs.remove(&nonce);

        if account_txs.is_empty() {
            self.txs.remove(&signer);
        }

        removed_tx
    }

    /// Remove from the [`Mempool`] the transactions that have been included into a built block
    /// and executed successfully.
    pub fn remove_included(&mut self, transactions: &[impl AsRef<NormalizedEthTransaction>]) {
        for tx in transactions {
            let tx = tx.as_ref();
            self.remove_by_hash(tx.tx_hash, tx.signer);
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        alloy::{
            consensus::{SignableTransaction, TxEip1559},
            network::TxSignerSync,
            primitives::{TxKind, ruint::aliases::U256},
            signers::local::PrivateKeySigner,
        },
        umi_shared::primitives::Address,
    };

    use super::*;

    fn create_test_tx(
        signer: &PrivateKeySigner,
        nonce: u64,
        to: Address,
    ) -> NormalizedEthTransaction {
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
        let signed_tx = tx.into_signed(signature);

        signed_tx.try_into().unwrap()
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
}
