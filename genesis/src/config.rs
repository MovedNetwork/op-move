use {
    crate::bridged_tokens::{self, BridgedToken},
    alloy::{genesis::Genesis, primitives::hex},
    aptos_gas_schedule::{InitialGasSchedule, NativeGasParameters, VMGasParameters},
    aptos_vm_types::storage::StorageGasParameters,
    move_core_types::{account_address::AccountAddress, gas_algebra::GasQuantity},
    moved_shared::primitives::B256,
    std::path::Path,
};

pub const CHAIN_ID: u64 = 404;
const DEFAULT_L2_CONTRACT_GENESIS: &str =
    include_str!("../../execution/src/tests/res/l2_genesis_tests.json");

// We're setting the scale factor lower than Aptos because we want
// our gas costs to align with expected values for EVM chains.
// For example, a simple token transfer should be around 21_000 gas.
const EXTERNAL_GAS_SCALE_FACTOR: u64 = 600;

// If we spend around 10k internal gas units per ms of computation
// as the Aptos code comments suggest
// (https://github.com/aptos-labs/aptos-core/blob/aptos-node-v1.27.2/aptos-move/aptos-gas-schedule/src/gas_schedule/transaction.rs#L212)
// then this is around 5 minutes worth of computation.
// However, with the current EVM conversion of 600 internal gas units
// per EVM gas unit, this is only 167 ms of computation.
// I'm not sure which is right, benchmarks are needed.
const INTERNAL_EXECUTION_LIMIT: u64 = 3_000_000_000;

// This is the amount of gas we charge to validate a transaction and thus
// is the minimum amount of gas a transaction must attach to be executed.
// Note: this is in internal gas units; the external gas units value is
// obtained by dividing this value by the `EXTERNAL_GAS_SCALE_FACTOR`.
const TRANSACTION_BASE_COST: u64 = 11_400_000;

#[derive(Debug, Clone)]
pub struct GasCosts {
    pub vm: VMGasParameters,
    pub storage: StorageGasParameters,
    pub natives: NativeGasParameters,

    pub version: u64,
}

#[derive(Debug, Clone)]
pub struct GenesisConfig {
    pub chain_id: u64,
    pub initial_state_root: B256,
    pub gas_costs: GasCosts,
    pub treasury: AccountAddress,
    pub l2_contract_genesis: Genesis,
    /// Superchain Token List data.
    pub token_list: Vec<BridgedToken>,
}

impl Default for GasCosts {
    fn default() -> Self {
        let mut result = Self {
            vm: VMGasParameters::initial(),
            storage: StorageGasParameters::latest(),
            natives: NativeGasParameters::initial(),
            version: aptos_gas_schedule::LATEST_GAS_FEATURE_VERSION,
        };
        result.vm.txn.gas_unit_scaling_factor = GasQuantity::new(EXTERNAL_GAS_SCALE_FACTOR);
        result.vm.txn.max_execution_gas = GasQuantity::new(INTERNAL_EXECUTION_LIMIT);
        result.vm.txn.min_transaction_gas_units = GasQuantity::new(TRANSACTION_BASE_COST);
        result
    }
}

impl Default for GenesisConfig {
    fn default() -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("execution/src/tests/res/bridged_tokens_test.json");
        Self {
            chain_id: CHAIN_ID,
            initial_state_root: B256::from(hex!(
                "0xb97674627a3557d273abcd3d80fd01a3bbc0d52c5ebd7e6897f06f5ac97a4e27"
            )),
            gas_costs: GasCosts::default(),
            treasury: AccountAddress::ONE, // todo: fill in the real address
            l2_contract_genesis: serde_json::from_str(DEFAULT_L2_CONTRACT_GENESIS)
                .expect("Default L2 contract genesis should be JSON encoded `Genesis` struct"),
            token_list: bridged_tokens::parse_token_list(&path).expect("Tokens list should parse"),
        }
    }
}

#[test]
fn test_default_token_list() {
    let config = GenesisConfig::default();

    // - `USD Coin` has an override to `Bridged USDC`,
    // - DAI is skipped because it uses a non-standard bridge,
    // - `Aave Token` includes no overrides.
    let expected_names = ["Bridged USDC", "Aave Token"];

    assert_eq!(config.token_list.len(), expected_names.len());
    for (token, expected_name) in config.token_list.into_iter().zip(expected_names) {
        assert_eq!(token.name, expected_name);
    }
}
