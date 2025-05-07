use {
    crate::transaction::{TransactionQueries, TransactionResponse},
    alloy::eips::eip2718::Encodable2718,
    moved_shared::primitives::{B256, U256},
    op_alloy::consensus::{OpTxEnvelope, TxDeposit},
    std::{convert::Infallible, fmt::Debug},
};

#[derive(Debug, Clone, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExtendedTransaction {
    pub inner: OpTxEnvelope,
    pub block_number: u64,
    pub block_hash: B256,
    pub transaction_index: u64,
    pub effective_gas_price: u128,
}

impl ExtendedTransaction {
    pub fn new(
        effective_gas_price: u128,
        inner: OpTxEnvelope,
        block_number: u64,
        block_hash: B256,
        transaction_index: u64,
    ) -> Self {
        Self {
            effective_gas_price,
            inner,
            block_number,
            block_hash,
            transaction_index,
        }
    }

    pub fn from(
        &self,
    ) -> Result<moved_shared::primitives::Address, alloy::primitives::SignatureError> {
        match self.inner() {
            OpTxEnvelope::Legacy(tx) => tx.recover_signer(),
            OpTxEnvelope::Eip1559(tx) => tx.recover_signer(),
            OpTxEnvelope::Eip2930(tx) => tx.recover_signer(),
            OpTxEnvelope::Eip7702(tx) => tx.recover_signer(),
            OpTxEnvelope::Deposit(tx) => Ok(tx.from),
        }
    }

    pub fn inner(&self) -> &OpTxEnvelope {
        &self.inner
    }

    pub fn hash(&self) -> B256 {
        self.inner.trie_hash()
    }

    pub fn deposit_nonce(&self) -> Option<VersionedNonce> {
        if let OpTxEnvelope::Deposit(tx) = self.inner() {
            inner_get_deposit_nonce(tx)
        } else {
            None
        }
    }
}

/// Nonce and version for messages of `CrossDomainMessenger` L2 contract.
pub struct VersionedNonce {
    pub version: u64,
    pub nonce: u64,
}

fn inner_get_deposit_nonce(tx: &TxDeposit) -> Option<VersionedNonce> {
    use alloy::sol_types::SolType;

    // Function selector for `relayMessage`.
    // See optimism/packages/contracts-bedrock/src/universal/CrossDomainMessenger.sol
    const RELAY_MESSAGE_SELECTOR: [u8; 4] = [0xd7, 0x64, 0xad, 0x0b];

    // The upper 16 bits are for the version, the rest are for the nonce.
    const NONCE_MASK: U256 = U256::from_be_bytes(alloy::hex!(
        "0000ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    ));

    alloy::sol! {
        struct RelayMessageArgs {
            uint256 nonce;
            address sender;
            address target;
            uint256 value;
            uint256 min_gas_limit;
            bytes message;
        }
    }

    if !tx.input.starts_with(&RELAY_MESSAGE_SELECTOR) {
        return None;
    }

    let args = RelayMessageArgs::abi_decode_params(&tx.input[4..], true).ok()?;

    // See optimism/packages/contracts-bedrock/src/libraries/Encoding.sol
    let encoded_versioned_nonce = args.nonce;
    let version = encoded_versioned_nonce.checked_shr(240)?.saturating_to();
    let nonce = (encoded_versioned_nonce & NONCE_MASK).saturating_to();
    Some(VersionedNonce { version, nonce })
}

pub trait TransactionRepository {
    type Err: Debug;
    type Storage;

    fn extend(
        &mut self,
        storage: &mut Self::Storage,
        transactions: impl IntoIterator<Item = ExtendedTransaction>,
    ) -> Result<(), Self::Err>;
}

pub mod in_memory {
    use {
        crate::{
            in_memory::SharedMemory,
            transaction::{ExtendedTransaction, TransactionRepository},
        },
        std::convert::Infallible,
    };

    #[derive(Debug, Clone, Default)]
    pub struct InMemoryTransactionRepository;

    impl InMemoryTransactionRepository {
        pub fn new() -> Self {
            Self
        }
    }

    impl TransactionRepository for InMemoryTransactionRepository {
        type Err = Infallible;
        type Storage = SharedMemory;

        fn extend(
            &mut self,
            storage: &mut Self::Storage,
            transactions: impl IntoIterator<Item = ExtendedTransaction>,
        ) -> Result<(), Self::Err> {
            storage.transaction_memory.extend(transactions);
            Ok(())
        }
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use super::*;

    impl TransactionRepository for () {
        type Err = Infallible;
        type Storage = ();

        fn extend(
            &mut self,
            _: &mut Self::Storage,
            _: impl IntoIterator<Item = ExtendedTransaction>,
        ) -> Result<(), Self::Err> {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_deposit_nonce_from_encoded_input() {
        const INPUT: [u8; 420] = alloy::hex!(
            "d764ad0b0001000000000000000000000000000000000000000000000000000000000002000000000000000000000000c8088d0362bb4ac757ca77e211c30503d39cef4800000000000000000000000042000000000000000000000000000000000000100000000000000000000000000000000000000000000000056bc75e2d631000000000000000000000000000000000000000000000000000000000000000030d4000000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000000a41635f5fd000000000000000000000000c152ff76a513e15be1be43d102a881f076e707b3000000000000000000000000c152ff76a513e15be1be43d102a881f076e707b30000000000000000000000000000000000000000000000056bc75e2d631000000000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
        );

        let tx = TxDeposit {
            input: INPUT.into(),
            ..Default::default()
        };
        let VersionedNonce { version, nonce } = inner_get_deposit_nonce(&tx).unwrap();
        assert_eq!(nonce, 2);
        assert_eq!(version, 1);
    }
}
