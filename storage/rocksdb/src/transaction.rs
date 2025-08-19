use {
    crate::generic::{FromValue, ToValue},
    rocksdb::{AsColumnFamilyRef, DB as RocksDb, WriteBatchWithTransaction},
    std::{marker::PhantomData, sync::Arc},
    umi_blockchain::transaction::{
        ExtendedTransaction, TransactionQueries, TransactionRepository, TransactionResponse,
    },
    umi_shared::primitives::B256,
};

pub const COLUMN_FAMILY: &str = "transaction";

#[derive(Debug)]
pub struct RocksDbTransactionRepository<'db>(PhantomData<&'db ()>);

impl Default for RocksDbTransactionRepository<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl RocksDbTransactionRepository<'_> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl TransactionRepository for RocksDbTransactionRepository<'_> {
    type Err = rocksdb::Error;
    type Storage = Arc<RocksDb>;

    fn extend(
        &mut self,
        db: &mut Self::Storage,
        transactions: impl IntoIterator<Item = ExtendedTransaction>,
    ) -> Result<(), Self::Err> {
        let cf = cf(db);

        db.write(transactions.into_iter().fold(
            WriteBatchWithTransaction::<false>::default(),
            |mut batch, transaction| {
                batch.put_cf(&cf, transaction.hash, transaction.to_value());
                batch
            },
        ))
    }
}

#[derive(Debug, Clone)]
pub struct RocksDbTransactionQueries<'db>(PhantomData<&'db ()>);

impl Default for RocksDbTransactionQueries<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl RocksDbTransactionQueries<'_> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl TransactionQueries for RocksDbTransactionQueries<'_> {
    type Err = rocksdb::Error;
    type Storage = Arc<RocksDb>;

    fn by_hash(
        &self,
        db: &Self::Storage,
        hash: B256,
    ) -> Result<Option<TransactionResponse>, Self::Err> {
        let cf = cf(db);

        Ok(db
            .get_pinned_cf(&cf, hash)?
            .map(|v| ExtendedTransaction::from_value(v.as_ref()).into()))
    }
}

pub(crate) fn cf(db: &RocksDb) -> impl AsColumnFamilyRef + use<'_> {
    db.cf_handle(COLUMN_FAMILY)
        .expect("Column family should exist")
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::{
            consensus::{SignableTransaction, TxEip1559},
            primitives::{Sealable, TxKind, address},
            signers::local::PrivateKeySigner,
        },
        hex_literal::hex,
        op_alloy::{consensus::TxDeposit, network::TxSignerSync},
        umi_execution::transaction::{NormalizedEthTransaction, NormalizedExtendedTxEnvelope},
        umi_shared::primitives::U256,
    };

    const PRIVATE_KEY: [u8; 32] = [0xaa; 32];

    #[test]
    fn test_transaction_deserializes_from_serialized_bytes() {
        let signer = PrivateKeySigner::from_bytes(&PRIVATE_KEY.into()).unwrap();
        let mut tx = TxEip1559 {
            chain_id: 404,
            nonce: 1,
            gas_limit: u64::MAX,
            max_fee_per_gas: 2,
            max_priority_fee_per_gas: 3,
            to: TxKind::Call(address!("ddddddddddadddddddddddd00000000022222222")),
            value: U256::from(23u64),
            access_list: Default::default(),
            input: vec![9, 9, 9].into(),
        };
        let signature = signer.sign_transaction_sync(&mut tx).unwrap();
        let signed_tx = tx.into_signed(signature);
        let normalized_tx = NormalizedEthTransaction::try_from(signed_tx).unwrap();

        let transaction = ExtendedTransaction::new(
            1,
            normalized_tx.into(),
            1,
            B256::new(hex!(
                "2222223123123121231231231231232222222231231231212312312312312322"
            )),
            1,
        );

        let serialized = transaction.to_value();
        let expected_transaction = transaction;
        let actual_transaction = ExtendedTransaction::from_value(serialized.as_slice());

        assert_eq!(actual_transaction, expected_transaction);
    }

    #[test]
    fn test_deposit_transaction_deserializes_from_serialized_bytes() {
        let tx = TxDeposit {
            source_hash: Default::default(),
            gas_limit: u64::MAX,
            to: TxKind::Call(address!("ddddddddddadddddddddddd00000000022222222")),
            mint: None,
            value: U256::from(23u64),
            input: vec![9, 9, 9].into(),
            from: Default::default(),
            is_system_transaction: false,
        };
        let sealed_tx = NormalizedExtendedTxEnvelope::DepositedTx(tx.seal_slow());

        let transaction = ExtendedTransaction::new(
            1,
            sealed_tx,
            1,
            B256::new(hex!(
                "2222223123123121231231231231232222222231231231212312312312312322"
            )),
            1,
        );

        let serialized = transaction.to_value();
        let expected_transaction = transaction;
        let actual_transaction = ExtendedTransaction::from_value(serialized.as_slice());

        assert_eq!(actual_transaction, expected_transaction);
    }
}
