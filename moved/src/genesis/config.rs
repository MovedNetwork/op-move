use {
    crate::primitives::B256,
    alloy::primitives::hex,
    aptos_gas_schedule::{InitialGasSchedule, VMGasParameters},
    aptos_vm_types::storage::StorageGasParameters,
    move_core_types::account_address::AccountAddress,
    std::path::{Path, PathBuf},
};

pub const CHAIN_ID: u64 = 404;

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
    // TODO: the genesis config should be self-contained instead of referring to an external file.
    pub l2_contract_genesis: PathBuf,
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
                "f7b355b8c91be64a8c171c711217f734d48c54ed4a2052da11be7960365a50a4"
            )),
            gas_costs: GasCosts::default(),
            treasury: AccountAddress::ONE, // todo: fill in the real address
            l2_contract_genesis: Path::new("../moved/src/tests/res/l2_genesis_tests.json").into(),
        }
    }
}
