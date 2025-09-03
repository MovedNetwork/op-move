use {
    alloy::eips::eip4895::Withdrawal,
    sha2::{Digest, Sha256},
    std::convert::identity,
    umi_shared::primitives::{Address, B256, U64},
};

#[cfg(feature = "op-upgrade")]
use crate::block::BaseFeeParameters;

pub type PayloadId = U64;

/// The payload ID algorithm arguments.
///
/// See trait [`NewPayloadId`] for the definition of the Payload ID creation behavior.
#[derive(Debug)]
pub struct NewPayloadIdInput<'a> {
    parent: &'a B256,
    timestamp: u64,
    random: &'a B256,
    fee_recipient: &'a Address,
    withdrawals: Vec<Withdrawal>,
    beacon_root: Option<&'a B256>,
    version: u8,
    transactions: Option<Vec<B256>>,
    no_tx_pool: Option<bool>,
    gas_limit: u64,
    #[cfg(feature = "op-upgrade")]
    eip1559_params: Option<u64>,
}

impl<'a> NewPayloadIdInput<'a> {
    /// Creates payload ID input parameters with `parent`, `timestamp`, `random`, `fee_recipient`
    /// and `gas_limit` and omits `withdrawals` and `beacon_root`.
    ///
    /// Marks `version` as `3`.
    pub fn new_v3(
        parent: &'a B256,
        timestamp: u64,
        random: &'a B256,
        fee_recipient: &'a Address,
        gas_limit: u64,
    ) -> Self {
        Self {
            parent,
            timestamp,
            random,
            fee_recipient,
            withdrawals: Vec::new(),
            beacon_root: None,
            version: 3,
            gas_limit,
            transactions: None,
            no_tx_pool: None,
            #[cfg(feature = "op-upgrade")]
            eip1559_params: None,
        }
    }

    /// Creates this input with `withdrawals`.
    pub fn with_withdrawals(
        mut self,
        withdrawals: impl IntoIterator<Item = impl Into<Withdrawal>>,
    ) -> Self {
        self.withdrawals = withdrawals.into_iter().map(Into::into).collect();
        self
    }

    /// Creates this input with `beacon_root`.
    pub fn with_beacon_root(mut self, beacon_root: &'a B256) -> Self {
        self.beacon_root.replace(beacon_root);
        self
    }

    /// Creates this input with `transactions`.
    pub fn with_transaction_hashes(mut self, tx_hashes: impl Iterator<Item = B256>) -> Self {
        self.transactions = Some(tx_hashes.collect());
        self
    }

    /// Creates this input with `eip1559_params`.
    #[cfg(feature = "op-upgrade")]
    pub fn with_eip1559_params(mut self, gas_params: &BaseFeeParameters) -> Self {
        let mut buf = Vec::with_capacity(8);
        buf.extend_from_slice(&gas_params.denominator.to_be_bytes());
        buf.extend_from_slice(&gas_params.elasticity.to_be_bytes());
        self.eip1559_params = Some(u64::from_be_bytes(
            buf.try_into().expect("Slice should be 8 bytes"),
        ));
        self
    }
}

/// Creates payload IDs.
///
/// This trait is defined by a single operation [`Self::new_payload_id`].
pub trait NewPayloadId {
    /// Creates new payload ID.
    ///
    /// The function is deterministic and idempotent. Meaning that calls with the same arguments
    /// provide the same result and repeated calls with the same arguments does not change the
    /// output.
    fn new_payload_id(&self, input: NewPayloadIdInput) -> PayloadId;
}

/// The implementation of node Payload ID creation algorithm by [`op-move`] domain.
#[derive(Debug)]
pub struct StatePayloadId;

impl NewPayloadId for StatePayloadId {
    fn new_payload_id(&self, input: NewPayloadIdInput) -> PayloadId {
        let mut hasher = Sha256::new();
        hasher.update(input.parent.as_slice());
        hasher.update(input.timestamp.to_be_bytes());
        hasher.update(input.random.as_slice());
        hasher.update(input.fee_recipient.0.as_slice());
        let mut buffer =
            Vec::with_capacity(input.withdrawals.len() * std::mem::size_of::<Withdrawal>());
        alloy::rlp::encode_list(&input.withdrawals, &mut buffer);
        hasher.update(buffer);
        if let Some(beacon_root) = input.beacon_root {
            hasher.update(beacon_root.as_slice());
        }
        if input.no_tx_pool.is_some_and(identity)
            || input
                .transactions
                .as_ref()
                .is_some_and(|txs| !txs.is_empty())
        {
            if let Some(no_tx_pool) = input.no_tx_pool {
                hasher.update([if no_tx_pool { 1 } else { 0 }]);
            }
            if let Some(txhs) = &input.transactions {
                let n = txhs.len() as u64;
                hasher.update(n.to_be_bytes());
                for txh in txhs {
                    hasher.update(txh.as_slice());
                }
            }
        }
        hasher.update(input.gas_limit.to_be_bytes());

        #[cfg(feature = "op-upgrade")]
        if let Some(eip1559_params) = input.eip1559_params {
            hasher.update(eip1559_params.to_be_bytes());
        }

        let mut hash = hasher.finalize();
        hash[0] = input.version;

        PayloadId::from(u64::from_be_bytes(
            hash[..8].try_into().expect("Slice should be 8-bytes"),
        ))
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use super::*;

    impl NewPayloadId for u64 {
        fn new_payload_id(&self, _input: NewPayloadIdInput) -> PayloadId {
            PayloadId::from(*self)
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, test_case::test_case};

    macro_rules! b256_0_ended {
        ($x: expr) => {
            B256::from([
                $x, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0,
            ])
        };
    }

    macro_rules! addr_0_ended {
        ($x: expr) => {
            Address::from([$x, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
        };
    }

    macro_rules! withdrawal {
        ($index: expr) => {{
            Withdrawal {
                index: $index,
                validator_index: Default::default(),
                address: Default::default(),
                amount: Default::default(),
            }
        }};
    }

    #[test_case(b256_0_ended!(1u8), 1, b256_0_ended!(1u8), addr_0_ended!(1u8), [], 0xa86df803a6df64_u64; "All ones")]
    #[test_case(b256_0_ended!(2u8), 1, b256_0_ended!(1u8), addr_0_ended!(1u8), [], 0x3a7a6b83f6a5d_u64; "Different parent")]
    #[test_case(b256_0_ended!(2u8), 2, b256_0_ended!(1u8), addr_0_ended!(1u8), [], 0x6d27f7b07c7a27_u64; "Different timestamp")]
    #[test_case(b256_0_ended!(2u8), 2, b256_0_ended!(2u8), addr_0_ended!(1u8), [], 0x368b379833708e_u64; "Different random")]
    #[test_case(b256_0_ended!(2u8), 2, b256_0_ended!(2u8), addr_0_ended!(2u8), [], 0xd5f4a4c9eddd5b_u64; "Different fee recipient")]
    #[test_case(b256_0_ended!(2u8), 2, b256_0_ended!(2u8), addr_0_ended!(2u8), [withdrawal!(0)], 0x5453403020d1e6_u64; "With withdrawals")]
    #[test_case(b256_0_ended!(2u8), 2, b256_0_ended!(2u8), addr_0_ended!(2u8), [withdrawal!(2)], 0x92629a69dd019d_u64; "Different withdrawals")]
    fn test_new_payload_id_creates_deterministic_id(
        parent: B256,
        timestamp: u64,
        random: B256,
        fee_recipient: Address,
        withdrawals: impl IntoIterator<Item = Withdrawal>,
        expected_payload_id: u64,
    ) {
        let actual_payload_id = StatePayloadId.new_payload_id(NewPayloadIdInput {
            parent: &parent,
            timestamp,
            random: &random,
            fee_recipient: &fee_recipient,
            withdrawals: withdrawals.into_iter().collect(),
            beacon_root: None,
            version: 0,
            transactions: None,
            no_tx_pool: None,
            gas_limit: 0,
            #[cfg(feature = "op-upgrade")]
            eip1559_params: None,
        });
        let expected_payload_id = PayloadId::from(expected_payload_id);

        assert_eq!(actual_payload_id, expected_payload_id,);
    }
}
