use {
    crate::generic::{FromValue, ToValue},
    moved::transaction::{
        ExtendedTransaction, TransactionQueries, TransactionRepository, TransactionResponse,
    },
    moved_shared::primitives::B256,
    rocksdb::{AsColumnFamilyRef, WriteBatchWithTransaction, DB as RocksDb},
};

pub const COLUMN_FAMILY: &str = "transaction";

#[derive(Debug)]
pub struct RocksDbTransactionRepository;

impl TransactionRepository for RocksDbTransactionRepository {
    type Err = rocksdb::Error;
    type Storage = &'static RocksDb;

    fn extend(
        &mut self,
        db: &mut Self::Storage,
        transactions: impl IntoIterator<Item = ExtendedTransaction>,
    ) -> Result<(), Self::Err> {
        let cf = cf(db);
        let mut batch = WriteBatchWithTransaction::<false>::default();

        for transaction in transactions {
            let bytes = transaction.to_value();
            batch.put_cf(&cf, transaction.hash(), bytes);
        }

        db.write(batch)
    }
}

#[derive(Debug)]
pub struct RocksDbTransactionQueries;

impl TransactionQueries for RocksDbTransactionQueries {
    type Err = rocksdb::Error;
    type Storage = &'static RocksDb;

    fn by_hash(
        &self,
        db: &Self::Storage,
        hash: B256,
    ) -> Result<Option<TransactionResponse>, Self::Err> {
        let cf = cf(db);

        Ok(db
            .get_pinned_cf(&cf, hash)?
            .and_then(|v| FromValue::from_value(v.as_ref())))
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
            primitives::{address, Sealable, TxKind},
            signers::local::PrivateKeySigner,
        },
        hex_literal::hex,
        moved_shared::primitives::U256,
        op_alloy::{
            consensus::{OpTxEnvelope, TxDeposit},
            network::TxSignerSync,
        },
    };

    pub const PRIVATE_KEY: [u8; 32] = [0xaa; 32];

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
        let signed_tx = OpTxEnvelope::Eip1559(tx.into_signed(signature));

        let transaction = ExtendedTransaction {
            inner: signed_tx,
            block_number: 1,
            block_hash: B256::new(hex!(
                "2222223123123121231231231231232222222231231231212312312312312322"
            )),
            transaction_index: 1,
            effective_gas_price: 1,
        };

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
        let sealed_tx = OpTxEnvelope::Deposit(tx.seal_slow());

        let transaction = ExtendedTransaction {
            inner: sealed_tx,
            block_number: 1,
            block_hash: B256::new(hex!(
                "2222223123123121231231231231232222222231231231212312312312312322"
            )),
            transaction_index: 1,
            effective_gas_price: 1,
        };

        let serialized = transaction.to_value();
        let expected_transaction = transaction;
        let actual_transaction = ExtendedTransaction::from_value(serialized.as_slice());

        assert_eq!(actual_transaction, expected_transaction);
    }
}
