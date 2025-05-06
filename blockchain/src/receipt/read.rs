use {
    crate::receipt::ExtendedReceipt,
    alloy::rpc::types::TransactionReceipt as AlloyTxReceipt,
    moved_shared::{primitives, primitives::B256},
    std::fmt::Debug,
};

pub trait ReceiptQueries {
    type Err: Debug;
    type Storage;

    fn by_transaction_hash(
        &self,
        storage: &Self::Storage,
        transaction_hash: B256,
    ) -> Result<Option<TransactionReceipt>, Self::Err>;
}

pub type TransactionReceipt = op_alloy::rpc_types::OpTransactionReceipt;

impl From<ExtendedReceipt> for TransactionReceipt {
    fn from(rx: ExtendedReceipt) -> Self {
        let contract_address = rx.contract_address;
        let logs = rx
            .receipt
            .logs()
            .iter()
            .enumerate()
            .map(|(internal_index, log)| alloy::rpc::types::Log {
                inner: log.clone(),
                block_hash: Some(rx.block_hash),
                block_number: Some(rx.block_number),
                block_timestamp: Some(rx.block_timestamp),
                transaction_hash: Some(rx.transaction_hash),
                transaction_index: Some(rx.transaction_index),
                log_index: Some(rx.logs_offset + (internal_index as u64)),
                removed: false,
            })
            .collect();
        let receipt = primitives::with_rpc_logs(&rx.receipt, logs);

        Self {
            inner: AlloyTxReceipt {
                inner: receipt,
                transaction_hash: rx.transaction_hash,
                transaction_index: Some(rx.transaction_index),
                block_hash: Some(rx.block_hash),
                block_number: Some(rx.block_number),
                gas_used: rx.gas_used,
                // TODO: make all gas prices bounded by u128?
                effective_gas_price: rx.l2_gas_price.saturating_to(),
                // Always None because we do not support eip-4844 transactions
                blob_gas_used: None,
                blob_gas_price: None,
                from: rx.from,
                to: rx.to,
                contract_address,
            },
            l1_block_info: rx.l1_block_info.unwrap_or_default(),
        }
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use {super::*, std::convert::Infallible};

    impl ReceiptQueries for () {
        type Err = Infallible;
        type Storage = ();

        fn by_transaction_hash(
            &self,
            _: &Self::Storage,
            _: B256,
        ) -> Result<Option<TransactionReceipt>, Self::Err> {
            Ok(None)
        }
    }
}
