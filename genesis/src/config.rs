use {
    alloy::{genesis::Genesis, primitives::hex},
    aptos_gas_schedule::{InitialGasSchedule, VMGasParameters},
    aptos_vm_types::storage::StorageGasParameters,
    move_core_types::account_address::AccountAddress,
    moved_shared::primitives::B256,
};

pub const CHAIN_ID: u64 = 404;
const DEFAULT_L2_CONTRACT_GENESIS: &str =
    include_str!("../../execution/src/tests/res/l2_genesis_tests.json");

#[derive(Debug, Clone)]
pub struct GasCosts {
    pub vm: VMGasParameters,
    pub storage: StorageGasParameters,
    pub version: u64,
}

#[derive(Debug, Clone)]
pub struct GenesisConfig {
    pub chain_id: u64,
    pub initial_state_root: B256,
    pub gas_costs: GasCosts,
    pub treasury: AccountAddress,
    pub l2_contract_genesis: Genesis,
}

impl Default for GasCosts {
    fn default() -> Self {
        Self {
            vm: VMGasParameters::initial(),
            storage: StorageGasParameters::latest(),
            version: aptos_gas_schedule::LATEST_GAS_FEATURE_VERSION,
        }
    }
}

impl Default for GenesisConfig {
    fn default() -> Self {
        Self {
            chain_id: CHAIN_ID,
            initial_state_root: B256::from(hex!(
                "f292c09656b1ae4eb90f88337a7d8433eeeed18bd961dc9a52e0f15f853c9322"
            )),
            gas_costs: GasCosts::default(),
            treasury: AccountAddress::ONE, // todo: fill in the real address
            l2_contract_genesis: serde_json::from_str(DEFAULT_L2_CONTRACT_GENESIS)
                .expect("Default L2 contract genesis should be JSON encoded `Genesis` struct"),
        }
    }
}
