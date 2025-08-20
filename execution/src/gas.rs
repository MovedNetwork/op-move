use {
    crate::transaction::NormalizedEthTransaction,
    aptos_gas_algebra::{GasExpression, NumBytes},
    aptos_gas_meter::{AptosGasMeter, GasAlgebra, StandardGasAlgebra, StandardGasMeter},
    aptos_gas_schedule::gas_params::natives::aptos_framework::{
        CODE_REQUEST_PUBLISH_BASE, CODE_REQUEST_PUBLISH_PER_BYTE,
    },
    move_core_types::{account_address::AccountAddress, ident_str},
    op_alloy::rpc_types::L1BlockInfo,
    umi_genesis::config::GenesisConfig,
    umi_shared::primitives::U256,
};

pub fn new_gas_meter(
    genesis_config: &GenesisConfig,
    gas_limit: u64,
) -> StandardGasMeter<StandardGasAlgebra> {
    StandardGasMeter::new(StandardGasAlgebra::new(
        genesis_config.gas_costs.version,
        genesis_config.gas_costs.vm.clone(),
        genesis_config.gas_costs.storage.clone(),
        false,
        gas_limit,
    ))
}

pub fn total_gas_used<G: AptosGasMeter>(gas_meter: &G, genesis_config: &GenesisConfig) -> u64 {
    let gas_algebra = gas_meter.algebra();
    // Note: this sum is overflow safe because it uses saturating addition
    // by default in the implementation of `GasQuantity`.
    let total = gas_algebra.execution_gas_used()
        + gas_algebra.io_gas_used()
        + gas_algebra.storage_fee_used_in_gas_units();
    let total: u64 = total.into();
    // Aptos scales up the input gas limit for some reason,
    // so we need to reverse that scaling when we return.
    let scaling_factor: u64 = genesis_config.gas_costs.vm.txn.scaling_factor().into();
    total / scaling_factor
}

pub fn charge_new_module_processing<G: AptosGasMeter>(
    gas_meter: &mut G,
    genesis_config: &GenesisConfig,
    address: &AccountAddress,
    module_size: u64,
) -> Result<(), umi_shared::error::Error> {
    let module_size = NumBytes::new(module_size);

    // Charge for requesting to publish a module
    let publish_request_exp =
        CODE_REQUEST_PUBLISH_BASE + CODE_REQUEST_PUBLISH_PER_BYTE * module_size;
    let publish_request_cost = publish_request_exp.evaluate(
        gas_meter.feature_version(),
        &genesis_config.gas_costs.natives,
    );
    gas_meter
        .algebra_mut()
        .charge_execution(publish_request_cost)
        .map_err(umi_shared::error::Error::from)?;

    // Charge for loading that module into memory
    // Note: the name does not matter because it is not used in the
    // standard gas meter implementation.
    gas_meter
        .charge_dependency(true, address, ident_str!("does_not_matter"), module_size)
        .map_err(umi_shared::error::Error::from)?;

    Ok(())
}

impl NormalizedEthTransaction {
    /// Calculates an amount of Wei per a single unit of gas that is paid on top of the base fee for
    /// this transaction.
    ///
    /// The max fee per gas should be greater than sum of base fee and max priority fee per gas. The
    /// difference is refunded to the user.
    ///
    /// Therefore, the returned value should be max priority fee per gas, also known as "tip" for
    /// validator.
    pub fn tip_per_gas(&self, base_fee: u64) -> u128 {
        let extra_fee = self.max_fee_per_gas.saturating_sub(base_fee.into());
        self.max_priority_fee_per_gas.min(extra_fee)
    }

    pub fn effective_gas_price(&self, base_fee: u64) -> u128 {
        self.tip_per_gas(base_fee).saturating_add(base_fee.into())
    }
}

pub trait L1GasFee {
    fn l1_fee(&self, input: L1GasFeeInput) -> U256;
    fn l1_block_info(&self, input: L1GasFeeInput) -> Option<L1BlockInfo>;
}

pub trait L2GasFee {
    fn l2_fee(&self, input: L2GasFeeInput) -> U256;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct L1GasFeeInput {
    zero_bytes: U256,
    non_zero_bytes: U256,
}

impl L1GasFeeInput {
    pub fn new(zero_bytes: U256, non_zero_bytes: U256) -> Self {
        Self {
            zero_bytes,
            non_zero_bytes,
        }
    }
}

impl<T: AsRef<[u8]>> From<T> for L1GasFeeInput {
    fn from(value: T) -> Self {
        let tx_data = value.as_ref();
        let zero_bytes = U256::from(tx_data.iter().filter(|&&v| v == 0).count());
        let non_zero_bytes = U256::from(tx_data.len()) - zero_bytes;

        Self::new(zero_bytes, non_zero_bytes)
    }
}

/// Transaction-defined parameters necessary for
/// calculation of L2 gas costs.
#[derive(Debug, Clone)]
pub struct L2GasFeeInput {
    pub gas_limit: u64,
    pub effective_gas_price: u128,
}

impl L2GasFeeInput {
    pub fn new(gas_limit: u64, effective_gas_price: u128) -> Self {
        Self {
            gas_limit,
            effective_gas_price,
        }
    }
}

impl From<(u64, u128)> for L2GasFeeInput {
    fn from(value: (u64, u128)) -> Self {
        Self {
            gas_limit: value.0,
            effective_gas_price: value.1,
        }
    }
}

#[derive(Debug)]
pub struct EcotoneGasFee {
    base_fee: U256,
    base_fee_scalar: U256,
    blob_base_fee: U256,
    blob_base_fee_scalar: U256,
}

impl EcotoneGasFee {
    const ZERO_BYTE_MULTIPLIER: U256 = U256::from_limbs([4, 0, 0, 0]);
    const GAS_PRICE_MULTIPLIER: U256 = U256::from_limbs([16, 0, 0, 0]);

    pub fn new(
        base_fee: U256,
        base_fee_scalar: u32,
        blob_base_fee: U256,
        blob_base_fee_scalar: u32,
    ) -> Self {
        Self {
            base_fee,
            base_fee_scalar: U256::from(base_fee_scalar),
            blob_base_fee,
            blob_base_fee_scalar: U256::from(blob_base_fee_scalar),
        }
    }
}

impl L1GasFee for EcotoneGasFee {
    fn l1_fee(&self, input: L1GasFeeInput) -> U256 {
        let zero_bytes = input.zero_bytes;
        let non_zero_bytes = input.non_zero_bytes;
        let tx_compressed_size = (zero_bytes * Self::ZERO_BYTE_MULTIPLIER
            + non_zero_bytes * Self::GAS_PRICE_MULTIPLIER)
            / Self::GAS_PRICE_MULTIPLIER;
        let weighted_gas_price = Self::GAS_PRICE_MULTIPLIER * self.base_fee_scalar * self.base_fee
            + self.blob_base_fee_scalar * self.blob_base_fee;

        tx_compressed_size * weighted_gas_price
    }

    fn l1_block_info(&self, input: L1GasFeeInput) -> Option<L1BlockInfo> {
        Some(L1BlockInfo {
            l1_gas_price: Some(self.base_fee.saturating_to()),
            l1_gas_used: None,
            l1_fee: Some(self.l1_fee(input).saturating_to()),
            l1_fee_scalar: None,
            l1_base_fee_scalar: Some(self.base_fee_scalar.saturating_to()),
            l1_blob_base_fee: Some(self.blob_base_fee.saturating_to()),
            l1_blob_base_fee_scalar: Some(self.blob_base_fee_scalar.saturating_to()),
            // TODO: These fields are at operator discretion and were introduced with
            // Isthmus hard fork (<https://gov.optimism.io/t/upgrade-proposal-15-isthmus-hard-fork/9804>).
            // They are not set by default yet, but might be in the future, so we track it in a separate issue. (#327)
            operator_fee_scalar: None,
            operator_fee_constant: None,
        })
    }
}

/// This struct holds additional parameters and behavior as
/// defined by Umi network for L2 gas calculation that are
/// independent of transaction-defined limits or block state.
#[derive(Debug, Clone)]
pub struct UmiGasFee {
    gas_fee_multiplier: U256,
}

impl L2GasFee for UmiGasFee {
    fn l2_fee(&self, input: L2GasFeeInput) -> U256 {
        U256::from(input.effective_gas_price)
            .saturating_mul(U256::from(input.gas_limit))
            .saturating_mul(self.gas_fee_multiplier)
    }
}

/// Creates algorithm for calculating cost of publishing a transaction to layer-1 blockchain.
pub trait CreateL1GasFee {
    /// Extracts parameters from deposit transaction and creates the algorithm for calculating L1
    /// gas cost.
    fn for_deposit(&self, data: &[u8]) -> impl L1GasFee + 'static;
}

pub struct CreateEcotoneL1GasFee;

impl CreateL1GasFee for CreateEcotoneL1GasFee {
    fn for_deposit(&self, data: &[u8]) -> impl L1GasFee + 'static {
        let l1_base_fee = U256::from_be_slice(&data[36..68]);
        let l1_blob_base_fee = U256::from_be_slice(&data[68..100]);
        let l1_base_fee_scalar =
            u32::from_be_bytes(data[4..8].try_into().expect("Slice should be 4 bytes"));
        let l1_blob_base_fee_scalar =
            u32::from_be_bytes(data[8..12].try_into().expect("Slice should be 4 bytes"));

        EcotoneGasFee::new(
            l1_base_fee,
            l1_base_fee_scalar,
            l1_blob_base_fee,
            l1_blob_base_fee_scalar,
        )
    }
}

pub struct CreateUmiL2GasFee;

/// Creates algorithm for calculating cost of publishing a transaction to layer-2 blockchain.
pub trait CreateL2GasFee {
    const DEFAULT_L2_GAS_MULTIPLIER: U256 = U256::from_limbs([1, 0, 0, 0]);
    /// Instantiates L2 gas fee structure with a given multiplier. Basically a decoupled
    /// constructor.
    fn with_gas_fee_multiplier(&self, gas_fee_multiplier: U256) -> impl L2GasFee + 'static + Clone;

    fn with_default_gas_fee_multiplier(&self) -> impl L2GasFee + 'static + Clone {
        self.with_gas_fee_multiplier(Self::DEFAULT_L2_GAS_MULTIPLIER)
    }
}

impl CreateL2GasFee for CreateUmiL2GasFee {
    fn with_gas_fee_multiplier(&self, gas_fee_multiplier: U256) -> impl L2GasFee + 'static + Clone {
        UmiGasFee { gas_fee_multiplier }
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod tests {
    use super::*;

    impl L1GasFee for U256 {
        fn l1_fee(&self, _input: L1GasFeeInput) -> U256 {
            *self
        }

        fn l1_block_info(&self, _input: L1GasFeeInput) -> Option<L1BlockInfo> {
            None
        }
    }

    impl L2GasFee for U256 {
        fn l2_fee(&self, _input: L2GasFeeInput) -> U256 {
            *self
        }
    }

    impl CreateL1GasFee for U256 {
        fn for_deposit(&self, _data: &[u8]) -> impl L1GasFee + 'static {
            *self
        }
    }

    impl CreateL2GasFee for U256 {
        fn with_gas_fee_multiplier(&self, _base_fee: U256) -> impl L2GasFee + 'static + Clone {
            *self
        }
    }
}
