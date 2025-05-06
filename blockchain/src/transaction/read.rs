use {
    crate::transaction::ExtendedTransaction, alloy::consensus::transaction::Recovered,
    moved_shared::primitives::B256, std::fmt::Debug,
};

pub trait TransactionQueries {
    type Err: Debug;
    type Storage;

    fn by_hash(
        &self,
        storage: &Self::Storage,
        hash: B256,
    ) -> Result<Option<TransactionResponse>, Self::Err>;
}

pub type TransactionResponse = op_alloy::rpc_types::Transaction;

impl From<ExtendedTransaction> for TransactionResponse {
    fn from(value: ExtendedTransaction) -> Self {
        let (deposit_nonce, deposit_receipt_version) = value
            .deposit_nonce()
            .map(|nonce| (Some(nonce.nonce), Some(nonce.version)))
            .unwrap_or((None, None));

        let from = value
            .from()
            .expect("Block transactions should contain valid signature");
        Self {
            inner: alloy::rpc::types::eth::Transaction {
                inner: Recovered::new_unchecked(value.inner, from),
                block_hash: Some(value.block_hash),
                block_number: Some(value.block_number),
                transaction_index: Some(value.transaction_index),
                effective_gas_price: Some(value.effective_gas_price),
            },
            deposit_nonce,
            deposit_receipt_version,
        }
    }
}

pub mod in_memory {
    use {
        crate::{
            in_memory::SharedMemoryReader,
            transaction::{TransactionQueries, TransactionResponse},
        },
        moved_shared::primitives::B256,
        std::convert::Infallible,
    };

    #[derive(Debug, Default)]
    pub struct InMemoryTransactionQueries;

    impl InMemoryTransactionQueries {
        pub fn new() -> Self {
            Self
        }
    }

    impl TransactionQueries for InMemoryTransactionQueries {
        type Err = Infallible;
        type Storage = SharedMemoryReader;

        fn by_hash(
            &self,
            storage: &Self::Storage,
            hash: B256,
        ) -> Result<Option<TransactionResponse>, Self::Err> {
            Ok(storage
                .transaction_memory
                .by_hash(hash)
                .map(TransactionResponse::from))
        }
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use {super::*, std::convert::Infallible};

    impl TransactionQueries for () {
        type Err = Infallible;
        type Storage = ();

        fn by_hash(
            &self,
            _: &Self::Storage,
            _: B256,
        ) -> Result<Option<TransactionResponse>, Self::Err> {
            Ok(None)
        }
    }
}
