use {
    alloy::eips::eip4895::Withdrawal,
    sha2::{Digest, Sha256},
    umi_shared::primitives::{Address, B256, U64},
};

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
}

impl<'a> NewPayloadIdInput<'a> {
    /// Creates payload ID input parameters with `parent`, `timestamp`, `random` and `fee_recipient`
    /// and omits `withdrawals` and `beacon_root`.
    ///
    /// Marks `version` as `3`.
    pub fn new_v3(
        parent: &'a B256,
        timestamp: u64,
        random: &'a B256,
        fee_recipient: &'a Address,
    ) -> Self {
        Self {
            parent,
            timestamp,
            random,
            fee_recipient,
            withdrawals: Vec::new(),
            beacon_root: None,
            version: 3,
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

    #[test_case(b256_0_ended!(1u8), 1, b256_0_ended!(1u8), addr_0_ended!(1u8), [], 0x004cffc0e01f12fau64; "All ones")]
    #[test_case(b256_0_ended!(2u8), 1, b256_0_ended!(1u8), addr_0_ended!(1u8), [], 0x00fda8bfe79f5f1bu64; "Different parent")]
    #[test_case(b256_0_ended!(2u8), 2, b256_0_ended!(1u8), addr_0_ended!(1u8), [], 0x00410bd3dc768689u64; "Different timestamp")]
    #[test_case(b256_0_ended!(2u8), 2, b256_0_ended!(2u8), addr_0_ended!(1u8), [], 0x0040399b0c29a27fu64; "Different random")]
    #[test_case(b256_0_ended!(2u8), 2, b256_0_ended!(2u8), addr_0_ended!(2u8), [], 0x0024950cf11b41b5u64; "Different fee recipient")]
    #[test_case(b256_0_ended!(2u8), 2, b256_0_ended!(2u8), addr_0_ended!(2u8), [withdrawal!(0)], 0x00d1a6974d7595ccu64; "With withdrawals")]
    #[test_case(b256_0_ended!(2u8), 2, b256_0_ended!(2u8), addr_0_ended!(2u8), [withdrawal!(2)], 0x0070e1a339c8ed47u64; "Different withdrawals")]
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
        });
        let expected_payload_id = PayloadId::from(expected_payload_id);

        assert_eq!(actual_payload_id, expected_payload_id,);
    }
}
