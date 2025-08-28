//! This module is concerned about calculating fees charged for gas usage.

use {
    alloy::primitives::{Bytes, U64},
    std::cmp::Ordering,
};

pub const DEFAULT_EIP1559_ELASTICITY_MULTIPLIER: u64 = 6;
pub const DEFAULT_EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR: u128 = 250;

/// Determines amount of fees charged per gas used in transaction execution.
///
/// The base fee applies to the entire block and does not include tips for validators.
/// Does not take into account any priority fees.
pub trait BaseGasFee {
    /// Calculates base fee per gas for a block based on the parent block.
    ///
    /// The concrete formula applied depends on the implementation.
    fn base_fee_per_gas(
        &self,
        parent_gas_limit: u64,
        parent_gas_used: u64,
        parent_base_fee_per_gas: u64,
    ) -> u64;

    fn set_parameters_from_attrs(&mut self, header_extra_data: U64);
    fn encode_parameters_for_header(&self) -> Bytes;
}

/// Calculates base fee per gas according to the Ethereum model based on EIP-1559.
///
/// The formula works in these steps:
/// 1. Calculate the *gas target*. The *gas target* is less than or equal to *gas limit*.
/// 2. Compare the *gas target* to *gas used* of the parent block.
/// 3. If *gas used* exceeds *gas target*, the fee is increased.
/// 4. If *gas used* falls below *gas target*, the fee is decreased.
/// 5. Otherwise, the fee is not changed.
///
/// The greater the difference between *gas used* and *gas target*, the greater the increase or
/// decrease of the fee.
///
/// The formula can be controlled by the given parameters. Their effect is:
/// * The greater the `elasticity_multiplier`, the smaller the *gas target*.
/// * The greater the `base_fee_max_change_denominator`, the smaller the increase or decrease of
///   the fee.
#[derive(Debug, Clone)]
pub struct Eip1559GasFee {
    /// Magnifies the difference between target gas amount and limit. Here are some facts about this
    /// parameter:
    ///
    /// * The greater the value the smaller the target gas.
    /// * This value has to be greater than zero.
    /// * A value of 1 makes the target the same as the limit.
    elasticity_multiplier: u64,
    /// Reduces the difference between block's base fee per gas and its parent. Some properties can
    /// be observed:
    ///
    /// * The greater the value the smaller the increase or decrease of the base fee per gas.
    /// * This value has to be greater than zero.
    /// * A value of 1 makes the greatest fee increases or decreases.
    base_fee_max_change_denominator: u128,
}

impl Eip1559GasFee {
    /// Sets up the base fee per gas calculation with given parameters.
    ///
    /// # Panics
    /// If either `elasticity_multiplier` or `base_fee_max_change_denominator` is zero.
    pub fn new(elasticity_multiplier: u64, base_fee_max_change_denominator: u128) -> Self {
        assert!(
            elasticity_multiplier > 0,
            "Supplied `elasticity_multiplier` was 0"
        );
        assert!(
            base_fee_max_change_denominator > 0,
            "Supplied `base_fee_max_change_denominator` was 0"
        );

        Self {
            elasticity_multiplier,
            base_fee_max_change_denominator,
        }
    }
}

impl BaseGasFee for Eip1559GasFee {
    fn base_fee_per_gas(
        &self,
        parent_gas_limit: u64,
        parent_gas_used: u64,
        parent_base_fee_per_gas: u64,
    ) -> u64 {
        // Bump up to 128 bits for calculation to avoid overflows.
        let parent_base_fee_per_gas: u128 = parent_base_fee_per_gas.into();
        let gas_target = parent_gas_limit / self.elasticity_multiplier;

        let result = match parent_gas_used.cmp(&gas_target) {
            Ordering::Greater => {
                let delta = (parent_base_fee_per_gas
                    .saturating_mul(u128::from(parent_gas_used - gas_target))
                    / u128::from(gas_target)
                    / self.base_fee_max_change_denominator)
                    .max(1);

                parent_base_fee_per_gas.saturating_add(delta)
            }
            Ordering::Less => {
                let delta = parent_base_fee_per_gas
                    .saturating_mul(u128::from(gas_target - parent_gas_used))
                    / u128::from(gas_target)
                    / self.base_fee_max_change_denominator;

                parent_base_fee_per_gas.saturating_sub(delta)
            }
            Ordering::Equal => parent_base_fee_per_gas,
        };
        result.try_into().unwrap_or(u64::MAX)
    }

    fn set_parameters_from_attrs(&mut self, eip1559_params: U64) {
        let denom = eip1559_params.wrapping_shr(32).saturating_to::<u32>();
        let elasticity = (eip1559_params.bitand(U64::from(0xFFFF_FFFFu64))).saturating_to::<u32>();

        if elasticity != 0 {
            assert!(
                denom != 0,
                "Holocene gas parameters should only have a 0 denominator if elasticity is also 0"
            );
        }

        self.elasticity_multiplier = elasticity as u64;
        self.base_fee_max_change_denominator = denom as u128;
    }

    fn encode_parameters_for_header(&self) -> Bytes {
        let mut out = Vec::with_capacity(9);

        // Header `extra_data` should be prepended with a 0 version byte
        out.extend_from_slice(&[0u8]);

        out.extend_from_slice(&(self.base_fee_max_change_denominator as u32).to_be_bytes());
        out.extend_from_slice(&(self.elasticity_multiplier as u32).to_be_bytes());

        out.into()
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use super::*;

    const ELASTICITY_MULTIPLIER: u64 = 2;
    const BASE_FEE_MAX_CHANGE_DENOMINATOR: u128 = 8;

    impl Default for Eip1559GasFee {
        fn default() -> Self {
            Self::new(ELASTICITY_MULTIPLIER, BASE_FEE_MAX_CHANGE_DENOMINATOR)
        }
    }

    impl Eip1559GasFee {
        /// Creates a new [`Eip1559GasFee`] that always makes the gas target equal to gas limit.
        pub fn with_max_gas_target(mut self) -> Self {
            self.elasticity_multiplier = 1;
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_is_not_changed_when_gas_used_matches_gas_target() {
        let gas_limit = 15_000_000;
        let parent_fee = 1;

        let actual_fee = Eip1559GasFee::default()
            .with_max_gas_target()
            .base_fee_per_gas(gas_limit, gas_limit, parent_fee);

        assert_eq!(actual_fee, parent_fee);
    }

    #[test]
    fn test_fee_is_increased_when_gas_used_exceeds_gas_target() {
        let gas_limit = 15_000_000;
        let gas_used = 8_500_000;
        let parent_fee = 2;

        let actual_fee = Eip1559GasFee::default().base_fee_per_gas(gas_limit, gas_used, parent_fee);

        assert!(actual_fee > parent_fee, "{actual_fee} > {parent_fee}");
    }

    #[test]
    fn test_fee_is_decreased_when_gas_used_falls_below_gas_target() {
        let gas_limit = 15_000_000;
        let gas_used = 6_500_000;
        let parent_fee = 200;

        let actual_fee = Eip1559GasFee::default().base_fee_per_gas(gas_limit, gas_used, parent_fee);

        assert!(actual_fee < parent_fee, "{actual_fee} < {parent_fee}");
    }
}
