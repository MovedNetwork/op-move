use {
    super::ReaderDependency,
    crate::dependency::{dependencies, Dependency},
    umi_app::{Application, ApplicationReader},
    umi_genesis::config::GenesisConfig,
};

pub fn create(
    genesis_config: &GenesisConfig,
) -> (Application<Dependency>, ApplicationReader<ReaderDependency>) {
    let deps = dependencies();
    let reader_deps = deps.reader();

    (
        Application::new(deps, genesis_config),
        ApplicationReader::new(reader_deps, genesis_config),
    )
}

macro_rules! impl_shared {
    () => {
        type BlockHash = umi_blockchain::block::UmiBlockHash;
        type BaseTokenAccounts = umi_execution::UmiBaseTokenAccounts;
        type BaseGasFee = umi_blockchain::block::Eip1559GasFee;
        type CreateL1GasFee = umi_execution::CreateEcotoneL1GasFee;
        type CreateL2GasFee = umi_execution::CreateUmiL2GasFee;

        fn block_hash() -> Self::BlockHash {
            umi_blockchain::block::UmiBlockHash
        }

        fn base_gas_fee() -> Self::BaseGasFee {
            umi_blockchain::block::Eip1559GasFee::new(
                crate::EIP1559_ELASTICITY_MULTIPLIER,
                crate::EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR,
            )
        }

        fn create_l1_gas_fee() -> Self::CreateL1GasFee {
            umi_execution::CreateEcotoneL1GasFee
        }

        fn create_l2_gas_fee() -> Self::CreateL2GasFee {
            umi_execution::CreateUmiL2GasFee
        }

        fn base_token_accounts(genesis_config: &GenesisConfig) -> Self::BaseTokenAccounts {
            umi_execution::UmiBaseTokenAccounts::new(genesis_config.treasury)
        }
    };
}

pub(crate) use impl_shared;

#[cfg(any(feature = "storage-lmdb", feature = "storage-rocksdb"))]
pub(super) mod fallible {
    use std::{
        fmt::{Debug, Display},
        time::Duration,
    };

    pub(crate) fn retry<T, Err: Debug + Display>(f: impl Fn() -> Result<T, Err>) -> T {
        let mut tries = 1..60;

        loop {
            match f() {
                Ok(state) => return state,
                Err(error) if tries.next().is_none() => panic!("{error}"),
                Err(error) => {
                    let duration = Duration::from_secs(1);
                    eprintln!("WARN: Failed to create state {error}, retrying in {duration:?}...");
                    std::thread::sleep(duration);
                }
            }
        }
    }
}
