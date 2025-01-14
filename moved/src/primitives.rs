pub use alloy::primitives::{aliases::B2048, Address, Bytes, B256, B64, U256, U64};
use {
    alloy::consensus::{Receipt, ReceiptWithBloom},
    move_core_types::{account_address::AccountAddress, u256::U256 as MoveU256},
    op_alloy::consensus::{OpDepositReceipt, OpDepositReceiptWithBloom, OpReceiptEnvelope},
};

pub(crate) trait ToEthAddress {
    fn to_eth_address(&self) -> Address;
}

impl ToEthAddress for AccountAddress {
    fn to_eth_address(&self) -> Address {
        let bytes = &self.as_slice()[12..];
        let bytes = bytes.try_into().expect("Slice should be 20 bytes");
        Address::new(bytes)
    }
}

pub trait ToMoveAddress {
    fn to_move_address(&self) -> AccountAddress;
}

impl<T: AsRef<[u8; 20]>> ToMoveAddress for T {
    fn to_move_address(&self) -> AccountAddress {
        // TODO: is there a way to make Move use 20-byte addresses?
        let mut bytes = [0; 32];
        bytes[12..32].copy_from_slice(self.as_ref());
        AccountAddress::new(bytes)
    }
}

pub struct KeyHash(pub B256);

pub trait KeyHashable {
    fn key_hash(&self) -> KeyHash;
}

pub trait ToU64 {
    fn to_u64(self) -> u64;
}

impl ToU64 for U64 {
    fn to_u64(self) -> u64 {
        self.into_limbs()[0]
    }
}

pub(crate) trait ToSaturatedU64 {
    fn to_saturated_u64(self) -> u64;
}

impl ToSaturatedU64 for U256 {
    fn to_saturated_u64(self) -> u64 {
        match self.into_limbs() {
            [value, 0, 0, 0] => value,
            _ => u64::MAX,
        }
    }
}

pub(crate) trait ToU256 {
    fn to_u256(self) -> U256;
}

impl ToU256 for MoveU256 {
    fn to_u256(self) -> U256 {
        U256::from_le_bytes(self.to_le_bytes())
    }
}

pub(crate) trait ToMoveU256 {
    fn to_move_u256(self) -> MoveU256;
}

impl ToMoveU256 for B256 {
    fn to_move_u256(self) -> MoveU256 {
        MoveU256::from_le_bytes(&self.0)
    }
}

impl ToMoveU256 for U256 {
    fn to_move_u256(self) -> MoveU256 {
        MoveU256::from_le_bytes(&self.to_le_bytes())
    }
}

pub fn with_rpc_logs(
    receipt: &OpReceiptEnvelope,
    logs: Vec<alloy::rpc::types::Log>,
) -> OpReceiptEnvelope<alloy::rpc::types::Log> {
    match receipt {
        OpReceiptEnvelope::Legacy(receipt_with_bloom) => {
            OpReceiptEnvelope::Legacy(ReceiptWithBloom {
                receipt: Receipt {
                    status: receipt_with_bloom.receipt.status,
                    cumulative_gas_used: receipt_with_bloom.receipt.cumulative_gas_used,
                    logs,
                },
                logs_bloom: receipt_with_bloom.logs_bloom,
            })
        }
        OpReceiptEnvelope::Eip2930(receipt_with_bloom) => {
            OpReceiptEnvelope::Eip2930(ReceiptWithBloom {
                receipt: Receipt {
                    status: receipt_with_bloom.receipt.status,
                    cumulative_gas_used: receipt_with_bloom.receipt.cumulative_gas_used,
                    logs,
                },
                logs_bloom: receipt_with_bloom.logs_bloom,
            })
        }
        OpReceiptEnvelope::Eip1559(receipt_with_bloom) => {
            OpReceiptEnvelope::Eip1559(ReceiptWithBloom {
                receipt: Receipt {
                    status: receipt_with_bloom.receipt.status,
                    cumulative_gas_used: receipt_with_bloom.receipt.cumulative_gas_used,
                    logs,
                },
                logs_bloom: receipt_with_bloom.logs_bloom,
            })
        }
        OpReceiptEnvelope::Deposit(op_deposit_receipt_with_bloom) => {
            OpReceiptEnvelope::Deposit(OpDepositReceiptWithBloom {
                receipt: OpDepositReceipt {
                    inner: Receipt {
                        status: op_deposit_receipt_with_bloom.receipt.inner.status,
                        cumulative_gas_used: op_deposit_receipt_with_bloom
                            .receipt
                            .inner
                            .cumulative_gas_used,
                        logs,
                    },
                    deposit_nonce: op_deposit_receipt_with_bloom.receipt.deposit_nonce,
                    deposit_receipt_version: op_deposit_receipt_with_bloom
                        .receipt
                        .deposit_receipt_version,
                },
                logs_bloom: op_deposit_receipt_with_bloom.logs_bloom,
            })
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::{address, hex},
        test_case::test_case,
    };

    #[test]
    fn test_move_from_eth_address_match_at_intersection() {
        let eth_address = address!("ffffffffffffffffffffffffffffffffffffffff");
        let move_address = eth_address.to_move_address();
        let actual = &move_address.into_bytes()[12..];
        let expected = &hex!("ffffffffffffffffffffffffffffffffffffffff");

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_eth_from_move_address_match_at_intersection() {
        let move_address: AccountAddress =
            hex!("000000000000000000000000ffffffffffffffffffffffffffffffffffffffff").into();
        let eth_address = move_address.to_eth_address();
        let actual = eth_address.as_slice();
        let expected = &hex!("ffffffffffffffffffffffffffffffffffffffff");

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_eth_from_move_address_with_only_zero_bits_outside_intersection_and_back_produces_identical_value(
    ) {
        let expected_move_address: AccountAddress =
            hex!("000000000000000000000000ffffffffffffffffffffffffffffffffffffffff").into();
        let eth_address = expected_move_address.to_eth_address();
        let actual_move_address = eth_address.to_move_address();

        assert_eq!(actual_move_address, expected_move_address);
    }

    #[test]
    fn test_eth_from_move_address_with_non_zero_bits_outside_intersection_and_back_produces_different_value(
    ) {
        let expected_move_address: AccountAddress =
            hex!("100000000000000000000000ffffffffffffffffffffffffffffffffffffffff").into();
        let eth_address = expected_move_address.to_eth_address();
        let actual_move_address = eth_address.to_move_address();

        assert_ne!(actual_move_address, expected_move_address);
    }

    #[test]
    fn test_eth_address_to_move_address_round_trip() {
        // All Ethereum addresses should map to Move addresses, which
        // then map back to the same Ethereum address.

        let eth_address = {
            // Generate a random address based on the system time.
            use std::time::SystemTime;
            let ns = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let bytes = alloy::primitives::keccak256(ns.to_be_bytes());
            Address::from_word(bytes)
        };
        let move_address = eth_address.to_move_address();
        let rt_eth_address = move_address.to_eth_address();

        assert_eq!(
            eth_address, rt_eth_address,
            "Round trip eth to move address must cause no change"
        );
    }

    #[test_case(U256::from_limbs([4, 4, 0, 0]), u64::MAX; "U256 number greater than 2^64 - 1")]
    #[test_case(U256::from_limbs([4, 0, 0, 0]), 4; "U256 number less than 2^64 - 1")]
    fn test_converting_to_saturated_u64_saturates_at_numerical_bound(
        n: impl ToSaturatedU64,
        expected: u64,
    ) {
        let actual = n.to_saturated_u64();

        assert_eq!(actual, expected);
    }
}
